//! Same-stream WASAPI writer with IAudioClock on one COM thread.
//! Public execution may consume the probe; this module does not grant output
//! authorization. A returned probe publishes frequency, raw position, and
//! stream generation.
use nai_media_core::device_clock::DeviceClockCursor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};
use zeroize::Zeroizing;

/// Research-only one-shot lease: no renewal or production authority.
/// Local live leases expire as loss-of-contact; the external armed state never expires locally.
const FENCE_IDLE: u8 = 0;
const FENCE_LIVE: u8 = 1;
const FENCE_REVOKED: u8 = 2;
const FENCE_INITIALIZING: u8 = 3;
const FENCE_EXTERNAL: u8 = 4;
const LOCAL_LEASE_CEILING: Duration = Duration::from_millis(500);
const DROP_JOIN_BUDGET: Duration = Duration::from_millis(500);

pub struct ExperimentalFence {
    state: AtomicU8,
    deadline_ns: AtomicU64,
    base: Instant,
    externally_controlled: bool,
}
impl Default for ExperimentalFence {
    fn default() -> Self {
        Self {
            state: AtomicU8::new(FENCE_IDLE),
            deadline_ns: AtomicU64::new(0),
            base: Instant::now(),
            externally_controlled: false,
        }
    }
}
impl ExperimentalFence {
    /// Caller-owned fence; public execution supplies its own lease.
    pub fn externally_controlled() -> Self {
        Self {
            externally_controlled: true,
            ..Self::default()
        }
    }
    pub fn activate(&self) -> bool {
        if self.externally_controlled {
            return self
                .state
                .compare_exchange(
                    FENCE_IDLE,
                    FENCE_EXTERNAL,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok();
        }
        self.activate_for(LOCAL_LEASE_CEILING)
    }
    pub fn activate_for(&self, ttl: Duration) -> bool {
        if self.externally_controlled || ttl.is_zero() || ttl > LOCAL_LEASE_CEILING {
            return false;
        }
        // Reserve initialization so concurrent activation cannot extend a live lease.
        if self
            .state
            .compare_exchange(
                FENCE_IDLE,
                FENCE_INITIALIZING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false;
        }
        let Some(deadline) = u64::try_from(self.base.elapsed().as_nanos())
            .ok()
            .and_then(|now| now.checked_add(ttl.as_nanos() as u64))
        else {
            self.revoke();
            return false;
        };
        self.deadline_ns.store(deadline, Ordering::Release);
        self.state
            .compare_exchange(
                FENCE_INITIALIZING,
                FENCE_LIVE,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
    pub fn revoke(&self) {
        self.state.store(FENCE_REVOKED, Ordering::Release);
    }
    fn allowed(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state == FENCE_EXTERNAL {
            return true;
        }
        if state != FENCE_LIVE {
            return false;
        }
        let expired = u64::try_from(self.base.elapsed().as_nanos())
            .map_or(true, |now| now >= self.deadline_ns.load(Ordering::Acquire));
        if expired {
            self.revoke();
            return false;
        }
        self.state.load(Ordering::Acquire) == FENCE_LIVE
    }
    fn revoked(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        if state == FENCE_LIVE {
            let _ = self.allowed();
            return self.state.load(Ordering::Acquire) == FENCE_REVOKED;
        }
        state == FENCE_REVOKED
    }
}

pub struct Probe {
    stop: Arc<AtomicBool>,
    /// Request a WASAPI Stop without Reset. The audio thread owns the client.
    hold: Arc<AtomicBool>,
    /// True only after that Stop has returned and before Start.
    holding: Arc<AtomicBool>,
    /// Write silence while the clock keeps running. This does not stop the stream.
    gap: Arc<AtomicBool>,
    /// True after the audio thread has stopped extending the sentence for a gap.
    gapping: Arc<AtomicBool>,
    /// Source samples committed to the sentence. Gap silence is not included.
    sentence_cap: Arc<AtomicU64>,
    /// Device frames where the first cue interval starts. Zero before it exists.
    cue_device_start: Arc<AtomicU64>,
    /// Device frames where the first cue interval ends. Later silence is outside it.
    cue_device_end: Arc<AtomicU64>,
    /// Committed cue intervals. Start and end above are only the first interval.
    cue_span_count: Arc<AtomicU64>,
    gate: Arc<ExperimentalFence>,
    pub submitted: Arc<AtomicU64>,
    pub played: Arc<AtomicU64>,
    /// IAudioClock frequency. Zero until this stream has confirmed position zero.
    pub device_frequency: Arc<AtomicU64>,
    /// Raw IAudioClock position. Not a source-sample count.
    pub device_position: Arc<AtomicU64>,
    /// Assigned only after this client's position is confirmed at zero.
    pub stream_generation: Arc<AtomicU64>,
    /// This client's position immediately before Start. Not the previous stream's position.
    position_before_start: Arc<AtomicU64>,
    zero_confirmed: Arc<AtomicBool>,
    pub failed: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// One consistent reading from the stream that is writing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockReading {
    pub submitted_samples: u64,
    pub played_samples: u64,
    pub device_frequency: u64,
    pub device_position: u64,
    pub stream_generation: u64,
    /// Device frames of the first cue interval. Zero when that interval does not exist.
    pub cue_device_start: u64,
    pub cue_device_end: u64,
    /// Committed cue intervals. Zero means leading silence, not a cue.
    pub cue_span_count: u64,
}

/// Generation of a new device stream. Zero is not a generation.
/// The caller opens a new cursor at position zero and does not copy the old position.
pub fn fresh_stream_generation(previous: u64) -> u64 {
    previous.saturating_add(1)
}

impl Probe {
    /// Played is loaded before submitted. Position is stored before the played release.
    pub fn clock_reading(&self) -> ClockReading {
        let cue_span_count = self.cue_span_count.load(Ordering::Acquire);
        let cue_device_start = self.cue_device_start.load(Ordering::Acquire);
        let cue_device_end = self.cue_device_end.load(Ordering::Acquire);
        let played_samples = self.played.load(Ordering::Acquire);
        let device_position = self.device_position.load(Ordering::Acquire);
        let device_frequency = self.device_frequency.load(Ordering::Acquire);
        let stream_generation = self.stream_generation.load(Ordering::Acquire);
        let submitted_samples = self.submitted.load(Ordering::Acquire);
        ClockReading {
            submitted_samples,
            played_samples,
            device_frequency,
            device_position,
            stream_generation,
            cue_device_start,
            cue_device_end,
            cue_span_count,
        }
    }
    /// True only after this client read position zero and then started.
    pub fn confirmed_at_zero(&self) -> bool {
        self.zero_confirmed.load(Ordering::Acquire)
    }
    /// Position observed on this client before Start. A confirmed stream returns zero.
    pub fn position_before_start(&self) -> u64 {
        self.position_before_start.load(Ordering::Acquire)
    }
    /// Insert clock-running silence outside the sentence. This does not renew a lease.
    pub fn set_gap(&self, value: bool) {
        self.gap.store(value, Ordering::Release);
    }
    /// True once the audio thread is leaving the sentence unchanged for a gap.
    pub fn is_gapping(&self) -> bool {
        self.gapping.load(Ordering::Acquire)
    }
    /// Source samples the sentence can reach until another cue span is committed.
    pub fn sentence_cap(&self) -> u64 {
        self.sentence_cap.load(Ordering::Acquire)
    }
    /// Device-frame start of the first cue interval. Zero when that interval does not exist.
    pub fn cue_device_start(&self) -> u64 {
        self.cue_device_start.load(Ordering::Acquire)
    }
    /// Device-frame end of the first cue interval. Zero when that interval does not exist.
    pub fn cue_device_end(&self) -> u64 {
        self.cue_device_end.load(Ordering::Acquire)
    }
    /// Number of committed cue intervals. A count above one means start and end are not the whole sentence.
    pub fn cue_span_count(&self) -> u64 {
        self.cue_span_count.load(Ordering::Acquire)
    }
    /// Park or release the stream. This does not renew a lease.
    pub fn set_hold(&self, value: bool) {
        self.hold.store(value, Ordering::Release);
    }
    pub fn is_holding(&self) -> bool {
        self.holding.load(Ordering::Acquire)
    }
    pub fn is_finished(&self) -> bool {
        self.thread
            .as_ref()
            .is_none_or(|thread| thread.is_finished())
    }
    /// Requests fail-closed stop and waits at most the caller's bound for worker exit.
    /// A timeout is not hardware silence; the worker remains owned by this Probe.
    pub fn stop_within(&mut self, timeout: Duration) -> Result<(), &'static str> {
        if timeout > Duration::from_secs(2) {
            return Err("CLOCK_PROBE_INVALID_TIMEOUT");
        }
        self.gate.revoke();
        self.stop.store(true, Ordering::Release);
        let deadline = Instant::now() + timeout;
        while self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            if Instant::now() >= deadline {
                return Err("CLOCK_PROBE_STOP_TIMEOUT");
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| "CLOCK_PROBE_THREAD_FAILED")?;
        }
        Ok(())
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        if self.stop_within(DROP_JOIN_BUDGET).is_ok() {
            return;
        }
        // Timeout is not hardware silence; detaching keeps Drop from joining without a bound.
        if let Some(thread) = self.thread.take() {
            std::mem::forget(thread);
        }
    }
}

/// Explicit research-only silent signal; no user PCM or public permission.
pub fn open_silent(gate: Arc<ExperimentalFence>) -> Result<Probe, &'static str> {
    open_pcm(gate, Zeroizing::new(vec![0i16; 480000]))
}

/// Experimental only: no public-output authorization, never use for live output.
pub fn open_pcm(
    gate: Arc<ExperimentalFence>,
    pcm: Zeroizing<Vec<i16>>,
) -> Result<Probe, &'static str> {
    if gate.externally_controlled {
        return Err("CLOCK_PROBE_EXTERNAL_FENCE_REQUIRED");
    }
    open_pcm_controlled(gate, pcm, Arc::new(|| true))
}

/// Caller-owned nonblocking lease fence. Authorization stays with the caller.
/// Public execution may pass its callback fence as `permitted`.
/// The stream generation is 1. A rebuild must call [`open_pcm_controlled_generation`].
pub fn open_pcm_controlled(
    gate: Arc<ExperimentalFence>,
    pcm: Zeroizing<Vec<i16>>,
    permitted: Arc<dyn Fn() -> bool + Send + Sync>,
) -> Result<Probe, &'static str> {
    open_pcm_controlled_generation(gate, pcm, permitted, 0)
}

/// Opens the next device stream. `previous_generation` chooses only the generation.
/// The new client confirms its own position is zero and receives no previous position.
pub fn open_pcm_controlled_generation(
    gate: Arc<ExperimentalFence>,
    pcm: Zeroizing<Vec<i16>>,
    permitted: Arc<dyn Fn() -> bool + Send + Sync>,
    previous_generation: u64,
) -> Result<Probe, &'static str> {
    if pcm.is_empty() || pcm.len() > 480000 {
        return Err("CLOCK_PROBE_INVALID_PCM");
    }
    let stop = Arc::new(AtomicBool::new(false));
    let hold = Arc::new(AtomicBool::new(false));
    let holding = Arc::new(AtomicBool::new(false));
    let gap = Arc::new(AtomicBool::new(false));
    let gapping = Arc::new(AtomicBool::new(false));
    let sentence_cap = Arc::new(AtomicU64::new(0));
    let cue_device_start = Arc::new(AtomicU64::new(0));
    let cue_device_end = Arc::new(AtomicU64::new(0));
    let cue_span_count = Arc::new(AtomicU64::new(0));
    let submitted = Arc::new(AtomicU64::new(0));
    let played = Arc::new(AtomicU64::new(0));
    let device_frequency = Arc::new(AtomicU64::new(0));
    let device_position = Arc::new(AtomicU64::new(0));
    let stream_generation = Arc::new(AtomicU64::new(0));
    let position_before_start = Arc::new(AtomicU64::new(0));
    let zero_confirmed = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (
        s,
        h,
        g,
        w,
        p,
        f,
        freq,
        pos,
        generation_slot,
        started_at,
        confirmed,
        gap_request,
        gapping_flag,
        cap_slot,
        span_start,
        span_end,
        span_count_slot,
    ) = (
        stop.clone(),
        hold.clone(),
        holding.clone(),
        submitted.clone(),
        played.clone(),
        failed.clone(),
        device_frequency.clone(),
        device_position.clone(),
        stream_generation.clone(),
        position_before_start.clone(),
        zero_confirmed.clone(),
        gap.clone(),
        gapping.clone(),
        sentence_cap.clone(),
        cue_device_start.clone(),
        cue_device_end.clone(),
        cue_span_count.clone(),
    );
    let worker_gate = gate.clone();
    let thread = std::thread::Builder::new()
        .name("nai-clocked-experiment".into())
        .spawn(move || {
            let outcome = drive(
                &s,
                &h,
                &g,
                &worker_gate,
                &w,
                &p,
                &freq,
                &pos,
                &generation_slot,
                &started_at,
                &confirmed,
                &gap_request,
                &gapping_flag,
                &cap_slot,
                &span_start,
                &span_end,
                &span_count_slot,
                previous_generation,
                &ready_tx,
                pcm,
                &*permitted,
            );
            g.store(false, Ordering::Release);
            worker_gate.revoke();
            if outcome.is_err() {
                f.store(true, Ordering::Release);
                let _ = ready_tx.try_send(Err("CLOCK_PROBE_FAILED"));
            }
        })
        .map_err(|_| "CLOCK_PROBE_THREAD")?;
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(Probe {
            stop,
            hold,
            holding,
            gap,
            gapping,
            sentence_cap,
            cue_device_start,
            cue_device_end,
            cue_span_count,
            gate,
            submitted,
            played,
            device_frequency,
            device_position,
            stream_generation,
            position_before_start,
            zero_confirmed,
            failed,
            thread: Some(thread),
        }),
        _ => {
            gate.revoke();
            stop.store(true, Ordering::Release);
            let _ = thread.join();
            Err("CLOCK_PROBE_UNAVAILABLE")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    stop: &AtomicBool,
    hold: &AtomicBool,
    holding: &AtomicBool,
    gate: &ExperimentalFence,
    submitted: &AtomicU64,
    played: &AtomicU64,
    device_frequency: &AtomicU64,
    device_position: &AtomicU64,
    stream_generation: &AtomicU64,
    position_before_start: &AtomicU64,
    zero_confirmed: &AtomicBool,
    gap: &AtomicBool,
    gapping: &AtomicBool,
    sentence_cap: &AtomicU64,
    cue_device_start: &AtomicU64,
    cue_device_end: &AtomicU64,
    cue_span_count: &AtomicU64,
    previous_generation: u64,
    ready: &mpsc::SyncSender<Result<(), &'static str>>,
    pcm: Zeroizing<Vec<i16>>,
    permitted: &dyn Fn() -> bool,
) -> Result<(), ()> {
    wasapi::initialize_mta().ok().map_err(|_| ())?;
    let device = DeviceEnumerator::new()
        .map_err(|_| ())?
        .get_default_device(&Direction::Render)
        .map_err(|_| ())?;
    let mut client = device.get_iaudioclient().map_err(|_| ())?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, 48000, 2, None);
    client
        .initialize_client(
            &format,
            &Direction::Render,
            &StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 200_000,
            },
        )
        .map_err(|_| ())?;
    let event = client.set_get_eventhandle().map_err(|_| ())?;
    let render = client.get_audiorenderclient().map_err(|_| ())?;
    let clock = client.get_audioclock().map_err(|_| ())?;
    let frequency = clock.get_frequency().map_err(|_| ())?;
    if clock.get_position().map_err(|_| ())?.0 != 0 {
        return Err(());
    }
    let generation = fresh_stream_generation(previous_generation);
    let mut cursor = DeviceClockCursor::new(generation, frequency, 48000).map_err(|_| ())?;
    let mut bytes = Zeroizing::new([0u8; 4800 * 8]);
    let first = client.get_available_space_in_frames().map_err(|_| ())? as usize;
    if first == 0 || first > 4800 {
        return Err(());
    }
    render
        .write_to_device(first, &bytes[..first * 8], None)
        .map_err(|_| ())?;
    cursor.commit(first as u64).map_err(|_| ())?;
    let starting_position = clock.get_position().map_err(|_| ())?.0;
    if starting_position != 0 {
        return Err(());
    }
    client.start_stream().map_err(|_| ())?;
    position_before_start.store(starting_position, Ordering::Release);
    zero_confirmed.store(true, Ordering::Release);
    stream_generation.store(generation, Ordering::Release);
    device_frequency.store(frequency, Ordering::Release);
    struct Stop<'a>(&'a wasapi::AudioClient);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = self.0.stop_stream();
        }
    }
    let _guard = Stop(&client);
    ready.try_send(Ok(())).map_err(|_| ())?;
    let mut device_submitted = first as u64;
    let mut spans = [CueSpan { start: 0, end: 0 }; 4];
    let mut span_count = 0usize;
    let mut cue_device_frames = 0u64;
    let mut parked = false;
    while !stop.load(Ordering::Acquire) {
        // Stop without Reset freezes IAudioClock. Hold wins over a gap.
        // Pause still consumes the caller-owned lease before publishing.
        if hold.load(Ordering::Acquire) {
            gapping.store(false, Ordering::Release);
            if !after_wait_may_publish(stop, gate, permitted) {
                break;
            }
            if !parked {
                client.stop_stream().map_err(|_| ())?;
                parked = true;
                holding.store(true, Ordering::Release);
            }
            let position = clock.get_position().map_err(|_| ())?.0;
            let frequency_now = clock.get_frequency().map_err(|_| ())?;
            let device_played =
                observe_verified(&mut cursor, gate, generation, frequency_now, position)?;
            publish_verified(
                device_frequency,
                device_position,
                played,
                frequency_now,
                position,
                published_reach(&spans[..span_count], device_played),
            );
            if gate.revoked() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        if parked {
            client.start_stream().map_err(|_| ())?;
            parked = false;
            holding.store(false, Ordering::Release);
        }
        // Sample before the wait so this write matches the flag the caller observes.
        let in_gap = gap.load(Ordering::Acquire);
        gapping.store(in_gap, Ordering::Release);
        event.wait_for_event(100).map_err(|_| ())?;
        if !after_wait_may_publish(stop, gate, permitted) {
            break;
        }
        let position = clock.get_position().map_err(|_| ())?.0;
        let frequency_now = clock.get_frequency().map_err(|_| ())?;
        let device_played =
            observe_verified(&mut cursor, gate, generation, frequency_now, position)?;
        publish_verified(
            device_frequency,
            device_position,
            played,
            frequency_now,
            position,
            published_reach(&spans[..span_count], device_played),
        );
        if gate.revoked() {
            break;
        }
        let count = client.get_available_space_in_frames().map_err(|_| ())? as usize;
        if count > 4800 {
            return Err(());
        }
        if count == 0 {
            continue;
        }
        let armed = gate.allowed();
        let sentence = armed && !in_gap;
        if !consume_external_lease(gate, armed, permitted) {
            break;
        }
        bytes[..count * 8].fill(0);
        let cue_frames = if sentence {
            stage_pcm(&pcm, cue_device_frames, &mut bytes[..count * 8])
        } else {
            0
        };
        // Recheck the independent fence after staging, before native submission.
        if !consume_external_lease(gate, armed, permitted) {
            break;
        }
        // Silence still commits while a gap is open. The lease check uses the armed flag.
        if !write_committed(&mut cursor, gate, stop, armed, count as u64, || {
            render
                .write_to_device(count, &bytes[..count * 8], None)
                .map_err(|_| ())
        })? {
            break;
        }
        if sentence && cue_frames > 0 {
            commit_span(&mut spans, &mut span_count, device_submitted, cue_frames)?;
            cue_device_frames = cue_device_frames.checked_add(cue_frames).ok_or(())?;
            let committed = &spans[..span_count];
            cue_device_start.store(spans[0].start, Ordering::Release);
            cue_device_end.store(spans[0].end, Ordering::Release);
            sentence_cap.store(spans_played_cap(committed), Ordering::Release);
            submitted.store(
                spans_submitted(committed, pcm.len() as u64),
                Ordering::Release,
            );
            cue_span_count.store(
                u64::try_from(span_count).unwrap_or(u64::MAX),
                Ordering::Release,
            );
        }
        if span_count > 0 && played.load(Ordering::Acquire) >= pcm.len() as u64 {
            break;
        }
        device_submitted = device_submitted.checked_add(count as u64).ok_or(())?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CueSpan {
    start: u64,
    end: u64,
}

fn span_played(device_frames: u64, span: CueSpan) -> u64 {
    if span.end <= span.start {
        return 0;
    }
    device_frames
        .saturating_sub(span.start)
        .min(span.end - span.start)
        / 2
}

fn spans_played(device_frames: u64, spans: &[CueSpan]) -> u64 {
    spans.iter().fold(0u64, |sum, span| {
        sum.saturating_add(span_played(device_frames, *span))
    })
}

fn spans_played_cap(spans: &[CueSpan]) -> u64 {
    spans.iter().fold(0u64, |sum, span| {
        sum.saturating_add(span.end.saturating_sub(span.start) / 2)
    })
}

fn spans_submitted(spans: &[CueSpan], pcm_len: u64) -> u64 {
    spans
        .iter()
        .fold(0u64, |sum, span| {
            sum.saturating_add(span.end.saturating_sub(span.start).div_ceil(2))
        })
        .min(pcm_len)
}

/// Extend the open span when this write starts where the previous cue ended.
/// A later device cursor, after silence, starts another span. Four spans is the bound.
fn commit_span(
    spans: &mut [CueSpan; 4],
    count: &mut usize,
    at: u64,
    frames: u64,
) -> Result<(), ()> {
    if frames == 0 {
        return Ok(());
    }
    let end = at.checked_add(frames).ok_or(())?;
    if *count > 0 && spans[*count - 1].end == at {
        spans[*count - 1].end = end;
        return Ok(());
    }
    if *count >= spans.len() {
        return Err(());
    }
    spans[*count] = CueSpan { start: at, end };
    *count += 1;
    Ok(())
}

fn published_reach(spans: &[CueSpan], device_played: u64) -> Option<u64> {
    if spans.is_empty() {
        None
    } else {
        Some(spans_played(device_played, spans))
    }
}

fn publish_verified(
    device_frequency: &AtomicU64,
    device_position: &AtomicU64,
    played: &AtomicU64,
    frequency: u64,
    position: u64,
    reached: Option<u64>,
) {
    // Position is released before played, so a reader that observes played also sees it.
    device_frequency.store(frequency, Ordering::Release);
    device_position.store(position, Ordering::Release);
    if let Some(reached) = reached {
        played.store(reached, Ordering::Release);
    }
}

fn observe_verified(
    cursor: &mut DeviceClockCursor,
    gate: &ExperimentalFence,
    stream: u64,
    frequency: u64,
    position: u64,
) -> Result<u64, ()> {
    cursor.observe(stream, frequency, position).map_err(|_| {
        gate.revoke();
    })
}

/// Armed writes consult the caller-owned nonblocking lease. A false callback
/// revokes the one-shot fence and stops; that stop is not a device fault.
/// Unarmed writes skip the callback so silence can fill before activation.
fn consume_external_lease(
    gate: &ExperimentalFence,
    armed: bool,
    permitted: impl FnOnce() -> bool,
) -> bool {
    if armed && !permitted() {
        gate.revoke();
        return false;
    }
    true
}

/// Hold ticks consult the caller-owned lease before publishing an observation.
/// A false callback revokes and stops; that stop is not a device fault.
/// Unarmed ticks skip the callback so silence can fill before activation.
fn hold_may_publish(gate: &ExperimentalFence, permitted: impl FnOnce() -> bool) -> bool {
    if !consume_external_lease(gate, gate.allowed(), permitted) {
        return false;
    }
    !gate.revoked()
}

/// After an event wait, check stop then the caller-owned lease before publishing.
/// Stop does not itself revoke. A false callback revokes and stops; that stop is
/// not a device fault. The wait itself may still run to its timeout.
fn after_wait_may_publish(
    stop: &AtomicBool,
    gate: &ExperimentalFence,
    permitted: impl FnOnce() -> bool,
) -> bool {
    if stop.load(Ordering::Acquire) {
        return false;
    }
    hold_may_publish(gate, permitted)
}

fn write_committed(
    cursor: &mut DeviceClockCursor,
    gate: &ExperimentalFence,
    stop: &AtomicBool,
    active: bool,
    frames: u64,
    write: impl FnOnce() -> Result<(), ()>,
) -> Result<bool, ()> {
    if stop.load(Ordering::Acquire) || gate.revoked() || (active && !gate.allowed()) {
        return Ok(false);
    }
    if write().is_err() || cursor.commit(frames).is_err() {
        gate.revoke();
        return Err(());
    }
    Ok(true)
}

fn stage_pcm(pcm: &[i16], already: u64, bytes: &mut [u8]) -> u64 {
    let mut frames = 0;
    for output in bytes.chunks_exact_mut(8) {
        let source_frame = already + frames;
        let index = (source_frame / 2) as usize;
        if index >= pcm.len() {
            break;
        }
        let next = (index + 1).min(pcm.len() - 1);
        let a = f32::from(pcm[index]);
        let b = f32::from(pcm[next]);
        let fraction = (source_frame % 2) as f32 * 0.5;
        let sample = ((a + (b - a) * fraction) / 32768.0 * 0.5).to_le_bytes();
        output[..4].copy_from_slice(&sample);
        output[4..].copy_from_slice(&sample);
        frames += 1;
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_mode_requires_explicit_callback_and_revocation_is_terminal() {
        let gate = Arc::new(ExperimentalFence::externally_controlled());
        assert_eq!(
            open_pcm(gate.clone(), Zeroizing::new(vec![0; 4])).err(),
            Some("CLOCK_PROBE_EXTERNAL_FENCE_REQUIRED")
        );
        assert!(gate.activate());
        assert!(gate.allowed());
        assert!(!gate.activate());
        gate.revoke();
        assert!(!gate.allowed());
        assert!(!gate.activate());
    }

    #[test]
    fn write_stops_when_external_lease_drops() {
        let gate = ExperimentalFence::externally_controlled();
        assert!(gate.activate());
        let permitted = AtomicBool::new(true);
        assert!(consume_external_lease(&gate, true, || permitted.load(Ordering::Acquire)));
        permitted.store(false, Ordering::Release);
        assert!(!consume_external_lease(&gate, true, || permitted.load(Ordering::Acquire)));
        assert!(gate.revoked());
        let stop = AtomicBool::new(false);
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        let mut wrote = false;
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || {
                wrote = true;
                Ok(())
            }),
            Ok(false)
        );
        assert!(!wrote);
        assert!(!gate.activate());

        let idle = ExperimentalFence::externally_controlled();
        assert!(idle.activate());
        assert!(consume_external_lease(&idle, false, || false));
        assert!(idle.allowed());
    }
    #[test]
    fn hold_stops_publishing_when_external_lease_drops() {
        let gate = ExperimentalFence::externally_controlled();
        assert!(gate.activate());
        let permitted = AtomicBool::new(true);
        assert!(hold_may_publish(&gate, || permitted.load(Ordering::Acquire)));
        permitted.store(false, Ordering::Release);
        assert!(!hold_may_publish(&gate, || permitted.load(Ordering::Acquire)));
        assert!(gate.revoked());
        assert!(!hold_may_publish(&gate, || true));
        assert!(!gate.activate());

        let idle = ExperimentalFence::default();
        assert!(hold_may_publish(&idle, || false));
        assert!(!idle.revoked());
    }
    #[test]
    fn after_wait_stop_or_lease_drop_refuses_publish() {
        let gate = ExperimentalFence::externally_controlled();
        assert!(gate.activate());
        let stop = AtomicBool::new(true);
        let permitted = AtomicBool::new(true);
        assert!(!after_wait_may_publish(&stop, &gate, || permitted.load(Ordering::Acquire)));
        assert!(gate.allowed());
        stop.store(false, Ordering::Release);
        assert!(after_wait_may_publish(&stop, &gate, || permitted.load(Ordering::Acquire)));
        permitted.store(false, Ordering::Release);
        assert!(!after_wait_may_publish(&stop, &gate, || permitted.load(Ordering::Acquire)));
        assert!(gate.revoked());
        permitted.store(true, Ordering::Release);
        assert!(!after_wait_may_publish(&stop, &gate, || true));
        assert!(!gate.activate());

        let idle = ExperimentalFence::default();
        let idle_stop = AtomicBool::new(false);
        assert!(after_wait_may_publish(&idle_stop, &idle, || false));
        assert!(!idle.revoked());
    }
    #[test]
    fn expired_lease_cannot_be_reactivated_or_extended() {
        let gate = ExperimentalFence::default();
        assert!(!gate.activate_for(Duration::ZERO));
        assert!(!gate.activate_for(Duration::from_millis(501)));
        assert!(gate.activate_for(Duration::from_millis(20)));
        assert!(!gate.activate());
        std::thread::sleep(Duration::from_millis(35));
        assert!(!gate.allowed());
        assert!(gate.revoked());
        assert!(!gate.activate());
    }

    #[test]
    fn expired_local_lease_revokes_as_loss_of_contact() {
        let gate = ExperimentalFence::default();
        assert!(gate.activate_for(Duration::from_millis(20)));
        std::thread::sleep(Duration::from_millis(35));
        let stop = AtomicBool::new(false);
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        let mut wrote = false;
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || {
                wrote = true;
                Ok(())
            }),
            Ok(false)
        );
        assert!(!wrote);
        assert!(gate.revoked());
        assert!(!gate.activate());
        assert!(!gate.activate_for(Duration::from_millis(20)));
        assert!(!gate.activate_for(LOCAL_LEASE_CEILING));

        let external = ExperimentalFence::externally_controlled();
        assert!(external.activate());
        std::thread::sleep(Duration::from_millis(35));
        assert!(external.allowed());
        assert!(!external.revoked());
        assert!(!external.activate_for(Duration::from_millis(20)));
        let mut external_wrote = false;
        assert_eq!(
            write_committed(&mut cursor, &external, &stop, true, 4, || {
                external_wrote = true;
                Ok(())
            }),
            Ok(true)
        );
        assert!(external_wrote);
        external.revoke();
        assert!(external.revoked());
        assert!(!external.allowed());
        assert!(!external.activate());
    }
    #[test]
    fn revoked_fence_is_one_shot_even_before_activation() {
        let gate = ExperimentalFence::default();
        assert!(gate.activate());
        assert!(gate.allowed());
        gate.revoke();
        assert!(!gate.activate());
        assert!(!gate.allowed());
        let pre_revoked = ExperimentalFence::default();
        pre_revoked.revoke();
        assert!(!pre_revoked.activate());
    }
    #[test]
    fn unarmed_write_commits_silence_without_publishing_played() {
        let gate = ExperimentalFence::default();
        assert!(!gate.allowed());
        let stop = AtomicBool::new(false);
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        let mut wrote = false;
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, false, 4, || {
                wrote = true;
                Ok(())
            }),
            Ok(true)
        );
        assert!(wrote);
        let frequency = AtomicU64::new(0);
        let position = AtomicU64::new(0);
        let played = AtomicU64::new(0);
        publish_verified(&frequency, &position, &played, 384000, 100, None);
        assert_eq!(played.load(Ordering::Acquire), 0);
        assert_eq!(position.load(Ordering::Acquire), 100);
        assert_eq!(frequency.load(Ordering::Acquire), 384000);
    }

    #[test]
    fn observe_with_wrong_generation_revokes() {
        let gate = ExperimentalFence::default();
        assert!(gate.activate());
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        cursor.commit(10).unwrap();
        assert_eq!(observe_verified(&mut cursor, &gate, 2, 48000, 4), Err(()));
        assert!(gate.revoked());
        assert!(!gate.activate());
    }

    #[test]
    fn bounded_pcm_rejects_empty_and_oversize_before_device() {
        let gate = Arc::new(ExperimentalFence::default());
        assert_eq!(
            open_pcm(gate.clone(), Zeroizing::new(vec![])).err(),
            Some("CLOCK_PROBE_INVALID_PCM")
        );
        assert_eq!(
            open_pcm(gate, Zeroizing::new(vec![0; 480001])).err(),
            Some("CLOCK_PROBE_INVALID_PCM")
        );
    }

    #[test]
    fn failed_write_cannot_reuse_fence_and_next_generation_starts_clean() {
        let gate = ExperimentalFence::default();
        assert!(gate.activate());
        let stop = AtomicBool::new(false);
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || Err(())),
            Err(())
        );
        assert!(gate.revoked());
        assert!(!gate.activate());
        assert!(!gate.activate_for(Duration::from_millis(20)));
        let next_gate = ExperimentalFence::default();
        assert!(next_gate.activate());
        let next_gen = fresh_stream_generation(1);
        assert_eq!(next_gen, 2);
        let mut rebuilt = DeviceClockCursor::new(next_gen, 48000, 48000).unwrap();
        assert_eq!(
            write_committed(&mut rebuilt, &next_gate, &stop, true, 4, || Ok(())),
            Ok(true)
        );
        assert_eq!(rebuilt.observe(next_gen, 48000, 0), Ok(0));
        assert!(cursor.observe(1, 48000, 1).is_err());
    }

    #[test]
    fn write_failure_or_clock_fault_revokes_without_fake_progress() {
        let gate = ExperimentalFence::default();
        assert!(gate.activate());
        let stop = AtomicBool::new(false);
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || Err(())),
            Err(())
        );
        assert!(gate.revoked());
        assert!(cursor.observe(1, 48000, 1).is_err());
        let gate = ExperimentalFence::default();
        assert!(gate.activate());
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || Ok(())),
            Ok(true)
        );
        assert_eq!(observe_verified(&mut cursor, &gate, 1, 48000, 5), Err(()));
        assert!(gate.revoked());
        assert!(!gate.activate());
    }
    #[test]
    fn stopped_or_revoked_fence_never_calls_writer() {
        let gate = ExperimentalFence::default();
        assert!(gate.activate());
        gate.revoke();
        let stop = AtomicBool::new(false);
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || panic!(
                "write called"
            )),
            Ok(false)
        );
        assert!(cursor.observe(1, 48000, 1).is_err());
    }
    fn parked_probe(gate: Arc<ExperimentalFence>, park: Duration) -> Probe {
        let thread = std::thread::spawn(move || std::thread::sleep(park));
        Probe {
            stop: Arc::new(AtomicBool::new(false)),
            hold: Arc::new(AtomicBool::new(false)),
            holding: Arc::new(AtomicBool::new(false)),
            gap: Arc::new(AtomicBool::new(false)),
            gapping: Arc::new(AtomicBool::new(false)),
            sentence_cap: Arc::new(AtomicU64::new(0)),
            cue_device_start: Arc::new(AtomicU64::new(0)),
            cue_device_end: Arc::new(AtomicU64::new(0)),
            cue_span_count: Arc::new(AtomicU64::new(0)),
            gate,
            submitted: Arc::new(AtomicU64::new(0)),
            played: Arc::new(AtomicU64::new(0)),
            device_frequency: Arc::new(AtomicU64::new(0)),
            device_position: Arc::new(AtomicU64::new(0)),
            stream_generation: Arc::new(AtomicU64::new(0)),
            position_before_start: Arc::new(AtomicU64::new(0)),
            zero_confirmed: Arc::new(AtomicBool::new(false)),
            failed: Arc::new(AtomicBool::new(false)),
            thread: Some(thread),
        }
    }
    #[test]
    fn stop_timeout_reports_worker_still_running_and_keeps_ownership() {
        let gate = Arc::new(ExperimentalFence::default());
        assert!(gate.activate());
        let mut probe = parked_probe(gate.clone(), Duration::from_millis(150));
        assert_eq!(
            probe.stop_within(Duration::from_millis(10)),
            Err("CLOCK_PROBE_STOP_TIMEOUT")
        );
        assert!(gate.revoked());
        assert!(probe.thread.is_some());
        assert!(probe.stop_within(Duration::from_secs(1)).is_ok());
        assert!(probe.thread.is_none());
    }
    #[test]
    fn drop_of_unfinished_probe_returns_within_budget() {
        let gate = Arc::new(ExperimentalFence::default());
        assert!(gate.activate());
        let park = Duration::from_secs(2);
        let probe = parked_probe(gate.clone(), park);
        let stop = probe.stop.clone();
        let started = Instant::now();
        drop(probe);
        let elapsed = started.elapsed();
        assert!(
            elapsed <= DROP_JOIN_BUDGET + Duration::from_millis(250),
            "drop waited {elapsed:?}"
        );
        assert!(elapsed < park, "drop joined the unfinished worker");
        assert!(gate.revoked());
        assert!(stop.load(Ordering::Acquire));
    }
    #[test]
    fn revoke_after_commit_keeps_submitted_cursor_and_refuses_new_writes() {
        let gate = ExperimentalFence::default();
        assert!(gate.activate());
        let stop = AtomicBool::new(false);
        let mut cursor = DeviceClockCursor::new(1, 48000, 48000).unwrap();
        let mut writes = 0u32;
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || {
                writes += 1;
                Ok(())
            }),
            Ok(true)
        );
        assert_eq!(writes, 1);
        assert_eq!(cursor.observe(1, 48000, 4), Ok(4));
        gate.revoke();
        assert_eq!(
            write_committed(&mut cursor, &gate, &stop, true, 4, || {
                writes += 1;
                Ok(())
            }),
            Ok(false)
        );
        assert_eq!(writes, 1);
        assert_eq!(cursor.observe(1, 48000, 4), Ok(4));
        assert!(cursor.observe(1, 48000, 5).is_err());
        assert!(gate.revoked());
    }
    #[test]
    fn rebuilt_stream_takes_the_next_generation_and_a_new_cursor() {
        let first = fresh_stream_generation(0);
        let second = fresh_stream_generation(first);
        assert_eq!((first, second), (1, 2));
        assert_eq!(fresh_stream_generation(u64::MAX), u64::MAX);
        let mut older = DeviceClockCursor::new(first, 48000, 48000).unwrap();
        older.commit(10).unwrap();
        assert!(older.observe(first, 48000, 4).is_ok());
        let mut rebuilt = DeviceClockCursor::new(second, 48000, 48000).unwrap();
        rebuilt.commit(10).unwrap();
        assert!(rebuilt.observe(first, 48000, 4).is_err());
        assert_eq!(rebuilt.observe(second, 48000, 0), Ok(0));
    }
    #[test]
    fn published_position_precedes_played_samples() {
        let frequency = AtomicU64::new(0);
        let position = AtomicU64::new(0);
        let played = AtomicU64::new(0);
        publish_verified(&frequency, &position, &played, 384000, 436992, Some(240));
        assert_eq!(played.load(Ordering::Acquire), 240);
        assert_eq!(position.load(Ordering::Acquire), 436992);
        assert_eq!(frequency.load(Ordering::Acquire), 384000);
        let played_before_cue = AtomicU64::new(0);
        publish_verified(&frequency, &position, &played_before_cue, 384000, 100, None);
        assert_eq!(played_before_cue.load(Ordering::Acquire), 0);
        assert_eq!(position.load(Ordering::Acquire), 100);
        publish_verified(&frequency, &position, &played, 384000, 200, None);
        assert_eq!(played.load(Ordering::Acquire), 240);
    }
    #[test]
    fn one_span_matches_the_contiguous_cue_formula() {
        let span = CueSpan { start: 0, end: 960 };
        assert_eq!(span_played(480, span), 240);
        assert_eq!(spans_played(480, &[span]), 240);
        assert_eq!(spans_submitted(&[span], 10_000), 480);
        assert_eq!(spans_played_cap(&[span]), 480);
        assert_eq!(
            span_played(
                0,
                CueSpan {
                    start: 100,
                    end: 200
                }
            ),
            0
        );
        let odd = CueSpan { start: 0, end: 101 };
        assert_eq!(spans_submitted(&[odd], 10_000), 51);
        assert_eq!(spans_played_cap(&[odd]), 50);
        assert_eq!(span_played(101, odd), 50);
    }
    #[test]
    fn gap_between_spans_holds_played_samples_until_the_next_span() {
        let first = CueSpan {
            start: 100,
            end: 200,
        };
        let second = CueSpan {
            start: 500,
            end: 700,
        };
        assert_eq!(spans_played(50, &[first]), 0);
        assert_eq!(spans_played(400, &[first, second]), 50);
        assert_eq!(spans_played(400, &[first]), spans_played_cap(&[first]));
        assert_eq!(spans_played(600, &[first, second]), 100);
        assert!(spans_played(600, &[first, second]) > spans_played(400, &[first, second]));
    }
    #[test]
    fn commit_span_extends_when_contiguous_and_splits_after_a_gap() {
        let mut spans = [CueSpan { start: 0, end: 0 }; 4];
        let mut count = 0usize;
        commit_span(&mut spans, &mut count, 100, 50).unwrap();
        commit_span(&mut spans, &mut count, 150, 50).unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            spans[0],
            CueSpan {
                start: 100,
                end: 200
            }
        );
        commit_span(&mut spans, &mut count, 500, 200).unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            spans[1],
            CueSpan {
                start: 500,
                end: 700
            }
        );
        commit_span(&mut spans, &mut count, 900, 1).unwrap();
        commit_span(&mut spans, &mut count, 901, 1).unwrap();
        assert_eq!(count, 3);
        assert!(commit_span(&mut spans, &mut count, 2000, 1).is_ok());
        assert!(commit_span(&mut spans, &mut count, 3000, 1).is_err());
    }
    #[test]
    fn staged_pcm_is_stereo_resampled_and_tail_is_bounded() {
        let mut bytes = [0u8; 48];
        assert_eq!(stage_pcm(&[0, 16384], 0, &mut bytes), 4);
        let samples: Vec<_> = bytes
            .chunks_exact(8)
            .map(|v| {
                assert_eq!(&v[..4], &v[4..]);
                f32::from_le_bytes(v[..4].try_into().unwrap())
            })
            .collect();
        assert_eq!(samples, vec![0.0, 0.125, 0.25, 0.25, 0.0, 0.0]);
        assert_eq!(stage_pcm(&[0, 16384], 3, &mut [0u8; 16]), 1);
    }
}

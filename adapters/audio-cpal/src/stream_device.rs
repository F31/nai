//! Incremental mono source -> WASAPI, with a bounded pending device timeline.
use nai_media_core::{
    device_clock::DeviceClockCursor,
    stream_pcm::{Buffer, Timeline},
    v561_audio_lease::AudioLease,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
// A bounded acquisition within a 20ms WASAPI buffer period, never an
// unbounded blocking lock. Revocation is checked on every attempt.
fn source_lock<T>(
    mutex: &Mutex<T>,
    live: impl Fn() -> bool,
    budget: Duration,
) -> Result<std::sync::MutexGuard<'_, T>, ()> {
    let until = Instant::now() + budget;
    loop {
        if !live() {
            return Err(());
        }
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(_)) => return Err(()),
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
        if Instant::now() >= until {
            return Err(());
        }
        std::thread::yield_now();
    }
}
#[derive(Clone, Copy)]
pub struct Observation {
    pub played_samples: u64,
    pub observed_at: Instant,
}

pub struct Device {
    pub generation: u64,
    observation: Arc<Mutex<Option<Observation>>>,
    pub submitted: Arc<AtomicU64>,
    pub played: Arc<AtomicU64>,
    pub failed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
#[derive(Debug)]
pub struct DeviceError;
impl Drop for Device {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let end = Instant::now() + Duration::from_millis(500);
        while self.thread.as_ref().is_some_and(|t| !t.is_finished()) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(2));
        }
        if let Some(thread) = self.thread.take().filter(|t| t.is_finished()) {
            let _ = thread.join();
        }
    }
}
impl Device {
    pub fn observation(&self) -> Option<Observation> {
        self.observation.lock().ok().and_then(|v| *v)
    }
    pub fn open(
        buffer: Arc<Mutex<Buffer>>,
        lease: Arc<AudioLease>,
        until: Arc<AtomicU64>,
        prefill: u32,
    ) -> Result<Self, DeviceError> {
        let generation = NEXT_GENERATION
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .map_err(|_| DeviceError)?;
        let observation = Arc::new(Mutex::new(None));
        let observed = observation.clone();
        let submitted = Arc::new(AtomicU64::new(0));
        let played = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel(1);
        let (s, p, f, x) = (
            submitted.clone(),
            played.clone(),
            failed.clone(),
            stop.clone(),
        );
        let thread = std::thread::spawn(move || {
            if run(
                buffer, lease, until, prefill, generation, observed, &s, &p, &x, &tx,
            )
            .is_err()
            {
                f.store(true, Ordering::Release);
                let _ = tx.try_send(Err(()));
            }
        });
        let result = Self {
            generation,
            observation,
            submitted,
            played,
            failed,
            stop,
            thread: Some(thread),
        };
        rx.recv_timeout(Duration::from_secs(3))
            .map_err(|_| DeviceError)?
            .map_err(|_| DeviceError)?;
        Ok(result)
    }
}
#[allow(clippy::too_many_arguments)]
fn run(
    buffer: Arc<Mutex<Buffer>>,
    lease: Arc<AudioLease>,
    until: Arc<AtomicU64>,
    prefill: u32,
    generation: u64,
    observation: Arc<Mutex<Option<Observation>>>,
    submitted: &AtomicU64,
    played: &AtomicU64,
    stop: &AtomicBool,
    ready: &mpsc::SyncSender<Result<(), ()>>,
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
    let mut cursor = DeviceClockCursor::new(generation, frequency, 48000).map_err(|_| ())?;
    let mut bytes = zeroize::Zeroizing::new(vec![0u8; 4800 * 8]);
    let first = client.get_available_space_in_frames().map_err(|_| ())? as usize;
    if first == 0 || first > 4800 {
        return Err(());
    }
    render
        .write_to_device(first, &bytes[..first * 8], None)
        .map_err(|_| ())?;
    cursor.commit(first as u64).map_err(|_| ())?;
    client.start_stream().map_err(|_| ())?;
    struct Stop<'a>(&'a wasapi::AudioClient);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            let _ = self.0.stop_stream();
        }
    }
    let _guard = Stop(&client);
    let observed_at = Instant::now();
    cursor
        .observe(
            generation,
            clock.get_frequency().map_err(|_| ())?,
            clock.get_position().map_err(|_| ())?.0,
        )
        .map_err(|_| ())?;
    *observation.lock().map_err(|_| ())? = Some(Observation {
        played_samples: 0,
        observed_at,
    });
    ready.try_send(Ok(())).map_err(|_| ())?;
    let mut timeline = Timeline::default();
    let mut device_end = first as u64;
    let mut armed = false;
    let mut started = false;
    let mut first_pcm_at = None;
    while !stop.load(Ordering::Acquire) {
        event.wait_for_event(100).map_err(|_| ())?;
        let allowed = lease.allowed();
        if armed && !allowed {
            break;
        }
        armed |= allowed;
        let observed_at = Instant::now();
        let position = clock.get_position().map_err(|_| ())?.0;
        let now_frequency = clock.get_frequency().map_err(|_| ())?;
        let device_played = cursor
            .observe(generation, now_frequency, position)
            .map_err(|_| ())?;
        let source_played = timeline.observe(device_played).map_err(|_| ())?;
        played.store(source_played, Ordering::Release);
        if let Ok(mut snapshot) = observation.try_lock() {
            *snapshot = Some(Observation {
                played_samples: source_played,
                observed_at,
            });
        }
        let count = (client.get_available_space_in_frames().map_err(|_| ())? as usize) & !1;
        if count > 4800 {
            return Err(());
        }
        if count == 0 {
            continue;
        }
        let mut source = 0;
        let mut samples = Vec::new();
        if allowed {
            let mut data = match source_lock(
                &buffer,
                || !stop.load(Ordering::Acquire) && lease.allowed(),
                Duration::from_millis(1),
            ) {
                Ok(data) => data,
                Err(_) if stop.load(Ordering::Acquire) || !lease.allowed() => break,
                Err(_) => {
                    eprintln!("NAI_STREAM_DEVICE_SOURCE_BUSY");
                    return Err(());
                }
            };
            let wait = if data.received() > 0 {
                first_pcm_at
                    .get_or_insert_with(std::time::Instant::now)
                    .elapsed()
            } else {
                std::time::Duration::ZERO
            };
            if !started
                && nai_media_core::stream_pcm::authorized_prefill_ready(
                    data.received(),
                    data.verified_tail(data.received()),
                    prefill,
                    wait,
                    until.load(Ordering::Acquire),
                )
            {
                started = true;
            }
            if started {
                let play_until = until.load(Ordering::Acquire);
                (source, samples) = data.take(count / 2, play_until);
                nai_media_core::stream_pcm::validate_device_pull(
                    count / 2,
                    samples.len(),
                    data.verified_tail(source + samples.len() as u64),
                )
                .map_err(|_| {
                    eprintln!(
                        "NAI_STREAM_DEVICE_UNDERRUN reason={} requested={} actual={} source_start={} received={} play_until={} submitted={} played={}",
                        nai_media_core::stream_pcm::shortfall_reason(source, count / 2, data.received(), play_until),
                        count / 2, samples.len(), source, data.received(), play_until,
                        submitted.load(Ordering::Acquire), played.load(Ordering::Acquire)
                    );
                })?;
            }
        }
        bytes[..count * 8].fill(0);
        for (index, value) in samples.iter().enumerate() {
            let sample = (*value as f32 / 32768.0).to_le_bytes();
            for frame in 0..2 {
                for channel in 0..2 {
                    let at = (index * 2 + frame) * 8 + channel * 4;
                    bytes[at..at + 4].copy_from_slice(&sample);
                }
            }
        }
        if stop.load(Ordering::Acquire) || (armed && !lease.allowed()) {
            break;
        }
        render
            .write_to_device(count, &bytes[..count * 8], None)
            .map_err(|_| ())?;
        cursor.commit(count as u64).map_err(|_| ())?;
        if !samples.is_empty() {
            timeline
                .commit(device_end, source, samples.len() as u64)
                .map_err(|_| ())?;
            submitted.store(timeline.submitted(), Ordering::Release);
        }
        device_end = device_end.checked_add(count as u64).ok_or(())?;
    }
    Ok(())
}
#[cfg(test)]
mod source_lock_tests {
    use super::*;
    #[test]
    fn brief_contention_can_acquire_but_expiry_and_revoke_are_bounded() {
        let mutex = Mutex::new(1);
        let guard = mutex.lock().unwrap();
        let began = Instant::now();
        assert!(source_lock(&mutex, || true, Duration::from_millis(1)).is_err());
        assert!(began.elapsed() < Duration::from_millis(50));
        assert!(source_lock(&mutex, || false, Duration::from_secs(1)).is_err());
        drop(guard);
        assert_eq!(
            *source_lock(&mutex, || true, Duration::from_millis(1)).unwrap(),
            1
        );
    }
}

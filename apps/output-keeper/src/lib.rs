//! Explicit headless watchdog drill. No device, PCM/frame consumer, Ready or
//! PlaybackAck producer. Parent-owned binary stdio is not production IPC auth.
use nai_contracts::v1::{
    ContractError, ErrorCode, FrameSlotEvent, OutputWatchdogCommand, OutputWatchdogStatus,
    SessionMode, output_watchdog_command::Action,
};
use nai_media_core::{
    MediaError, MediaGate,
    frame_slots::consumer::{ConsumeOutcome, FrameConsumer, FrameReader, StandbyReason},
};
use nai_plugin_sdk::frame;
use prost::Message;
use std::{
    io::{Cursor, Read, Write},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const TICK: Duration = Duration::from_millis(10);
pub const FRAME_BUDGET: Duration = Duration::from_millis(250);
pub const INPUT_CAPACITY: usize = 16;

fn failure(code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        code: code as i32,
        category: "output-watchdog".into(),
        message: message.into(),
        ..Default::default()
    }
}
fn utc_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}
fn media_error(error: MediaError) -> ContractError {
    failure(
        match error {
            MediaError::Expired => ErrorCode::DeadlineExceeded,
            MediaError::Stale => ErrorCode::StaleEpoch,
            MediaError::Capacity => ErrorCode::ResourceExhausted,
            _ => ErrorCode::PermissionDenied,
        },
        "drill lease rejected",
    )
}

/// Frame consumption and standby state. Presenting is a diagnostic observation
/// of a consumed frame under a live lease; it is never a device or ACK grant.
struct FramePath {
    reader: FrameReader,
    consumer: FrameConsumer,
    standby: Option<StandbyReason>,
    presenting: bool,
    copied: Option<nai_media_core::frame_slots::shared_ram::OwnedSlotFrame>,
    copied_sha: String,
}
impl FramePath {
    fn new(reader: FrameReader, min_epoch: u64) -> Self {
        let consumer = FrameConsumer::new(&reader.spec().session_id, min_epoch);
        Self {
            reader,
            consumer,
            standby: Some(StandbyReason::NoFrame),
            presenting: false,
            copied: None,
            copied_sha: String::new(),
        }
    }
}

pub struct Watchdog {
    session: String,
    gate: MediaGate,
    seq: u64,
    live: bool,
    input_closed: bool,
    last_error: Option<ContractError>,
    frames: Option<FramePath>,
}
impl Watchdog {
    pub fn new(session: &str, epoch_floor: u64) -> Result<Self, ContractError> {
        if session.is_empty()
            || session.len() > 128
            || !session
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(failure(ErrorCode::InvalidArgument, "invalid drill session"));
        }
        Ok(Self {
            session: session.into(),
            gate: MediaGate::with_epoch_floor(session, 16, epoch_floor),
            seq: 0,
            live: false,
            input_closed: false,
            last_error: None,
            frames: None,
        })
    }
    /// Enables the frame path only under an explicit `--frame-drill`. The reader
    /// must belong to the same session; nothing is created otherwise.
    pub fn with_frames(
        session: &str,
        epoch_floor: u64,
        reader: FrameReader,
    ) -> Result<Self, ContractError> {
        let mut watchdog = Self::new(session, epoch_floor)?;
        if reader.spec().session_id != watchdog.session {
            return Err(failure(
                ErrorCode::InvalidArgument,
                "frame pool session mismatch",
            ));
        }
        watchdog.frames = Some(FramePath::new(reader, epoch_floor));
        Ok(watchdog)
    }
    pub fn poll(&mut self, now: Instant) -> bool {
        let result = self.gate.poll(now);
        let live = result.is_ok();
        let changed = live != self.live;
        if changed && let Err(error) = result {
            self.last_error = Some(media_error(error));
        }
        self.live = live;
        let mut standby = false;
        if !live
            && let Some(frames) = self.frames.as_mut()
            && frames.presenting
        {
            frames.presenting = false;
            frames.copied = None;
            frames.standby = Some(StandbyReason::LeaseLost);
            standby = true;
        }
        changed || standby
    }
    pub fn apply(
        &mut self,
        command: OutputWatchdogCommand,
        received: Instant,
        now: Instant,
        utc: u64,
    ) -> Result<(), ContractError> {
        self.poll(now);
        let result = (|| {
            if self.input_closed {
                return Err(failure(ErrorCode::PermissionDenied, "drill control closed"));
            }
            if command.schema_version != 1
                || command.session_id != self.session
                || command.seq <= self.seq
            {
                return Err(failure(
                    ErrorCode::StaleEpoch,
                    "drill scope/version/sequence rejected",
                ));
            }
            if command.remaining_budget_ms == 0
                || command.remaining_budget_ms > 250
                || now >= received + Duration::from_millis(command.remaining_budget_ms.into())
            {
                return Err(failure(
                    ErrorCode::DeadlineExceeded,
                    "drill command expired",
                ));
            }
            match command.action {
                Some(Action::Arm(lease)) | Some(Action::Renew(lease))
                    if lease.mode != SessionMode::Rehearsal as i32 =>
                {
                    return Err(failure(
                        ErrorCode::PermissionDenied,
                        "only rehearsal leases accepted",
                    ));
                }
                Some(Action::Arm(lease)) => self.gate.arm(lease, utc, now).map_err(media_error)?,
                Some(Action::Renew(lease)) => {
                    self.gate.renew(lease, utc, now).map_err(media_error)?
                }
                Some(Action::Inspect(true)) => {}
                Some(Action::SubmitFrame(event)) => self.submit_frame(event, now)?,
                _ => return Err(failure(ErrorCode::InvalidArgument, "unknown drill action")),
            }
            self.seq = command.seq;
            self.last_error = None;
            self.live = self.gate.poll(now).is_ok();
            Ok(())
        })();
        if let Err(error) = &result {
            self.last_error = Some(error.clone());
        }
        result
    }
    /// Consumes one peer frame event under the current lease. A false lease never
    /// touches the payload; success only flips the diagnostic presenting flag.
    fn submit_frame(&mut self, event: FrameSlotEvent, now: Instant) -> Result<(), ContractError> {
        if self.frames.is_none() {
            return Err(failure(
                ErrorCode::PermissionDenied,
                "frame path not enabled",
            ));
        }
        let lease_valid = self.gate.poll(now).is_ok();
        let copied_sha = match event.event.as_ref() {
            Some(nai_contracts::v1::frame_slot_event::Event::Committed(c)) => c.sha256.clone(),
            _ => String::new(),
        };
        let scope_valid = match event.event.as_ref() {
            Some(nai_contracts::v1::frame_slot_event::Event::Committed(commit)) => commit
                .ticket
                .as_ref()
                .and_then(|t| t.frame.as_ref())
                .is_some_and(|f| self.gate.rehearsal_frame_permitted(f, now).is_ok()),
            _ => true,
        };
        let frames = self.frames.as_mut().unwrap();
        if !scope_valid {
            frames.presenting = false;
            frames.copied = None;
            frames.standby = Some(StandbyReason::StaleEpoch);
            return Ok(());
        }
        match frames
            .consumer
            .submit(event, lease_valid, &frames.reader, now)
        {
            ConsumeOutcome::Delivered(frame) => {
                if self
                    .gate
                    .rehearsal_frame_permitted(&frame.metadata, Instant::now())
                    .is_ok()
                {
                    frames.presenting = true;
                    frames.standby = None;
                    frames.copied = Some(*frame);
                    frames.copied_sha = copied_sha;
                } else {
                    frames.presenting = false;
                    frames.copied = None;
                    frames.standby = Some(StandbyReason::LeaseLost);
                }
            }
            ConsumeOutcome::Standby(reason) => {
                frames.presenting = false;
                frames.copied = None;
                frames.standby = Some(reason);
            }
        }
        Ok(())
    }
    pub fn close(&mut self, code: ErrorCode, now: Instant) {
        self.input_closed = true;
        self.gate.emergency().mute();
        self.poll(now);
        self.last_error = Some(failure(
            code,
            "drill control disconnected, malformed, overloaded or expired",
        ));
    }
    pub fn status(&self, dropped: u64, skipped: u64) -> OutputWatchdogStatus {
        let (presenting, standby_reason) = match self.frames.as_ref() {
            Some(frames) => (
                frames.presenting,
                frames
                    .standby
                    .map(|reason| reason.as_str().to_string())
                    .unwrap_or_default(),
            ),
            None => (false, StandbyReason::NotReady.as_str().to_string()),
        };
        OutputWatchdogStatus {
            schema_version: 1,
            session_id: self.session.clone(),
            last_command_seq: self.seq,
            observed_at_utc_ms: utc_ms(),
            epoch_floor: self.gate.epoch_floor(),
            lease_valid: self.live,
            input_closed: self.input_closed,
            output_available: false,
            backend_error: Some(failure(
                ErrorCode::CapabilityNotReady,
                "watchdog-drill: real output devices unavailable",
            )),
            last_control_error: self.last_error.clone(),
            dropped_commands: dropped,
            skipped_statuses: skipped,
            presenting,
            standby_reason,
            consumed_buffer_ref: self
                .frames
                .as_ref()
                .and_then(|f| f.copied.as_ref())
                .map(|f| f.metadata.buffer_ref.clone())
                .unwrap_or_default(),
            consumed_sha256: self
                .frames
                .as_ref()
                .filter(|f| f.copied.is_some())
                .map(|f| f.copied_sha.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Default)]
struct Transport {
    // 0 means idle; otherwise milliseconds since start +1 at first frame byte.
    frame_started: AtomicU64,
    failed: AtomicU64,
    dropped: AtomicU64,
}
struct Incoming {
    command: OutputWatchdogCommand,
    received: Instant,
}
#[derive(Default)]
struct Latest {
    pending: Mutex<Option<OutputWatchdogStatus>>,
    changed: Condvar,
    skipped: AtomicU64,
}
impl Latest {
    fn publish(&self, mut status: OutputWatchdogStatus) {
        if let Ok(mut pending) = self.pending.try_lock() {
            if pending.is_some() {
                self.skipped.fetch_add(1, Ordering::Relaxed);
            }
            status.skipped_statuses = self.skipped.load(Ordering::Relaxed);
            *pending = Some(status);
            self.changed.notify_one();
        } else {
            self.skipped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// One invocation per keeper process. stdin/stdout blocking happens only on
/// dedicated threads. Logical close fences late data; process exit reclaims
/// underlying blocked OS reads/writes. `stop` is for a trusted local owner.
pub fn run<R: Read + Send + 'static, W: Write + Send + 'static>(
    session: &str,
    epoch_floor: u64,
    reader: R,
    writer: W,
    stop: Arc<AtomicBool>,
) -> Result<OutputWatchdogStatus, ContractError> {
    run_with_frames(session, epoch_floor, None, reader, writer, stop)
}

/// `--frame-drill` variant: an explicit inherited pool reader enables frame
/// consumption and standby switching. Without a reader no frame path exists.
pub fn run_frame_drill<R: Read + Send + 'static, W: Write + Send + 'static>(
    session: &str,
    epoch_floor: u64,
    frame_reader: FrameReader,
    reader: R,
    writer: W,
    stop: Arc<AtomicBool>,
) -> Result<OutputWatchdogStatus, ContractError> {
    run_with_frames(
        session,
        epoch_floor,
        Some(frame_reader),
        reader,
        writer,
        stop,
    )
}

fn run_with_frames<R: Read + Send + 'static, W: Write + Send + 'static>(
    session: &str,
    epoch_floor: u64,
    frames: Option<FrameReader>,
    mut reader: R,
    mut writer: W,
    stop: Arc<AtomicBool>,
) -> Result<OutputWatchdogStatus, ContractError> {
    let mut watchdog = match frames {
        Some(frame_reader) => Watchdog::with_frames(session, epoch_floor, frame_reader)?,
        None => Watchdog::new(session, epoch_floor)?,
    };
    let transport = Arc::new(Transport::default());
    let latest = Arc::new(Latest::default());
    let start = Instant::now();
    let (sender, receiver) = mpsc::sync_channel::<Incoming>(INPUT_CAPACITY);
    {
        let transport = transport.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            let mut first = [0u8; 1];
            while !stop.load(Ordering::Acquire) && transport.failed.load(Ordering::Acquire) == 0 {
                let n = loop {
                    match reader.read(&mut first) {
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        other => break other,
                    }
                };
                if !matches!(n, Ok(1)) {
                    transport
                        .failed
                        .store(ErrorCode::Cancelled as u64, Ordering::Release);
                    break;
                }
                let received = Instant::now();
                transport
                    .frame_started
                    .store(start.elapsed().as_millis() as u64 + 1, Ordering::Release);
                let mut prefixed = Cursor::new(first).chain(&mut reader);
                let decoded = frame::read(&mut prefixed, received + FRAME_BUDGET, &stop)
                    .and_then(|body| {
                        body.ok_or_else(|| {
                            failure(ErrorCode::InvalidArgument, "missing drill frame")
                        })
                    })
                    .and_then(|body| {
                        OutputWatchdogCommand::decode(body.as_slice()).map_err(|_| {
                            failure(ErrorCode::InvalidArgument, "invalid drill protobuf")
                        })
                    });
                transport.frame_started.store(0, Ordering::Release);
                match decoded {
                    Ok(command) => {
                        if stop.load(Ordering::Acquire)
                            || transport.failed.load(Ordering::Acquire) != 0
                        {
                            break;
                        }
                        if sender.try_send(Incoming { command, received }).is_err() {
                            transport.dropped.fetch_add(1, Ordering::Relaxed);
                            transport
                                .failed
                                .store(ErrorCode::ResourceExhausted as u64, Ordering::Release);
                            break;
                        }
                    }
                    Err(error) => {
                        transport.failed.store(error.code as u64, Ordering::Release);
                        break;
                    }
                }
            }
        });
    }
    {
        let latest = latest.clone();
        let transport = transport.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                let status = {
                    let Ok(mut pending) = latest.pending.lock() else {
                        break;
                    };
                    while pending.is_none() && !stop.load(Ordering::Acquire) {
                        let Ok((next, _)) = latest
                            .changed
                            .wait_timeout(pending, Duration::from_millis(100))
                        else {
                            return;
                        };
                        pending = next;
                    }
                    pending.take()
                };
                if let Some(status) = status
                    && (frame::write(&mut writer, &status.encode_to_vec()).is_err()
                        || writer.flush().is_err())
                {
                    transport
                        .failed
                        .store(ErrorCode::Internal as u64, Ordering::Release);
                    break;
                }
            }
        });
    }
    let mut publish_at = start;
    while !stop.load(Ordering::Acquire) {
        let now = Instant::now();
        let active = transport.frame_started.load(Ordering::Acquire);
        if active != 0
            && start.elapsed().as_millis() >= u128::from(active - 1) + FRAME_BUDGET.as_millis()
        {
            transport
                .failed
                .store(ErrorCode::DeadlineExceeded as u64, Ordering::Release);
        }
        let failed = transport.failed.load(Ordering::Acquire);
        let mut changed = watchdog.poll(now);
        if failed != 0 && !watchdog.input_closed {
            watchdog.close(
                ErrorCode::try_from(failed as i32).unwrap_or(ErrorCode::Internal),
                now,
            );
            changed = true;
        }
        if !watchdog.input_closed
            && let Ok(incoming) = receiver.try_recv()
        {
            let _ = watchdog.apply(
                incoming.command,
                incoming.received,
                Instant::now(),
                utc_ms(),
            );
            changed = true;
        }
        // Recheck after dispatch and before publishing a diagnostic observation.
        changed |= watchdog.poll(Instant::now());
        if changed || now >= publish_at {
            latest.publish(watchdog.status(
                transport.dropped.load(Ordering::Relaxed),
                latest.skipped.load(Ordering::Relaxed),
            ));
            publish_at = now + Duration::from_millis(100);
        }
        thread::sleep(TICK);
    }
    watchdog.close(ErrorCode::Cancelled, Instant::now());
    latest.changed.notify_all();
    Ok(watchdog.status(
        transport.dropped.load(Ordering::Relaxed),
        latest.skipped.load(Ordering::Relaxed),
    ))
}

#[cfg(test)]
mod tests;

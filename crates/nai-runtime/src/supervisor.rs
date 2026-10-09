//! Supervised media subprocesses (keeper and render-worker).
//!
//! The binary paths are supplied explicitly by the caller: `env!("CARGO_BIN_EXE_…")`
//! is only defined inside a package's own tests, so a composition root cannot
//! locate another package's binary. Missing binaries surface as an explicit
//! error instead of a silent skip.
//!
//! Ordering invariant for faults (owned by [`crate::Composition::observe`]):
//! media mute first, then output-authority revocation, then the critical
//! decision log, then standby. A log failure never rolls back the mute.
use crate::{Result, err};
use nai_contracts::v1::ErrorCode;
use nai_contracts::v1::{
    FrameSlotEvent, OutputWatchdogCommand, OutputWatchdogStatus, PlaybackLease, SessionMode,
    frame_slot_event::Event, output_watchdog_command::Action,
};
pub use nai_media_core::frame_slots::consumer::StandbyReason;
use nai_plugin_sdk::frame;
use prost::Message;
use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const READ_BUDGET: Duration = Duration::from_secs(10);
const REAP_TIMEOUT: Duration = Duration::from_secs(2);

/// Explicitly supplied drills. The keeper accepts only rehearsal leases.
#[derive(Debug, Clone)]
pub struct Binaries {
    pub keeper: PathBuf,
    pub render_worker: PathBuf,
}
impl Binaries {
    pub fn validate(&self) -> Result<()> {
        for path in [&self.keeper, &self.render_worker] {
            if !path.is_file() {
                return Err(err(
                    ErrorCode::CapabilityNotReady,
                    format!("required media binary is missing: {}", path.display()),
                ));
            }
        }
        Ok(())
    }
}

/// Optional inherited frame pool for the keeper's `--frame-drill` mode. The FD is
/// handed to the child through stderr, exactly like the keeper's own drill.
pub struct FramePoolArgs {
    pub epoch: u64,
    pub broker: Option<PathBuf>,
    pub pool_id: String,
    pub producer: String,
    pub consumer: String,
    pub slots: u32,
    pub slot_bytes: u32,
    pub file: File,
}

/// Inputs for one `--render-drill` render-worker invocation. PCM is raw bytes and
/// the ticket is a protobuf file; frame data never crosses a JSON/Base64 bus.
#[derive(Debug, Clone)]
pub struct RenderWorkerSpec {
    pub broker: Option<PathBuf>,
    pub finish: Option<PathBuf>,
    pub session: String,
    pub epoch: u64,
    pub pcm: PathBuf,
    pub ticket: PathBuf,
    pub manifest: PathBuf,
    pub config: PathBuf,
    pub assets: Vec<(String, PathBuf)>,
    pub licenses: Vec<String>,
    /// Optional warm-up marker: written once the worker has admitted the avatar
    /// and is waiting for the reserved ticket, so the root opens the
    /// non-renewable slot reserve window only after the admit cost is paid.
    pub ready: Option<PathBuf>,
}

/// Bounded description of a supervised-child fault, before any side effect.
#[derive(Debug)]
pub struct SupervisorFault {
    pub child: &'static str,
    pub exit: Option<ExitStatus>,
    pub reason: StandbyReason,
    pub detail: String,
}
impl SupervisorFault {
    pub fn exit_text(&self) -> Option<String> {
        self.exit.map(exit_text)
    }
}

fn exit_text(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("signal:{signal}");
        }
    }
    match status.code() {
        Some(code) => format!("exit:{code}"),
        None => "unknown".into(),
    }
}

struct Keeper {
    child: Child,
    commands: SyncSender<Vec<u8>>,
    writer: Option<JoinHandle<()>>,
    status: Arc<Mutex<Option<OutputWatchdogStatus>>>,
    read_failed: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

struct Worker {
    child: Child,
    events: Receiver<Result<FrameSlotEvent>>,
    reader: Option<JoinHandle<()>>,
    committed: bool,
}

/// Owns supervised subprocess handles and their bounded transport. It holds no
/// media authority: muting and revocation belong to the composition root.
pub struct Supervisor {
    binaries: Binaries,
    keeper: Option<Keeper>,
    worker: Option<Worker>,
    seq: u64,
    standby: Option<StandbyReason>,
    last_status: Option<OutputWatchdogStatus>,
    faulted: bool,
}

impl Supervisor {
    pub fn new(binaries: Binaries) -> Result<Self> {
        binaries.validate()?;
        Ok(Self {
            binaries,
            keeper: None,
            worker: None,
            seq: 0,
            standby: None,
            last_status: None,
            faulted: false,
        })
    }

    pub fn standby(&self) -> Option<StandbyReason> {
        self.standby
    }

    pub(crate) fn set_standby(&mut self, reason: StandbyReason) {
        self.standby = Some(reason);
        self.faulted = true;
    }

    pub fn last_status(&self) -> Option<&OutputWatchdogStatus> {
        self.last_status.as_ref()
    }

    pub fn keeper_pid(&mut self) -> Option<u32> {
        self.keeper
            .as_mut()
            .and_then(|keeper| keeper.child.id().into())
    }

    /// Spawns the keeper drill. `frame_pool`, when present, enables `--frame-drill`
    /// and delivers the pool FD through stderr (the data channel stays binary).
    pub fn spawn_keeper(
        &mut self,
        session: &str,
        epoch_floor: u64,
        frame_pool: Option<FramePoolArgs>,
    ) -> Result<()> {
        if self.keeper.is_some() {
            return Err(err(ErrorCode::Conflict, "keeper already supervised"));
        }
        if self.faulted {
            return Err(err(
                ErrorCode::PermissionDenied,
                "supervisor is on standby and refuses new delivery",
            ));
        }
        let mut command = Command::new(&self.binaries.keeper);
        command
            .arg("--watchdog-drill")
            .arg(session)
            .arg(epoch_floor.to_string());
        let stderr = match frame_pool {
            Some(FramePoolArgs {
                epoch,
                broker,
                pool_id,
                producer,
                consumer,
                slots,
                slot_bytes,
                file,
            }) => {
                command
                    .arg("--frame-drill")
                    .arg(pool_id)
                    .arg(producer)
                    .arg(consumer)
                    .arg(slots.to_string())
                    .arg(slot_bytes.to_string());
                command.arg("--frame-epoch").arg(epoch.to_string());
                if let Some(path) = broker {
                    command.arg("--broker").arg(path);
                }
                Stdio::from(file)
            }
            None => Stdio::null(),
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .map_err(|error| {
                err(
                    ErrorCode::CapabilityNotReady,
                    format!("keeper spawn failed: {error}"),
                )
            })?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| err(ErrorCode::Internal, "keeper stdin unavailable"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| err(ErrorCode::Internal, "keeper stdout unavailable"))?;
        let status = Arc::new(Mutex::new(None));
        let read_failed = Arc::new(AtomicBool::new(false));
        let (commands, command_rx) = mpsc::sync_channel::<Vec<u8>>(16);
        let writer = {
            let failed = read_failed.clone();
            thread::spawn(move || {
                while let Ok(bytes) = command_rx.recv() {
                    if stdin
                        .write_all(&bytes)
                        .and_then(|()| stdin.flush())
                        .is_err()
                    {
                        failed.store(true, Ordering::Release);
                        break;
                    }
                }
            })
        };
        let reader = {
            let status = status.clone();
            let read_failed = read_failed.clone();
            thread::spawn(move || {
                let cancelled = AtomicBool::new(false);
                loop {
                    match frame::read(&mut stdout, Instant::now() + READ_BUDGET, &cancelled) {
                        Ok(None) => break,
                        Ok(Some(body)) => match OutputWatchdogStatus::decode(body.as_slice()) {
                            Ok(decoded) => {
                                if let Ok(mut slot) = status.lock() {
                                    *slot = Some(decoded);
                                }
                            }
                            Err(_) => {
                                read_failed.store(true, Ordering::Release);
                                break;
                            }
                        },
                        Err(_) => {
                            read_failed.store(true, Ordering::Release);
                            break;
                        }
                    }
                }
            })
        };
        self.keeper = Some(Keeper {
            child,
            commands,
            writer: Some(writer),
            status,
            read_failed,
            reader: Some(reader),
        });
        Ok(())
    }

    /// Sends one arm command with a strictly increasing sequence. Only rehearsal
    /// leases are accepted, matching the keeper's own constraint.
    pub fn arm(&mut self, session: &str, lease: PlaybackLease) -> Result<()> {
        if lease.mode != SessionMode::Rehearsal as i32 {
            return Err(err(
                ErrorCode::PermissionDenied,
                "only rehearsal leases are accepted by the drill",
            ));
        }
        self.dispatch(session, Action::Arm(lease))
    }

    pub fn renew(&mut self, session: &str, lease: PlaybackLease) -> Result<()> {
        if lease.mode != SessionMode::Rehearsal as i32 {
            return Err(err(
                ErrorCode::PermissionDenied,
                "only rehearsal leases are accepted by the drill",
            ));
        }
        self.dispatch(session, Action::Renew(lease))
    }
    pub fn submit_frame(&mut self, session: &str, event: FrameSlotEvent) -> Result<u64> {
        self.dispatch(session, Action::SubmitFrame(event))?;
        Ok(self.seq)
    }
    pub fn status(&mut self) -> Option<OutputWatchdogStatus> {
        let status = self.latest();
        if let Some(s) = &status {
            self.last_status = Some(s.clone());
        }
        status
    }
    pub fn worker_pid(&self) -> Option<u32> {
        self.worker.as_ref().map(|w| w.child.id())
    }

    fn dispatch(&mut self, session: &str, action: Action) -> Result<()> {
        if self.standby.is_some() {
            return Err(err(
                ErrorCode::PermissionDenied,
                "supervisor is on standby and refuses further delivery",
            ));
        }
        let next = self.seq.checked_add(1).ok_or_else(|| {
            err(
                ErrorCode::ResourceExhausted,
                "supervisor sequence exhausted",
            )
        })?;
        let command = OutputWatchdogCommand {
            schema_version: 1,
            session_id: session.into(),
            seq: next,
            remaining_budget_ms: 250,
            action: Some(action),
        };
        let keeper = self
            .keeper
            .as_mut()
            .ok_or_else(|| err(ErrorCode::CapabilityNotReady, "no supervised keeper"))?;
        let bytes = frame::encode(&command.encode_to_vec()).map_err(crate::RuntimeError::from)?;
        keeper.commands.try_send(bytes).map_err(|error| {
            err(
                ErrorCode::Internal,
                format!("keeper control write failed: {error}"),
            )
        })?;
        self.seq = next;
        Ok(())
    }

    /// Blocks until the next keeper status arrives or the deadline passes.
    pub fn poll(&mut self, deadline: Instant) -> Result<OutputWatchdogStatus> {
        self.wait_until(deadline, |_| true)
    }

    pub fn wait_until(
        &mut self,
        deadline: Instant,
        predicate: impl Fn(&OutputWatchdogStatus) -> bool,
    ) -> Result<OutputWatchdogStatus> {
        loop {
            if let Some(status) = self.latest() {
                self.last_status = Some(status.clone());
                if predicate(&status) {
                    return Ok(status);
                }
            }
            if self
                .keeper
                .as_ref()
                .is_some_and(|keeper| keeper.read_failed.load(Ordering::Acquire))
            {
                return Err(err(
                    ErrorCode::Internal,
                    "keeper status channel failed before a matching observation",
                ));
            }
            if Instant::now() >= deadline {
                return Err(err(
                    ErrorCode::DeadlineExceeded,
                    "no matching keeper status before the deadline",
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn latest(&self) -> Option<OutputWatchdogStatus> {
        self.keeper
            .as_ref()
            .and_then(|keeper| keeper.status.lock().ok().and_then(|slot| slot.clone()))
    }

    /// Spawns the render-worker drill. The pool FD is inherited through stderr and
    /// stdin is closed; frame data flows only through the pool and stdout events.
    pub fn spawn_worker(&mut self, spec: &RenderWorkerSpec, pool_file: File) -> Result<()> {
        if self.worker.is_some() {
            return Err(err(ErrorCode::Conflict, "render-worker already supervised"));
        }
        if self.faulted {
            return Err(err(
                ErrorCode::PermissionDenied,
                "supervisor is on standby and refuses new delivery",
            ));
        }
        let mut command = Command::new(&self.binaries.render_worker);
        command
            .arg("--render-drill")
            .arg("--pcm")
            .arg(&spec.pcm)
            .arg("--ticket")
            .arg(&spec.ticket)
            .arg("--epoch")
            .arg(spec.epoch.to_string())
            .arg("--avatar")
            .arg(&spec.manifest)
            .arg("--avatar-config")
            .arg(&spec.config);
        for (id, path) in &spec.assets {
            command
                .arg("--asset")
                .arg(format!("{id}={}", path.display()));
        }
        for license in &spec.licenses {
            command.arg("--license").arg(license);
        }
        if let Some(ready) = &spec.ready {
            command.arg("--ready").arg(ready);
        }
        if let Some(path) = &spec.broker {
            command.arg("--broker").arg(path);
        }
        if let Some(path) = &spec.finish {
            command.arg("--finish").arg(path);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(pool_file))
            .spawn()
            .map_err(|error| {
                err(
                    ErrorCode::CapabilityNotReady,
                    format!("render-worker spawn failed: {error}"),
                )
            })?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| err(ErrorCode::Internal, "render-worker stdout unavailable"))?;
        let (sender, events) = mpsc::sync_channel(2);
        let reader = thread::spawn(move || {
            let cancelled = AtomicBool::new(false);
            loop {
                let result = frame::read(&mut stdout, Instant::now() + READ_BUDGET, &cancelled)
                    .map_err(crate::RuntimeError::from)
                    .and_then(|bytes| {
                        bytes.ok_or_else(|| err(ErrorCode::Internal, "worker event EOF"))
                    })
                    .and_then(|bytes| {
                        FrameSlotEvent::decode(bytes.as_slice())
                            .map_err(|_| err(ErrorCode::Internal, "malformed worker event"))
                    });
                let terminal = result.is_err();
                if sender.send(result).is_err() || terminal {
                    break;
                }
            }
        });
        self.worker = Some(Worker {
            child,
            events,
            reader: Some(reader),
            committed: false,
        });
        Ok(())
    }

    pub fn read_worker_event(&mut self, deadline: Instant) -> Result<FrameSlotEvent> {
        loop {
            if let Some(event) = self.try_worker_event()? {
                return Ok(event);
            }
            if Instant::now() >= deadline {
                return Err(err(ErrorCode::DeadlineExceeded, "worker event deadline"));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }
    pub fn try_worker_event(&mut self) -> Result<Option<FrameSlotEvent>> {
        let worker = self
            .worker
            .as_mut()
            .ok_or_else(|| err(ErrorCode::CapabilityNotReady, "no supervised render-worker"))?;
        let event = match worker.events.try_recv() {
            Ok(result) => result?,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => {
                return Err(err(ErrorCode::Internal, "worker event channel closed"));
            }
        };
        if matches!(event.event, Some(Event::Committed(_))) {
            worker.committed = true;
        }
        Ok(Some(event))
    }

    pub fn wait_worker(&mut self, timeout: Duration) -> Result<ExitStatus> {
        let worker = self
            .worker
            .as_mut()
            .ok_or_else(|| err(ErrorCode::CapabilityNotReady, "no supervised render-worker"))?;
        reap(&mut worker.child, timeout)
    }

    /// Non-blocking fault detection. Returns the first observed child fault
    /// without side effects; [`crate::Composition::observe`] performs the halt.
    pub fn detect_fault(&mut self) -> Option<SupervisorFault> {
        if let Some(keeper) = self.keeper.as_mut() {
            match keeper.child.try_wait() {
                Ok(Some(status)) => {
                    return Some(SupervisorFault {
                        child: "keeper",
                        exit: Some(status),
                        reason: StandbyReason::Retired,
                        detail: "keeper process exited".into(),
                    });
                }
                Ok(None) => {}
                Err(_) => {
                    return Some(SupervisorFault {
                        child: "keeper",
                        exit: None,
                        reason: StandbyReason::Timeout,
                        detail: "keeper wait failed".into(),
                    });
                }
            }
            if keeper.read_failed.load(Ordering::Acquire) {
                return Some(SupervisorFault {
                    child: "keeper",
                    exit: None,
                    reason: StandbyReason::Timeout,
                    detail: "keeper status channel failed".into(),
                });
            }
        }
        if let Some(worker) = self.worker.as_mut()
            && !worker.committed
            && let Ok(Some(status)) = worker.child.try_wait()
        {
            return Some(SupervisorFault {
                child: "render-worker",
                exit: Some(status),
                reason: StandbyReason::Retired,
                detail: "render-worker process exited before a commit".into(),
            });
        }
        None
    }

    /// Kills the supervised keeper (owner/test controlled). It does not itself
    /// revoke output authority; the caller must call `Composition::observe`.
    pub fn kill_keeper(&mut self) -> Result<()> {
        let keeper = self
            .keeper
            .as_mut()
            .ok_or_else(|| err(ErrorCode::CapabilityNotReady, "no supervised keeper"))?;
        keeper
            .child
            .kill()
            .map_err(|error| err(ErrorCode::Internal, format!("keeper kill failed: {error}")))
    }

    /// Bounded kill + reap of every supervised child (no zombies, no leaked FDs).
    pub fn shutdown(&mut self) -> Result<()> {
        if let Some(mut worker) = self.worker.take() {
            let _ = worker.child.kill();
            let _ = reap(&mut worker.child, REAP_TIMEOUT);
            drop(worker.events);
            if let Some(reader) = worker.reader.take() {
                join_bounded(reader)?;
            }
        }
        if let Some(mut keeper) = self.keeper.take() {
            let _ = keeper.child.kill();
            let _ = reap(&mut keeper.child, REAP_TIMEOUT);
            drop(keeper.commands);
            if let Some(writer) = keeper.writer.take() {
                join_bounded(writer)?;
            }
            if let Some(reader) = keeper.reader.take() {
                join_bounded(reader)?;
            }
        }
        Ok(())
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
fn join_bounded(handle: JoinHandle<()>) -> Result<()> {
    let deadline = Instant::now() + REAP_TIMEOUT;
    while !handle.is_finished() {
        if Instant::now() >= deadline {
            return Err(err(
                ErrorCode::DeadlineExceeded,
                "media transport thread did not close",
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
    handle
        .join()
        .map_err(|_| err(ErrorCode::Internal, "media transport thread panicked"))
}

fn reap(child: &mut Child, timeout: Duration) -> Result<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                return Err(err(
                    ErrorCode::Internal,
                    format!("child wait failed: {error}"),
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err(err(
                ErrorCode::DeadlineExceeded,
                "child did not exit in time",
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

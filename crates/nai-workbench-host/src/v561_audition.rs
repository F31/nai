//! Narrow child supervisor. No model, database, approval or audio implementation.
use nai_contracts::v561_audition::{Command, Event, Kind, Request};
use prost::Message;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

struct Inner {
    engine: PathBuf,
    audio: PathBuf,
    config: Mutex<Option<PathBuf>>,
    view: Mutex<Event>,
    busy: AtomicBool,
    nonce: AtomicU64,
    run_nonce: AtomicU64,
    id: AtomicU64,
    priority: Mutex<Option<(u64, String)>>,
}
pub struct Controller {
    inner: Arc<Inner>,
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.stop();
    }
}
impl Controller {
    pub fn new(engine: PathBuf, audio: PathBuf, config: Option<PathBuf>) -> Result<Self, String> {
        for (path, name) in [
            (&engine, "nai-v561-audition-control"),
            (&audio, "nai-v561-stream-audio-output"),
        ] {
            if !path.is_absolute()
                || !path.is_file()
                || path.file_name().and_then(|n| n.to_str())
                    != Some(&format!("{name}{}", std::env::consts::EXE_SUFFIX))
            {
                return Err("PROGRAMS_MISSING".into());
            }
        }
        let value = Self {
            inner: Arc::new(Inner {
                engine,
                audio,
                config: Mutex::new(None),
                view: Mutex::new(Event {
                    version: 1,
                    kind: Kind::Idle as i32,
                    ..Default::default()
                }),
                busy: AtomicBool::new(false),
                nonce: AtomicU64::new(1),
                run_nonce: AtomicU64::new(1),
                id: AtomicU64::new(0),
                priority: Mutex::new(None),
            }),
        };
        if let Some(config) = config {
            value.select_config(config)?;
        }
        Ok(value)
    }
    pub fn select_config(&self, path: PathBuf) -> Result<(), String> {
        if !path.is_absolute()
            || !path.is_file()
            || path.metadata().map_err(|_| "CONFIG_IO")?.len() > 32768
        {
            return Err("CONFIG_INVALID".into());
        }
        let mut selected = self.inner.config.lock().map_err(|_| "STATE_FAILED")?;
        if self.inner.busy.load(Ordering::Acquire) {
            return Err("BUSY".into());
        }
        *selected = Some(path);
        Ok(())
    }
    pub fn stop(&self) {
        let _ = self
            .inner
            .nonce
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1));
    }
    pub fn interject(&self, expected_task: u64, text: String) -> Result<(), String> {
        if text.trim().is_empty() || text.chars().count() > 1024 {
            return Err("TEXT_INVALID".into());
        }
        let id = self.inner.id.load(Ordering::Acquire);
        if id != expected_task {
            return Err("NOT_RUNNING".into());
        }
        if !self.inner.busy.load(Ordering::Acquire)
            || self.inner.run_nonce.load(Ordering::Acquire)
                != self.inner.nonce.load(Ordering::Acquire)
        {
            return Err("NOT_RUNNING".into());
        }
        let mut slot = self.inner.priority.try_lock().map_err(|_| "BUSY")?;
        if !self.inner.busy.load(Ordering::Acquire)
            || self.inner.id.load(Ordering::Acquire) != id
            || self.inner.run_nonce.load(Ordering::Acquire)
                != self.inner.nonce.load(Ordering::Acquire)
        {
            return Err("NOT_RUNNING".into());
        }
        if slot.is_some() {
            return Err("BUSY".into());
        }
        *slot = Some((id, text));
        Ok(())
    }
    pub fn status(&self) -> Event {
        let mut view = self.inner.view.lock().map(|v| v.clone()).unwrap_or(Event {
            version: 1,
            kind: Kind::Failed as i32,
            code: "STATE_FAILED".into(),
            ..Default::default()
        });
        view.busy = self.inner.busy.load(Ordering::Acquire);
        if self.inner.run_nonce.load(Ordering::Acquire) != self.inner.nonce.load(Ordering::Acquire)
        {
            view.kind = if view.busy {
                Kind::CancelRequested
            } else {
                Kind::Cancelled
            } as i32;
            view.code = if view.busy {
                "STOP_REQUESTED"
            } else {
                "SOFTWARE_STOPPED"
            }
            .into();
        }
        if let Ok(selected) = self.inner.config.lock() {
            view.config_selected = selected.is_some();
            view.config_label = selected
                .as_deref()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().chars().take(128).collect())
                .unwrap_or_default();
        }
        view
    }
    pub fn start(&self, text: String) -> Result<(), String> {
        if text.trim().is_empty() || text.chars().count() > 1024 {
            return Err("TEXT_INVALID".into());
        }
        let selected = self.inner.config.lock().map_err(|_| "STATE_FAILED")?;
        let config = selected.clone().ok_or("CONFIG_REQUIRED")?;
        let nonce = self.inner.nonce.load(Ordering::Acquire);
        if nonce == u64::MAX {
            return Err("TERMINAL".into());
        }
        self.inner
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "BUSY")?;
        drop(selected);
        let id = match self
            .inner
            .id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
        {
            Ok(id) => id + 1,
            Err(_) => {
                self.inner.busy.store(false, Ordering::Release);
                return Err("TERMINAL".into());
            }
        };
        self.inner.run_nonce.store(nonce, Ordering::Release);
        if let Ok(mut view) = self.inner.view.lock() {
            *view = Event {
                version: 1,
                task_id: id,
                kind: Kind::Starting as i32,
                code: "STARTING".into(),
                busy: true,
                ..Default::default()
            };
        }
        let inner = self.inner.clone();
        std::thread::spawn(move || supervise(inner, config, text, nonce, id));
        Ok(())
    }
}
fn valid(event: &Event) -> bool {
    event.version == 1
        && event.task_id == 1
        && (1..=64).contains(&event.request_sequence)
        && event.code.len() <= 64
        && event
            .code
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
        && event.samples.is_none_or(|n| n <= 480000)
        && event
            .played_samples
            .is_none_or(|n| event.samples.is_some_and(|total| n <= total))
        && matches!(
            Kind::try_from(event.kind),
            Ok(Kind::Running
                | Kind::Completed
                | Kind::CancelRequested
                | Kind::Cancelled
                | Kind::Failed)
        )
        && (event.kind != Kind::Completed as i32
            || event
                .samples
                .is_some_and(|n| n > 0 && event.played_samples == Some(n)))
}
fn packet(input: &mut impl Read) -> Result<Event, ()> {
    let mut n = [0; 4];
    input.read_exact(&mut n).map_err(|_| ())?;
    let n = u32::from_le_bytes(n) as usize;
    if n == 0 || n > 4096 {
        return Err(());
    }
    let mut bytes = vec![0; n];
    input.read_exact(&mut bytes).map_err(|_| ())?;
    let event = Event::decode(bytes.as_slice()).map_err(|_| ())?;
    if !valid(&event) {
        return Err(());
    }
    Ok(event)
}
struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn supervise(inner: Arc<Inner>, config: PathBuf, text: String, nonce: u64, id: u64) {
    let result = (|| -> Result<Event, &'static str> {
        let mut command = std::process::Command::new(&inner.engine);
        command.env_clear();
        for key in ["SystemRoot", "WINDIR"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command
            .arg("--config")
            .arg(config)
            .arg("--audio-output")
            .arg(&inner.audio)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = Reap(command.spawn().map_err(|_| "SPAWN_FAILED")?);
        let mut stdin = child.0.stdin.take().ok_or("IPC_FAILED")?;
        let mut stdout = child.0.stdout.take().ok_or("IPC_FAILED")?;
        let (events_tx, events) = mpsc::sync_channel(8);
        let (write_tx, writes) = mpsc::sync_channel::<Request>(2);
        let reader = std::thread::spawn(move || {
            loop {
                let event = packet(&mut stdout);
                let end = event.is_err();
                if events_tx.send(event).is_err() || end {
                    break;
                }
            }
        });
        let writer = std::thread::spawn(move || {
            for request in writes {
                let bytes = request.encode_to_vec();
                if stdin
                    .write_all(&(bytes.len() as u32).to_le_bytes())
                    .and_then(|_| stdin.write_all(&bytes))
                    .and_then(|_| stdin.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        let start_sent = inner.nonce.load(Ordering::Acquire) == nonce;
        if start_sent {
            write_tx
                .try_send(Request {
                    version: 1,
                    sequence: 1,
                    command: Command::Start as i32,
                    text,
                })
                .map_err(|_| "IPC_FAILED")?;
        }
        let mut last = None;
        let mut command_sequence = if start_sent { 1 } else { 0 };
        let mut stopping = None;
        let mut sent_stop = false;
        let mut faulty = false;
        let mut killed = false;
        let exit = loop {
            for _ in 0..8 {
                match events.try_recv() {
                    Ok(Ok(event)) => {
                        if inner.nonce.load(Ordering::Acquire) == nonce
                            && let Ok(mut view) = inner.view.lock()
                        {
                            *view = Event {
                                task_id: id,
                                ..event.clone()
                            };
                        }
                        last = Some(event);
                    }
                    Ok(Err(())) => {
                        faulty |= last.as_ref().is_none_or(|e| {
                            !matches!(
                                Kind::try_from(e.kind),
                                Ok(Kind::Completed | Kind::Cancelled | Kind::Failed)
                            )
                        });
                    }
                    Err(_) => break,
                }
            }
            if (inner.nonce.load(Ordering::Acquire) != nonce || faulty) && stopping.is_none() {
                stopping = Some(Instant::now());
            }
            if stopping.is_some() && !sent_stop {
                sent_stop = write_tx
                    .try_send(Request {
                        version: 1,
                        sequence: command_sequence + 1,
                        command: Command::Stop as i32,
                        text: String::new(),
                    })
                    .is_ok();
                if sent_stop {
                    command_sequence += 1;
                }
            }
            if stopping.is_none() && command_sequence < 63 {
                let next = inner
                    .priority
                    .try_lock()
                    .ok()
                    .and_then(|mut slot| slot.take());
                if let Some((owner, text)) = next
                    && owner == id
                {
                    let request = Request {
                        version: 1,
                        sequence: command_sequence + 1,
                        command: Command::Interject as i32,
                        text,
                    };
                    match write_tx.try_send(request) {
                        Ok(()) => command_sequence += 1,
                        Err(mpsc::TrySendError::Full(request)) => {
                            if let Ok(mut slot) = inner.priority.try_lock()
                                && slot.is_none()
                            {
                                *slot = Some((id, request.text));
                            }
                        }
                        Err(_) => faulty = true,
                    }
                }
            }
            if let Some(exit) = child.0.try_wait().map_err(|_| "PROCESS_FAILED")? {
                break exit;
            }
            if stopping.is_some_and(|at: Instant| at.elapsed() >= Duration::from_millis(500)) {
                let _ = child.0.kill();
                killed = true;
                break child.0.wait().map_err(|_| "PROCESS_FAILED")?;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        drop(write_tx);
        let deadline = Instant::now() + Duration::from_millis(100);
        while (!reader.is_finished() || !writer.is_finished()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        if reader.is_finished() {
            let _ = reader.join();
        }
        if writer.is_finished() {
            let _ = writer.join();
        }
        while let Ok(Ok(event)) = events.try_recv() {
            last = Some(event);
        }
        if killed || !exit.success() {
            std::thread::sleep(Duration::from_millis(750));
        }
        if inner.nonce.load(Ordering::Acquire) != nonce {
            return Ok(Event {
                version: 1,
                task_id: id,
                kind: Kind::Cancelled as i32,
                code: "SOFTWARE_STOPPED".into(),
                ..Default::default()
            });
        }
        if !exit.success() {
            return Err("PROCESS_FAILED");
        }
        let mut event = last.ok_or("IPC_INCOMPLETE")?;
        if !matches!(
            Kind::try_from(event.kind),
            Ok(Kind::Completed | Kind::Cancelled | Kind::Failed)
        ) {
            return Err("IPC_INCOMPLETE");
        }
        event.task_id = id;
        event.busy = false;
        Ok(event)
    })();
    let view = result.unwrap_or_else(|code| Event {
        version: 1,
        task_id: id,
        kind: Kind::Failed as i32,
        code: code.into(),
        ..Default::default()
    });
    if let Ok(mut cached) = inner.view.lock() {
        *cached = view;
    }
    inner.busy.store(false, Ordering::Release);
    if let Ok(mut slot) = inner.priority.lock()
        && slot.as_ref().is_some_and(|(owner, _)| *owner == id)
    {
        *slot = None;
    }
}

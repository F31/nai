//! Persistent private session supervisor; no model or audio implementation.
use nai_contracts::{
    v561_live::{Command, Event, Kind, ReplyText, Request},
    v561_live_validation,
};
use prost::Message;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
struct Inner {
    view: Mutex<Event>,
    pending: AtomicBool,
    stop: AtomicBool,
    shutdown: AtomicBool,
    terminated: AtomicBool,
    actions: mpsc::SyncSender<(Command, String, Option<ReplyText>)>,
}
pub struct Controller(Arc<Inner>);
impl Drop for Controller {
    fn drop(&mut self) {
        self.0.shutdown.store(true, Ordering::Release);
    }
}
impl Controller {
    pub fn new(
        engine: PathBuf,
        audio: PathBuf,
        profile: PathBuf,
        artifacts: PathBuf,
    ) -> Result<Self, String> {
        for (p, name) in [
            (&engine, "v561_live_control.exe"),
            (&audio, "nai-v561-stream-audio-output.exe"),
        ] {
            if !p.is_absolute()
                || !p.is_file()
                || p.file_name().and_then(|n| n.to_str()) != Some(name)
            {
                return Err("PROGRAMS_MISSING".into());
            }
        }
        if !profile.is_absolute()
            || !profile.is_file()
            || profile.metadata().map_err(|_| "CONFIG_INVALID")?.len() > 32768
            || !artifacts.is_absolute()
        {
            return Err("CONFIG_INVALID".into());
        }
        let (tx, rx) = mpsc::sync_channel(2);
        let inner = Arc::new(Inner {
            view: Mutex::new(Event {
                version: 1,
                kind: Kind::Unprepared as i32,
                ..Default::default()
            }),
            pending: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            terminated: AtomicBool::new(false),
            actions: tx,
        });
        let run = inner.clone();
        std::thread::spawn(move || supervise(run, rx, engine, audio, profile, artifacts));
        Ok(Self(inner))
    }
    pub fn status(&self) -> Event {
        let mut v = self.0.view.lock().map(|v| v.clone()).unwrap_or_default();
        v.busy |= self.0.pending.load(Ordering::Acquire);
        v
    }
    pub fn prepare(&self) -> Result<(), String> {
        self.send(Command::Prepare, String::new())
    }
    pub fn start(&self, text: String) -> Result<(), String> {
        let status = self.status();
        if !status.visual_prepared || !status.tts_prepared {
            return Err("NOT_PREPARED".into());
        }
        if text.trim().is_empty() || text.chars().count() > 1024 || text.len() > 4096 {
            return Err("TEXT_INVALID".into());
        }
        self.send(Command::Start, text)
    }
    pub fn stop(&self) {
        self.0.stop.store(true, Ordering::Release);
    }
    /// Executes explicitly supplied text; does not call an LLM.
    pub fn reply_text(&self, question: String, answer: String) -> Result<(), String> {
        self.interject(
            Command::ReplyText,
            String::new(),
            Some(ReplyText { question, answer }),
        )
    }
    pub fn ask(&self, question: String) -> Result<(), String> {
        self.interject(Command::Ask, question, None)
    }
    fn interject(
        &self,
        command: Command,
        text: String,
        reply: Option<ReplyText>,
    ) -> Result<(), String> {
        let view = self.status();
        if !view.busy || view.code != "PLAYING_PREPARED" || !view.content_prepared {
            return Err("REPLY_NOT_AVAILABLE".into());
        }
        let request = Request {
            version: 1,
            sequence: 1,
            command: command as i32,
            text: text.clone(),
            reply: reply.clone(),
        };
        if !v561_live_validation::request(&request, 0) {
            return Err("TEXT_INVALID".into());
        }
        if self
            .0
            .pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("BUSY".into());
        }
        if self.0.actions.try_send((command, text, reply)).is_err() {
            self.0.pending.store(false, Ordering::Release);
            return Err("IPC_FAILED".into());
        }
        Ok(())
    }
    pub fn prepare_content(&self, text: String) -> Result<(), String> {
        self.content_action(Command::PrepareContent, text)
    }
    pub fn play_prepared(&self, text: String) -> Result<(), String> {
        if !self.status().content_prepared {
            return Err("CONTENT_NOT_PREPARED".into());
        }
        self.content_action(Command::PlayPrepared, text)
    }
    pub fn play_prepared_obs(&self, text: String) -> Result<(), String> {
        let status = self.status();
        if status.obs_port.is_none() {
            return Err("OBS_NOT_CONFIGURED".into());
        }
        if !status.content_prepared {
            return Err("CONTENT_NOT_PREPARED".into());
        }
        self.content_action(Command::PlayPreparedObs, text)
    }
    fn content_action(&self, command: Command, text: String) -> Result<(), String> {
        let state = self.status();
        if !state.visual_prepared || !state.tts_prepared {
            return Err("NOT_PREPARED".into());
        }
        if text.trim().is_empty() || text.chars().count() > 1024 || text.len() > 4096 {
            return Err("TEXT_INVALID".into());
        }
        self.send(command, text)
    }
    pub fn shutdown_wait(&self) -> Result<(), String> {
        self.0.shutdown.store(true, Ordering::Release);
        let began = Instant::now();
        while !self.0.terminated.load(Ordering::Acquire) {
            if began.elapsed() > Duration::from_secs(5) {
                return Err("SESSION_EXITED".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
    fn send(&self, command: Command, text: String) -> Result<(), String> {
        if self.status().busy
            || self
                .0
                .pending
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err("BUSY".into());
        }
        if self.0.actions.try_send((command, text, None)).is_err() {
            self.0.pending.store(false, Ordering::Release);
            return Err("IPC_FAILED".into());
        }
        Ok(())
    }
}
fn packet(input: &mut impl Read, last: u64) -> Result<Event, ()> {
    let mut prefix = [0; 4];
    input.read_exact(&mut prefix).map_err(|_| ())?;
    let n = u32::from_le_bytes(prefix) as usize;
    if n == 0 || n > 4096 {
        return Err(());
    }
    let mut bytes = vec![0; n];
    input.read_exact(&mut bytes).map_err(|_| ())?;
    let e = Event::decode(bytes.as_slice()).map_err(|_| ())?;
    if !v561_live_validation::event(&e, last) {
        return Err(());
    }
    Ok(e)
}
struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn supervise(
    inner: Arc<Inner>,
    actions: mpsc::Receiver<(Command, String, Option<ReplyText>)>,
    engine: PathBuf,
    audio: PathBuf,
    profile: PathBuf,
    artifacts: PathBuf,
) {
    let result = (|| -> Result<(), ()> {
        let mut command = std::process::Command::new(engine);
        command.env_clear();
        for key in [
            "SystemRoot",
            "WINDIR",
            "PATH",
            "INCLUDE",
            "LIB",
            "LIBPATH",
            "CUDA_PATH",
            "CUDA_HOME",
        ] {
            if let Some(v) = std::env::var_os(key) {
                command.env(key, v);
            }
        }
        command
            .arg(profile)
            .arg(audio)
            .arg(&artifacts)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr({
                std::fs::create_dir_all(&artifacts).map_err(|_| ())?;
                Stdio::from(
                    std::fs::File::create_new(artifacts.join("session.stderr.log"))
                        .map_err(|_| ())?,
                )
            });
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = Reap(command.spawn().map_err(|_| ())?);
        let mut input = child.0.stdin.take().ok_or(())?;
        let mut out = child.0.stdout.take().ok_or(())?;
        let (events_tx, events) = mpsc::sync_channel(8);
        let (writes, rx) = mpsc::sync_channel::<Request>(2);
        std::thread::spawn(move || {
            let mut last = 0;
            loop {
                let event = packet(&mut out, last);
                if let Ok(e) = &event {
                    last = e.sequence;
                }
                let failed = event.is_err();
                if events_tx.send(event).is_err() || failed {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            for request in rx {
                let bytes = request.encode_to_vec();
                if input
                    .write_all(&(bytes.len() as u32).to_le_bytes())
                    .and_then(|_| input.write_all(&bytes))
                    .and_then(|_| input.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut seq = 0u64;
        let mut expected = 0;
        let mut stopping = None;
        let mut shutdown = false;
        loop {
            let exiting = inner.shutdown.load(Ordering::Acquire);
            let stop = inner.stop.swap(false, Ordering::AcqRel);
            let action = if (exiting && !shutdown) || stop {
                while actions.try_recv().is_ok() {}
                shutdown |= exiting;
                stopping = Some(Instant::now());
                Some((
                    if exiting {
                        Command::Shutdown
                    } else {
                        Command::Stop
                    },
                    String::new(),
                    None,
                ))
            } else if !shutdown {
                actions.try_recv().ok()
            } else {
                None
            };
            if let Some((command, text, reply)) = action {
                seq = seq.checked_add(1).ok_or(())?;
                expected = seq;
                writes
                    .try_send(Request {
                        version: 1,
                        sequence: seq,
                        command: command as i32,
                        text,
                        reply,
                    })
                    .map_err(|_| ())?;
            }
            for _ in 0..8 {
                match events.try_recv() {
                    Ok(Ok(event)) => {
                        if event.request_sequence >= expected {
                            inner.pending.store(false, Ordering::Release);
                            if !event.busy {
                                stopping = None;
                            }
                            *inner.view.lock().map_err(|_| ())? = event;
                        }
                    }
                    Ok(Err(_)) => return if shutdown { Ok(()) } else { Err(()) },
                    Err(_) => break,
                }
            }
            if child.0.try_wait().map_err(|_| ())?.is_some() {
                return if shutdown { Ok(()) } else { Err(()) };
            }
            if stopping.is_some_and(|at: Instant| at.elapsed() > Duration::from_secs(4)) {
                return Err(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    inner.pending.store(false, Ordering::Release);
    if result.is_err()
        && let Ok(mut view) = inner.view.lock()
    {
        *view = Event {
            version: 1,
            kind: Kind::Failed as i32,
            code: "SESSION_EXITED".into(),
            ..Default::default()
        };
    }
    inner.terminated.store(true, Ordering::Release);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supplied_reply_has_one_pending_slot_and_stop_never_waits_for_it() {
        let (tx, rx) = mpsc::sync_channel(2);
        let c = Controller(Arc::new(Inner {
            view: Mutex::new(Event {
                version: 1,
                kind: Kind::Starting as i32,
                busy: true,
                code: "PLAYING_PREPARED".into(),
                content_prepared: true,
                ..Default::default()
            }),
            pending: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            terminated: AtomicBool::new(false),
            actions: tx,
        }));
        assert!(c.reply_text("提问".into(), "回答".into()).is_ok());
        assert_eq!(
            c.reply_text("另一个问题".into(), "回答".into()),
            Err("BUSY".into())
        );
        let (kind, text, reply) = rx.try_recv().unwrap();
        assert_eq!(kind, Command::ReplyText);
        assert!(text.is_empty());
        assert_eq!(reply.unwrap().question, "提问");
        c.stop();
        assert!(c.0.stop.load(Ordering::Acquire));
        assert!(c.0.pending.load(Ordering::Acquire));
    }
    #[test]
    fn unexpected_binary_and_profile_rejected() {
        assert!(
            Controller::new(
                PathBuf::from("x"),
                PathBuf::from("y"),
                PathBuf::from("z"),
                PathBuf::from("a")
            )
            .is_err()
        );
    }
    #[test]
    fn malformed_event_rejected() {
        assert!(packet(&mut &[0, 0, 0, 0][..], 0).is_err());
        assert!(packet(&mut &[1, 0, 0, 0, 255][..], 0).is_err());
    }
}

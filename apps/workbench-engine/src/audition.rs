//! One isolated device child. No audio in the control pipe; bounded writers/readers.
use nai_contracts::{audition_wire as wire, workbench::*};
use nai_foundation_sqlite::Error;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
pub fn nonce(path: &Path) -> Result<String, Error> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= 64 => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err(Error::DataRootRejected);
                }
            }
            let mut s = String::new();
            std::fs::File::open(path)
                .map_err(|_| Error::IoFailed)?
                .take(65)
                .read_to_string(&mut s)
                .map_err(|_| Error::IoFailed)?;
            if s.len() > 64 {
                return Err(Error::DataRootRejected);
            }
            Ok(s)
        }
        _ => Err(Error::DataRootRejected),
    }
}
pub struct Active {
    pub ws: String,
    pub id: String,
    pub sha: String,
    pub until: Instant,
    init: Instant,
    selection_sent: bool,
    child: Child,
    tx: mpsc::SyncSender<AuditionControl>,
    rx: mpsc::Receiver<AuditionStatus>,
    _media: tempfile::NamedTempFile,
}
impl Drop for Active {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let end = Instant::now() + Duration::from_millis(250);
        while Instant::now() < end {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
pub struct Player {
    pub active: Option<Active>,
    pub view: AuditionView,
    stop_file: PathBuf,
}
impl Player {
    pub fn new(root: &Path) -> Self {
        Self {
            active: None,
            view: AuditionView {
                state: AuditionState::Stopped as i32,
                reason: String::new(),
                submitted_samples: 0,
                ..Default::default()
            },
            stop_file: root.join("audition.stop"),
        }
    }
    pub fn stop(&mut self, reason: &str) {
        self.active.take();
        self.view.state = AuditionState::Stopped as i32;
        self.view.reason = reason.into();
    }
    pub fn fail(&mut self) {
        self.active.take();
        self.view.state = AuditionState::Unavailable as i32;
        self.view.reason = "DEVICE_OR_LEASE_UNAVAILABLE".into();
    }
    pub fn start(
        &mut self,
        ws: &str,
        id: &str,
        sha: &str,
        p: &TtsAudioAuthorization,
        pcm: &[i16],
        expected: &str,
    ) -> Result<(), Error> {
        self.stop("REPLACED");
        // A stale stop fence must be rejected before probing any device capability;
        // otherwise an unavailable worker masks the expected Cancelled outcome.
        let stop_nonce = nonce(&self.stop_file)?;
        if stop_nonce != expected {
            return Err(Error::Cancelled);
        }
        let worker = std::env::current_exe()
            .map_err(|_| Error::IoFailed)?
            .with_file_name(if cfg!(windows) {
                "nai-audition-worker.exe"
            } else {
                "nai-audition-worker"
            });
        let meta = std::fs::symlink_metadata(&worker).map_err(|_| Error::CapabilityUnavailable)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(Error::CapabilityUnavailable);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err(Error::CapabilityUnavailable);
            }
        }
        let mut media = tempfile::NamedTempFile::new().map_err(|_| Error::IoFailed)?;
        {
            let mut out = std::io::BufWriter::new(media.as_file_mut());
            for s in pcm {
                out.write_all(&s.to_le_bytes())
                    .map_err(|_| Error::IoFailed)?;
            }
            out.flush().map_err(|_| Error::IoFailed)?;
        }
        if nonce(&self.stop_file)? != expected {
            return Err(Error::Cancelled);
        }
        let mut command = Command::new(worker);
        command
            .args(["--media"])
            .arg(media.path())
            .arg("--stop")
            .arg(&self.stop_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env_clear();
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| Error::CapabilityUnavailable)?;
        let mut input = child.stdin.take().ok_or(Error::IoFailed)?;
        let mut output = child.stdout.take().ok_or(Error::IoFailed)?;
        let (tx, rx) = mpsc::sync_channel::<AuditionControl>(2);
        // A final submission may immediately follow a progress event. Keep this
        // bounded burst intact; slow/stalled consumers still disconnect closed.
        let (status_tx, status_rx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            while let Ok(c) = rx.recv() {
                if wire::write(&mut input, &c).is_err() {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            while let Ok(s) = wire::read::<AuditionStatus>(&mut output) {
                if s.view.as_ref().is_none_or(|v| {
                    v.reason.len() > 64
                        || v.submitted_samples > 480000
                        || !(1..=7).contains(&v.state)
                        || v.devices.len() > 16
                        || v.devices
                            .iter()
                            .any(|d| d.device_id == 0 || d.name.len() > 128)
                        || v.selected_device_name.len() > 128
                }) || status_tx.try_send(s).is_err()
                {
                    break;
                }
            }
        });
        let output_session_id: String =
            Sha256::digest(format!("{}:{}:{}", p.lease_epoch, id, sha).as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
        let start = AuditionControl {
            operation: Some(audition_control::Operation::Start(AuditionStart {
                artifact_id: p.artifact_id.clone(),
                sample_count: p.sample_count,
                sample_rate: p.sample_rate,
                pcm_sha256: p.pcm_sha256.clone(),
                stop_nonce,
                output_session_id: output_session_id.clone(),
            })),
        };
        let mut active = Active {
            ws: ws.into(),
            id: id.into(),
            sha: sha.into(),
            until: Instant::now() + Duration::from_millis(600),
            init: Instant::now() + Duration::from_secs(2),
            selection_sent: false,
            child,
            tx,
            rx: status_rx,
            _media: media,
        };
        if active.tx.try_send(start).is_err() {
            let _ = active.child.kill();
            return Err(Error::CapabilityUnavailable);
        }
        self.active = Some(active);
        self.view = AuditionView {
            state: AuditionState::Starting as i32,
            reason: String::new(),
            submitted_samples: 0,
            output_session_id,
            ..Default::default()
        };
        Ok(())
    }
    pub fn select(&mut self, x: &FoundationOutputDeviceSelect) -> Result<(), Error> {
        let a = self.active.as_mut().ok_or(Error::CapabilityUnavailable)?;
        let s = x.selection.as_ref().ok_or(Error::InvalidArgument)?;
        if a.ws != x.workspace_id
            || a.id != x.task_id
            || s.output_session_id != self.view.output_session_id
            || self.view.state != AuditionState::Choosing as i32
            || a.selection_sent
        {
            return Err(Error::RevisionConflict);
        }
        if Instant::now() >= a.until {
            return Err(Error::DeadlineExceeded);
        }
        if !self.view.devices.iter().any(|d| d.device_id == s.device_id) {
            return Err(Error::InvalidArgument);
        }
        a.tx.try_send(AuditionControl {
            operation: Some(audition_control::Operation::SelectDevice(s.clone())),
        })
        .map_err(|_| Error::Busy)?;
        a.init = Instant::now() + Duration::from_secs(2);
        a.selection_sent = true;
        self.view.state = AuditionState::Starting as i32;
        Ok(())
    }
    pub fn next(&mut self) -> Result<Option<u64>, Error> {
        let Some(a) = self.active.as_mut() else {
            return Ok(None);
        };
        if Instant::now() >= a.until
            || self.view.state == AuditionState::Starting as i32 && Instant::now() >= a.init
        {
            self.fail();
            self.view.reason = "GUI_OR_INIT_DEADLINE".into();
            return Ok(None);
        }
        match a.rx.try_recv() {
            Ok(s) => {
                if a.selection_sent
                    && s.view
                        .as_ref()
                        .is_some_and(|v| v.state == AuditionState::Choosing as i32)
                {
                    return Ok(None);
                }
                self.view = s.view.ok_or(Error::ProtocolError)?;
                if matches!(self.view.state,x if x==AuditionState::Submitted as i32||x==AuditionState::Stopped as i32||x==AuditionState::Unavailable as i32)
                {
                    self.active.take();
                    return Ok(None);
                }
                Ok((s.challenge > 0).then_some(s.challenge))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.fail();
                self.view.reason = "WORKER_DISCONNECTED".into();
                Ok(None)
            }
        }
    }
    pub fn renew_worker(&mut self, challenge: u64, p: &TtsAudioAuthorization) -> Result<(), Error> {
        let a = self.active.as_ref().ok_or(Error::CapabilityUnavailable)?;
        let left = a
            .until
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(u128::from(p.ttl_ms)) as u32;
        if left == 0
            || a.tx
                .try_send(AuditionControl {
                    operation: Some(audition_control::Operation::Lease(AuditionLease {
                        challenge,
                        epoch: p.lease_epoch,
                        ttl_ms: left,
                    })),
                })
                .is_err()
        {
            self.fail();
            return Err(Error::CapabilityUnavailable);
        }
        Ok(())
    }
}

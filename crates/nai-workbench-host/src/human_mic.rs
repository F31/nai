//! Fixed on-demand raw-voice child. This pipe carries no microphone samples.
use super::{Reap, background_command};
use nai_contracts::{audition_wire as wire, workbench::*};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::mpsc,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};
pub fn binary() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(p) = std::env::var_os("NAI_HUMAN_MIC_WORKER") {
        let p = PathBuf::from(p);
        if p.is_absolute() && p.is_file() {
            return Ok(p);
        }
        return Err("WB_MIC_WORKER_UNAVAILABLE".into());
    }
    let p = std::env::current_exe()
        .map_err(|_| "WB_MIC_WORKER_UNAVAILABLE")?
        .with_file_name(format!(
            "nai-human-mic-worker{}",
            std::env::consts::EXE_SUFFIX
        ));
    if p.is_file() {
        Ok(p)
    } else {
        Err("WB_MIC_WORKER_UNAVAILABLE".into())
    }
}
pub fn stop(root: &Path) -> Result<(), String> {
    super::foundation::write_stop(root, "human-mic.stop")
}
pub struct MicOwner {
    child: Reap,
    input: mpsc::SyncSender<MicControl>,
    output: mpsc::Receiver<Result<MicStatus, std::io::Error>>,
    token: Zeroizing<String>,
    pub status: MicStatus,
    seq: u64,
    observed: Instant,
    poisoned: bool,
}
impl MicOwner {
    pub fn open_at(binary: &Path, root: &Path) -> Result<Self, String> {
        super::foundation::read_stop(root, "human-mic.stop")?;
        let mut raw = [0u8; 32];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut raw)
            .map_err(|_| "WB_RANDOM_FAILED")?;
        let token = Zeroizing::new(raw.iter().map(|b| format!("{b:02x}")).collect::<String>());
        raw.zeroize();
        let instance = super::foundation::random_id()?;
        let mut cmd = background_command(binary);
        cmd.env_clear();
        if let Some(p) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", p);
        }
        cmd.arg("--stop-file")
            .arg(root.join("human-mic.stop"))
            .env("NAI_MIC_TOKEN", token.as_str())
            .env("NAI_MIC_INSTANCE", &instance)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = Reap(cmd.spawn().map_err(|_| "WB_MIC_WORKER_UNAVAILABLE")?);
        let mut input = child.0.stdin.take().ok_or("WB_PIPE_FAILED")?;
        let mut output = child.0.stdout.take().ok_or("WB_PIPE_FAILED")?;
        let (tx, rx) = mpsc::sync_channel::<MicControl>(1);
        std::thread::spawn(move || {
            while let Ok(mut c) = rx.recv() {
                let r = wire::write(&mut input, &c);
                c.authorization.zeroize();
                if r.is_err() {
                    break;
                }
            }
        });
        let (otx, orx) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            loop {
                let r = wire::read::<MicStatus>(&mut output);
                let bad = r.is_err();
                if otx.send(r).is_err() || bad {
                    break;
                }
            }
        });
        let status = orx
            .recv_timeout(Duration::from_secs(4))
            .map_err(|_| "WB_MIC_START_TIMEOUT")?
            .map_err(|_| "WB_MIC_WORKER_LOST")?;
        if status.instance_id != instance || status.active || status.seq != 0 || !valid(&status) {
            return Err("WB_PROTOCOL_ERROR".into());
        }
        Ok(Self {
            child,
            input: tx,
            output: orx,
            token,
            status,
            seq: 0,
            observed: Instant::now(),
            poisoned: false,
        })
    }
    fn send(
        &mut self,
        action: u32,
        session: &str,
        input_id: u32,
        output_id: u32,
        ack: bool,
    ) -> Result<MicStatus, String> {
        self.send_pumping(action, session, input_id, output_id, ack, None)
    }
    #[allow(clippy::too_many_arguments)]
    fn send_pumping(
        &mut self,
        action: u32,
        session: &str,
        input_id: u32,
        output_id: u32,
        ack: bool,
        mut output: Option<&mut crate::public_output::OutputOwner>,
    ) -> Result<MicStatus, String> {
        if self.poisoned {
            return Err("WB_MIC_CHANNEL_INVALID".into());
        }
        if self
            .child
            .0
            .try_wait()
            .map_err(|_| "WB_MIC_WORKER_LOST")?
            .is_some()
        {
            return Err("WB_MIC_WORKER_LOST".into());
        }
        self.seq = self.seq.checked_add(1).ok_or("WB_PROTOCOL_ERROR")?;
        self.input
            .try_send(MicControl {
                protocol_major: 1,
                authorization: self.token.to_string(),
                instance_id: self.status.instance_id.clone(),
                seq: self.seq,
                challenge: self.status.challenge,
                action,
                session_id: session.into(),
                input_id,
                output_id,
                acknowledge_route: ack,
            })
            .map_err(|_| "WB_MIC_BUSY")?;
        let timeout = if action == 1 || action == 0 && !self.status.active {
            Duration::from_secs(4)
        } else {
            Duration::from_millis(250)
        };
        let r = wait_status(&self.output, timeout, || {
            if let Some(o) = output.as_deref_mut() {
                let s = o.poll()?;
                if s.terminal || s.audio_active {
                    return Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into());
                }
            }
            Ok(())
        });
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                self.poisoned = true;
                return Err(e);
            }
        };
        if r.instance_id != self.status.instance_id
            || r.seq != self.seq
            || r.challenge <= self.status.challenge
            || !valid(&r)
        {
            return Err("WB_MIC_PROTOCOL_ERROR".into());
        }
        self.status = r.clone();
        self.observed = Instant::now();
        Ok(r)
    }
    pub fn poll(&mut self) -> Result<MicStatus, String> {
        if self.observed.elapsed() >= Duration::from_millis(500) && self.status.active {
            return Err("WB_MIC_LEASE_EXPIRED".into());
        }
        let action = if self.status.active { 2 } else { 0 };
        self.send(
            action,
            &self.status.session_id.clone(),
            self.status.input_id,
            self.status.output_id,
            false,
        )
    }
    pub fn start(
        &mut self,
        instance: &str,
        input: u32,
        output: u32,
        ack: bool,
    ) -> Result<MicStatus, String> {
        self.start_pumping(instance, input, output, ack, None)
    }
    /// Maintain standby health while opening devices. Never renew an AI media permit.
    pub fn start_with_standby(
        &mut self,
        instance: &str,
        input: u32,
        output: u32,
        ack: bool,
        standby: &mut crate::public_output::OutputOwner,
    ) -> Result<MicStatus, String> {
        let s = standby.poll()?;
        if s.terminal || s.audio_active {
            return Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into());
        }
        self.start_pumping(instance, input, output, ack, Some(standby))
    }
    fn start_pumping(
        &mut self,
        instance: &str,
        input: u32,
        output: u32,
        ack: bool,
        mut standby: Option<&mut crate::public_output::OutputOwner>,
    ) -> Result<MicStatus, String> {
        if self.status.active
            || self.status.instance_id != instance
            || !ack
            || !self.status.inputs.iter().any(|d| d.device_id == input)
            || !self.status.outputs.iter().any(|d| d.device_id == output)
        {
            return Err("WB_MIC_ROUTE_REJECTED".into());
        }
        self.send_pumping(
            0,
            &self.status.session_id.clone(),
            self.status.input_id,
            self.status.output_id,
            false,
            standby.as_deref_mut(),
        )?;
        let session = super::foundation::random_id()?;
        let r = self.send_pumping(1, &session, input, output, true, standby.as_deref_mut())?;
        if r.reason != "PREPARED" || r.active {
            return Err("WB_MIC_DEVICE_UNAVAILABLE".into());
        }
        let r = self.send_pumping(4, &session, input, output, true, standby)?;
        if !r.active {
            return Err("WB_MIC_DEVICE_UNAVAILABLE".into());
        }
        Ok(r)
    }
    pub fn halt(&mut self) -> Result<MicStatus, String> {
        let r = self.send(3, &self.status.session_id.clone(), 0, 0, false)?;
        if r.active {
            return Err("WB_MIC_STOP_UNCONFIRMED".into());
        }
        Ok(r)
    }
}
fn wait_status(
    rx: &mpsc::Receiver<Result<MicStatus, std::io::Error>>,
    timeout: Duration,
    mut idle: impl FnMut() -> Result<(), String>,
) -> Result<MicStatus, String> {
    let until = Instant::now() + timeout;
    loop {
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("WB_MIC_RESPONSE_TIMEOUT".into());
        }
        match rx.recv_timeout(remaining.min(Duration::from_millis(10))) {
            Ok(r) => return r.map_err(|_| "WB_MIC_WORKER_LOST".into()),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("WB_MIC_WORKER_LOST".into()),
            Err(mpsc::RecvTimeoutError::Timeout) => idle()?,
        }
    }
}
fn valid(s: &MicStatus) -> bool {
    s.protocol_major == 1
        && s.inputs.len() <= 8
        && s.outputs.len() <= 8
        && s.reason.len() <= 64
        && s.inputs
            .iter()
            .chain(s.outputs.iter())
            .all(|d| d.device_id > 0 && d.name.len() <= 96 && !d.name.chars().any(char::is_control))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_wait_keeps_standby_polling_without_extending_deadline() {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut ticks = 0;
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            tx.send(Ok(MicStatus::default())).unwrap();
        });
        assert!(
            wait_status(&rx, Duration::from_secs(1), || {
                ticks += 1;
                Ok(())
            })
            .is_ok()
        );
        worker.join().unwrap();
        assert!(ticks >= 10);
        let (_tx, rx) = mpsc::sync_channel(1);
        let start = Instant::now();
        assert_eq!(
            wait_status(&rx, Duration::from_millis(50), || Ok(())).unwrap_err(),
            "WB_MIC_RESPONSE_TIMEOUT"
        );
        assert!(start.elapsed() < Duration::from_millis(200));
    }
    #[test]
    fn standby_failure_aborts_wait_without_waiting_for_device() {
        let (_tx, rx) = mpsc::sync_channel(1);
        let mut ticks = 0;
        assert_eq!(
            wait_status(&rx, Duration::from_secs(4), || {
                ticks += 1;
                Err("WB_PUBLIC_OUTPUT_UNAVAILABLE".into())
            })
            .unwrap_err(),
            "WB_PUBLIC_OUTPUT_UNAVAILABLE"
        );
        assert_eq!(ticks, 1);
    }
}

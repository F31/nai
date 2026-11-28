//! Private resident GPU process owner; no model implementation or playback grant.
use nai_contracts::{
    v561_avatar::{Cancel, Event, Kind, Render, Request, Shutdown, WireVersion, request::Action},
    workbench::WorkbenchError as Error,
};
use nai_runtime_core::foundation::Operation;
use prost::Message;
use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Child,
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub safetensors_backend: SafetensorsBackend,
    pub python: PathBuf,
    pub upstream_dir: PathBuf,
    pub dependency_dirs: Vec<PathBuf>,
    pub model_dir: PathBuf,
    pub wav2vec_dir: PathBuf,
    pub portrait: PathBuf,
    pub portrait_sha256: String,
    pub portrait_authorized: bool,
    pub mapping_reader: PathBuf,
    pub temp_dir: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frame_num: u32,
    pub seed: u32,
    pub face_ratio: f64,
    pub timeout_ms: u32,
    pub maintenance_timeout_ms: u32,
    pub warmup_samples: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_steps: Option<u32>,
}
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetensorsBackend {
    #[default]
    Mmap,
    Pread,
}
impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        let files = [&self.python, &self.portrait, &self.mapping_reader];
        if files.iter().any(|p| !p.is_absolute() || !p.is_file())
            || [
                &self.upstream_dir,
                &self.model_dir,
                &self.wav2vec_dir,
                &self.temp_dir,
            ]
            .iter()
            .any(|p| !p.is_absolute() || !p.is_dir())
            || self.dependency_dirs.is_empty()
            || self.dependency_dirs.len() > 8
            || self
                .dependency_dirs
                .iter()
                .any(|p| !p.is_absolute() || !p.is_dir())
            || !self.portrait_authorized
            || self.portrait_sha256.len() != 64
            || !self
                .portrait_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || ![self.width, self.height]
                .iter()
                .all(|n| (256..=1024).contains(n) && n.is_multiple_of(32))
            || !(17..=97).contains(&self.frame_num)
            || !self.face_ratio.is_finite()
            || !(1.5..=4.0).contains(&self.face_ratio)
            || !(1000..=45000).contains(&self.timeout_ms)
            || !(1000..=240000).contains(&self.maintenance_timeout_ms)
            || !(1..=144000).contains(&self.warmup_samples)
            || (self.warmup_samples * 25).div_ceil(24000) < u64::from(self.frame_num)
            || self.sample_steps.is_some_and(|s| !(2..=4).contains(&s))
        {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }
}
fn read_event(input: &mut impl Read) -> Result<Event, Error> {
    let mut size = [0; 4];
    input.read_exact(&mut size).map_err(|_| Error::IoFailed)?;
    let n = u32::from_le_bytes(size) as usize;
    if n == 0 || n > 16384 {
        return Err(Error::ProtocolError);
    }
    let mut data = vec![0; n];
    input.read_exact(&mut data).map_err(|_| Error::IoFailed)?;
    Event::decode(data.as_slice()).map_err(|_| Error::ProtocolError)
}
fn authenticate_event(event: &mut Event, key: &hmac::Key) -> Result<(), Error> {
    let mac = std::mem::take(&mut event.mac);
    hmac::verify(key, &event.encode_to_vec(), &mac).map_err(|_| Error::AuthenticationFailed)
}
fn validate_header(event: &Event, previous: u64, portrait: &[u8]) -> Result<(), Error> {
    if event.version != WireVersion::Current as u32
        || Some(event.sequence) != previous.checked_add(1)
        || event.portrait_sha256 != portrait
        || event.code.is_empty()
        || event.code.len() > 64
        || !event
            .code
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c == b'_')
        || event.model_loads > 1
        || event.frames > 150
        || event.frames_acked > event.frames
        || event.samples > 144000
        || Kind::try_from(event.kind)
            .ok()
            .is_none_or(|kind| kind == Kind::Unspecified)
        || (event.kind == Kind::Frame as i32) != event.frame.is_some()
        || (event.kind == Kind::Complete as i32) != event.stats.is_some()
        || (event.kind == Kind::Ready as i32) != event.warmup.is_some()
    {
        return Err(Error::ProtocolError);
    }
    Ok(())
}
fn validate_ready(
    event: &Event,
    samples: u64,
    timeout_ms: u32,
    frame_num: u32,
) -> Result<(), Error> {
    if event.kind != Kind::Ready as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    let warm = event.warmup.as_ref().ok_or(Error::ProtocolError)?;
    if event.model_loads != 1
        || event.busy
        || event.code != "READY"
        || event.task_id != 0
        || event.epoch != 0
        || event.request_sequence != 0
        || event.samples != 0
        || !event.pcm_sha256.is_empty()
        || event.frames != 0
        || event.frames_acked != 0
        || !event.mapping_name.is_empty()
        || event.elapsed_ms > u64::from(timeout_ms)
        || warm.samples != samples
        || warm.frames_acked != (samples * 25).div_ceil(24000)
        || !(2..=150).contains(&warm.blocks)
        || warm.unique_graphs == 0
        || warm.generated_kernels == 0
        || warm.block_frames == 0
        || warm.motion_frames == 0
        || warm.block_frames.checked_add(warm.motion_frames) != Some(frame_num)
        || u64::from(warm.blocks)
            != warm
                .frames_acked
                .div_ceil(u64::from(warm.block_frames.max(1)))
        || warm.elapsed_ms > event.elapsed_ms
    {
        return Err(Error::ProtocolError);
    }
    Ok(())
}
pub struct Resident {
    child: Child,
    key: hmac::Key,
    writes: Option<mpsc::SyncSender<Vec<u8>>>,
    events: mpsc::Receiver<Result<Event, Error>>,
    sequence: u64,
    event_sequence: u64,
    pending: Option<(u64, u64)>,
    pub ready_ms: u64,
    pub warmup: nai_contracts::v561_avatar::WarmupStats,
    portrait: Vec<u8>,
    mapping_reader: PathBuf,
    width: u32,
    height: u32,
    frame_num: u32,
    sample_steps: u32,
    _config: tempfile::NamedTempFile,
}
impl Drop for Resident {
    fn drop(&mut self) {
        let _ = self.send(0, 0, Action::Shutdown(Shutdown {}));
        self.writes.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Resident {
    pub fn warmup_json(&self) -> serde_json::Value {
        serde_json::json!({"samples":self.warmup.samples,"frames_acked":self.warmup.frames_acked,
            "blocks":self.warmup.blocks,"unique_graphs":self.warmup.unique_graphs,
            "generated_kernels":self.warmup.generated_kernels,"elapsed_ms":self.warmup.elapsed_ms,
            "block_frames":self.warmup.block_frames,"motion_frames":self.warmup.motion_frames})
    }
    pub fn start(
        config: &Config,
        script: &Path,
        operation: &Operation,
        log: &Path,
    ) -> Result<Self, Error> {
        config.validate()?;
        operation.check()?;
        let startup = operation.maintenance_child(config.maintenance_timeout_ms)?;
        if !script.is_absolute()
            || script.file_name().and_then(|v| v.to_str()) != Some("flashhead_resident_worker.py")
        {
            return Err(Error::InvalidArgument);
        }
        let mut file =
            tempfile::NamedTempFile::new_in(&config.temp_dir).map_err(|_| Error::IoFailed)?;
        serde_json::to_writer(&mut file, config).map_err(|_| Error::IoFailed)?;
        file.flush().map_err(|_| Error::IoFailed)?;
        let mut token = zeroize::Zeroizing::new([0u8; 32]);
        SystemRandom::new()
            .fill(&mut *token)
            .map_err(|_| Error::IoFailed)?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, &*token);
        let mut command =
            crate::v561_avatar_process::command(config, script, file.path(), &*token, log)?;
        let mut child = command.spawn().map_err(|_| Error::IoFailed)?;
        let mut input = child.stdin.take().ok_or(Error::IoFailed)?;
        let mut output = child.stdout.take().ok_or(Error::IoFailed)?;
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(2);
        let (event_tx, events) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            for bytes in rx {
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
        std::thread::spawn(move || {
            loop {
                let event = read_event(&mut output);
                let failed = event.is_err();
                if event_tx.send(event).is_err() || failed {
                    break;
                }
            }
        });
        let portrait = (0..64)
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&config.portrait_sha256[i..i + 2], 16)
                    .map_err(|_| Error::InvalidArgument)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut value = Self {
            child,
            key,
            writes: Some(tx),
            events,
            sequence: 0,
            event_sequence: 0,
            pending: None,
            ready_ms: 0,
            warmup: Default::default(),
            portrait,
            mapping_reader: config.mapping_reader.clone(),
            width: config.width,
            height: config.height,
            frame_num: config.frame_num,
            sample_steps: config.sample_steps.unwrap_or(4),
            _config: file,
        };
        let event = value.next(&startup)?;
        validate_ready(
            &event,
            config.warmup_samples,
            config.maintenance_timeout_ms,
            config.frame_num,
        )?;
        value.ready_ms = event.elapsed_ms;
        value.warmup = event.warmup.ok_or(Error::ProtocolError)?;
        Ok(value)
    }
    fn send(&mut self, task: u64, epoch: u64, action: Action) -> Result<(), Error> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Error::ResourceExhausted)?;
        let mut request = Request {
            version: WireVersion::Current as u32,
            sequence: self.sequence,
            task_id: task,
            epoch,
            mac: vec![],
            action: Some(action),
        };
        request.mac = hmac::sign(&self.key, &request.encode_to_vec())
            .as_ref()
            .to_vec();
        let bytes = request.encode_to_vec();
        if bytes.len() > 16384 {
            return Err(Error::ResourceExhausted);
        }
        self.writes
            .as_ref()
            .ok_or(Error::IoFailed)?
            .try_send(bytes)
            .map_err(|_| Error::ResourceExhausted)
    }
    fn next(&mut self, op: &Operation) -> Result<Event, Error> {
        loop {
            op.check()?;
            match self.events.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => {
                    let mut event = result?;
                    authenticate_event(&mut event, &self.key)?;
                    validate_header(&event, self.event_sequence, &self.portrait)?;
                    self.event_sequence = event.sequence;
                    op.check()?;
                    return Ok(event);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(Error::IoFailed),
            }
        }
    }
    pub fn render(
        &mut self,
        task: u64,
        epoch: u64,
        render: Render,
        op: &Operation,
    ) -> Result<Event, Error> {
        self.render_observed(task, epoch, render, op, |_| Ok(()))
    }
    pub fn render_observed(
        &mut self,
        task: u64,
        epoch: u64,
        render: Render,
        op: &Operation,
        mut accept: impl FnMut(nai_media_core::v561_frame_scheduler::Frame) -> Result<(), Error>,
    ) -> Result<Event, Error> {
        if self.pending.is_some()
            || task == 0
            || epoch == 0
            || render.sample_rate != 24000
            || !(1..=144000).contains(&render.samples)
            || render.pcm_sha256.len() != 32
            || render.pcm_path.len() > 4096
            || !Path::new(&render.pcm_path).is_absolute()
        {
            return Err(Error::InvalidArgument);
        }
        op.check()?;
        let samples = render.samples;
        let digest = render.pcm_sha256.clone();
        self.send(task, epoch, Action::Render(render))?;
        let request_sequence = self.sequence;
        self.pending = Some((task, epoch));
        let mut accepted = false;
        let mut ingress =
            crate::v561_avatar_frames::Ingress::new(epoch, samples, self.width, self.height)?;
        loop {
            let event = match self.next(op) {
                Ok(e) => e,
                Err(e) => {
                    let _ = self.send(task, epoch, Action::Cancel(Cancel {}));
                    return Err(e);
                }
            };
            if event.task_id != task
                || event.epoch != epoch
                || event.samples != samples
                || event.pcm_sha256 != digest
                || event.request_sequence != request_sequence
            {
                return Err(Error::ProtocolError);
            }
            match Kind::try_from(event.kind) {
                Ok(Kind::Accepted) if event.busy && !accepted => {
                    accepted = true;
                }
                Ok(Kind::Frame) if accepted && event.busy => {
                    let result = (|| {
                        let frame = event.frame.ok_or(Error::ProtocolError)?;
                        let (frame, release) =
                            ingress.read(frame, &event.mapping_name, &self.mapping_reader, op)?;
                        op.check()?;
                        accept(frame)?;
                        op.check()?;
                        self.send(task, epoch, Action::Release(release))
                    })();
                    if let Err(error) = result {
                        let _ = self.send(task, epoch, Action::Cancel(Cancel {}));
                        return Err(error);
                    }
                }
                Ok(Kind::Complete)
                    if accepted
                        && !event.busy
                        && event.model_loads == 1
                        && event.frames == (samples * 25).div_ceil(24000)
                        && event.frames_acked == event.frames =>
                {
                    if event.frames != ingress.count() {
                        return Err(Error::ProtocolError);
                    }
                    crate::v561_avatar_metrics::validate(
                        event.stats.as_ref().ok_or(Error::ProtocolError)?,
                        samples,
                        event.frames,
                        self.frame_num,
                        event.elapsed_ms,
                        self.sample_steps,
                    )?;
                    op.check()?;
                    self.pending = None;
                    return Ok(event);
                }
                Ok(Kind::Failed | Kind::Cancelled | Kind::Rejected) if !event.busy => {
                    self.pending = None;
                    return Err(Error::CapabilityUnavailable);
                }
                _ => return Err(Error::ProtocolError),
            }
        }
    }

    /// Wait for actual worker cleanup after cancellation; never treats cancel ACK as idle.
    pub fn drain_cancelled(&mut self, op: &Operation) -> Result<(), Error> {
        let Some((task, epoch)) = self.pending else {
            return Ok(());
        };
        loop {
            let event = self.next(op)?;
            if event.task_id != task || event.epoch != epoch {
                return Err(Error::ProtocolError);
            }
            match Kind::try_from(event.kind) {
                Ok(Kind::Accepted | Kind::CancelRequested | Kind::Frame) if event.busy => {}
                Ok(Kind::Complete | Kind::Cancelled | Kind::Failed) if !event.busy => {
                    // Complete can race with cancellation, but its media/result is discarded.
                    self.pending = None;
                    return Ok(());
                }
                Ok(Kind::Rejected) => {}
                _ => return Err(Error::ProtocolError),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_protocol_and_unbound_frames_are_rejected() {
        let event = Event {
            version: WireVersion::Current as u32,
            sequence: 1,
            kind: Kind::Ready as i32,
            code: "READY".into(),
            portrait_sha256: vec![1; 32],
            model_loads: 1,
            warmup: Some(Default::default()),
            ..Event::default()
        };
        assert!(validate_header(&event, 0, &[1; 32]).is_ok());
        for version in [1, 2, 3, 4, 5] {
            let mut bad = event.clone();
            bad.version = version;
            assert!(validate_header(&bad, 0, &[1; 32]).is_err());
        }
        let mut bad = event.clone();
        bad.kind = Kind::Frame as i32;
        assert!(validate_header(&bad, 0, &[1; 32]).is_err());
        bad.kind = Kind::Complete as i32;
        assert!(validate_header(&bad, 0, &[1; 32]).is_err());
        assert!(validate_header(&event, u64::MAX, &[1; 32]).is_err());
    }
    #[test]
    fn signature_covers_task_and_source_identity() {
        let key = hmac::Key::new(hmac::HMAC_SHA256, &[7u8; 32]);
        let mut event = Event {
            task_id: 1,
            epoch: 2,
            pcm_sha256: vec![3; 32],
            ..Event::default()
        };
        event.mac = hmac::sign(&key, &event.encode_to_vec()).as_ref().to_vec();
        let signed = event.clone();
        assert_eq!(authenticate_event(&mut event, &key), Ok(()));
        let mut changed = signed.clone();
        changed.task_id = 2;
        assert_eq!(
            authenticate_event(&mut changed, &key),
            Err(Error::AuthenticationFailed)
        );
        let mut changed = signed;
        changed.pcm_sha256[0] ^= 1;
        assert_eq!(
            authenticate_event(&mut changed, &key),
            Err(Error::AuthenticationFailed)
        );
        let mut frame = Event {
            mapping_name: "wnsm_first".into(),
            frame: Some(nai_contracts::v561_media::FrameRef::default()),
            ..Event::default()
        };
        frame.mac = hmac::sign(&key, &frame.encode_to_vec()).as_ref().to_vec();
        frame.mapping_name = "wnsm_other".into();
        assert_eq!(
            authenticate_event(&mut frame, &key),
            Err(Error::AuthenticationFailed)
        );
        let mut statistics = Event {
            stats: Some(nai_contracts::v561_avatar::RenderStats::default()),
            ..Event::default()
        };
        statistics.mac = hmac::sign(&key, &statistics.encode_to_vec())
            .as_ref()
            .to_vec();
        statistics.stats.as_mut().unwrap().tail_frames = 1;
        assert_eq!(
            authenticate_event(&mut statistics, &key),
            Err(Error::AuthenticationFailed)
        );
    }
    #[test]
    fn oversized_control_is_rejected_before_allocation() {
        assert_eq!(
            read_event(&mut std::io::Cursor::new(16385u32.to_le_bytes())),
            Err(Error::ProtocolError)
        );
    }
    #[test]
    fn truncated_control_is_not_a_receipt() {
        assert_eq!(
            read_event(&mut std::io::Cursor::new([1u8, 0, 0, 0])),
            Err(Error::IoFailed)
        );
    }
    #[test]
    fn ready_requires_actual_warmup_and_clean_maintenance_identity() {
        let event = Event {
            kind: Kind::Ready as i32,
            code: "READY".into(),
            model_loads: 1,
            elapsed_ms: 100,
            warmup: Some(nai_contracts::v561_avatar::WarmupStats {
                samples: 76800,
                frames_acked: 80,
                blocks: 2,
                unique_graphs: 13,
                generated_kernels: 300,
                elapsed_ms: 80,
                block_frames: 40,
                motion_frames: 9,
            }),
            ..Event::default()
        };
        assert_eq!(validate_ready(&event, 76800, 240000, 49), Ok(()));
        for mode in 0..10 {
            let mut bad = event.clone();
            match mode {
                0 => bad.warmup = None,
                1 => bad.warmup.as_mut().unwrap().unique_graphs = 0,
                2 => bad.warmup.as_mut().unwrap().frames_acked = 79,
                3 => bad.warmup.as_mut().unwrap().blocks = 1,
                4 => bad.elapsed_ms = 240001,
                5 => bad.task_id = 1,
                6 => bad.warmup.as_mut().unwrap().elapsed_ms = 101,
                7 => bad.warmup.as_mut().unwrap().generated_kernels = 0,
                8 => bad.warmup.as_mut().unwrap().block_frames = 0,
                _ => bad.warmup.as_mut().unwrap().motion_frames = 8,
            }
            assert!(validate_ready(&bad, 76800, 240000, 49).is_err());
        }
    }
    #[test]
    fn signature_covers_compilation_evidence() {
        let key = hmac::Key::new(hmac::HMAC_SHA256, &[8; 32]);
        let mut event = Event {
            warmup: Some(Default::default()),
            ..Event::default()
        };
        event.mac = hmac::sign(&key, &event.encode_to_vec()).as_ref().to_vec();
        event.warmup.as_mut().unwrap().generated_kernels = 1;
        assert_eq!(
            authenticate_event(&mut event, &key),
            Err(Error::AuthenticationFailed)
        );
    }
    #[test]
    fn sample_steps_only_two_through_four_and_roundtrip() {
        let directory = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(directory.path()).unwrap();
        let portrait = root.join("portrait");
        std::fs::write(&portrait, b"x").unwrap();
        let python = root.join("python");
        std::fs::write(&python, b"x").unwrap();
        let base = Config {
            safetensors_backend: Default::default(),
            python,
            upstream_dir: root.clone(),
            dependency_dirs: vec![root.clone()],
            model_dir: root.clone(),
            wav2vec_dir: root.clone(),
            portrait: portrait.clone(),
            portrait_sha256: "0".repeat(64),
            portrait_authorized: true,
            mapping_reader: portrait,
            temp_dir: root,
            width: 512,
            height: 512,
            frame_num: 41,
            seed: 42,
            face_ratio: 2.8,
            timeout_ms: 45000,
            maintenance_timeout_ms: 240000,
            warmup_samples: 76800,
            sample_steps: None,
        };
        assert!(base.validate().is_ok());
        let json = serde_json::to_string(&base).unwrap();
        assert!(!json.contains("sample_steps"));
        assert!(
            serde_json::from_str::<Config>(&json)
                .unwrap()
                .validate()
                .is_ok()
        );
        for steps in [2u32, 3, 4] {
            let mut config = base.clone();
            config.sample_steps = Some(steps);
            assert!(config.validate().is_ok());
            let back: Config =
                serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
            assert_eq!(back.sample_steps, Some(steps));
        }
        for bad in [0u32, 1, 5, 8] {
            let mut config = base.clone();
            config.sample_steps = Some(bad);
            assert_eq!(config.validate(), Err(Error::InvalidArgument));
        }
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "explicit Windows process-owner maintenance fault test; no GPU or media"]
    fn native_maintenance_cancel_and_timeout_never_return_ready() {
        use std::time::{Duration, Instant};
        let python = std::env::var_os("NAI_AVATAR_TEST_PYTHON").expect("explicit test Python");
        for cancel in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(directory.path()).unwrap();
            let fixture = root.join("flashhead_resident_worker.py");
            std::fs::write(&fixture, "import pathlib,time\np=pathlib.Path(__file__).with_name('heartbeat')\nn=0\nwhile True:\n p.write_text(str(n));n+=1;time.sleep(.01)\n").unwrap();
            let portrait = root.join("portrait");
            std::fs::write(&portrait, b"test-only-never-ready").unwrap();
            let config = Config {
                safetensors_backend: Default::default(),
                python: python.clone().into(),
                upstream_dir: root.clone(),
                dependency_dirs: vec![root.clone()],
                model_dir: root.clone(),
                wav2vec_dir: root.clone(),
                portrait: portrait.clone(),
                portrait_sha256: "0".repeat(64),
                portrait_authorized: true,
                mapping_reader: portrait,
                temp_dir: root.clone(),
                width: 768,
                height: 768,
                frame_num: 49,
                seed: 1,
                face_ratio: 2.8,
                timeout_ms: 45000,
                maintenance_timeout_ms: 2000,
                warmup_samples: 76800,
                sample_steps: None,
            };
            let parent = Operation::maintenance(if cancel { 2000 } else { 300 }).unwrap();
            let trigger = parent.clone();
            let heartbeat = root.join("heartbeat");
            let observed = heartbeat.clone();
            let thread = std::thread::spawn(move || {
                if cancel {
                    for _ in 0..100 {
                        if observed.exists() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    trigger.cancel();
                }
            });
            let started = Instant::now();
            let result = Resident::start(&config, &fixture, &parent, &root.join("worker.log"));
            thread.join().unwrap();
            assert_eq!(
                result.err(),
                Some(if cancel {
                    Error::Cancelled
                } else {
                    Error::DeadlineExceeded
                })
            );
            assert!(started.elapsed() < Duration::from_secs(3));
            if heartbeat.exists() {
                let before = std::fs::read(&heartbeat).unwrap();
                std::thread::sleep(Duration::from_millis(80));
                assert_eq!(std::fs::read(&heartbeat).unwrap(), before);
            }
        }
    }
}

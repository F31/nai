//! Rust owner for real worker2. No audio device or public playback authority.
use crate::{v561_avatar_frames::Ingress, v561_avatar_raw::Sender, v561_avatar_worker::Config};
use nai_contracts::{
    v561_avatar_stream::{self as wire, request::Action},
    workbench::WorkbenchError as Error,
};
use nai_media_core::v561_frame_scheduler::Frame;
use nai_runtime_core::foundation::Operation;
use prost::Message;
use ring::hmac;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    process::Child,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

struct Task {
    id: u64,
    epoch: u64,
    begin: u64,
    max: u64,
    samples: u64,
    hash: Sha256,
    op: Operation,
    ingress: Ingress,
    eos: bool,
}

pub(crate) fn safe_preparation_code(code: &str) -> &'static str {
    match code {
        "CUDA_PREPARATION_FAILED" => "CUDA_PREPARATION_FAILED",
        "CUDA_MEMORY_EXHAUSTED" => "CUDA_MEMORY_EXHAUSTED",
        "HOST_MEMORY_EXHAUSTED" => "HOST_MEMORY_EXHAUSTED",
        "MODEL_PREPARATION_FAILED" => "MODEL_PREPARATION_FAILED",
        "PREPARATION_TIMEOUT" => "PREPARATION_TIMEOUT",
        _ => "PREPARE_FAILED",
    }
}

/// Only bounded root-written metadata is exposed; never forward traceback text.
pub(crate) fn preparation_failure(dir: &Path) -> &'static str {
    let Ok(file) = std::fs::File::open(dir.join("worker.startup-failure.json")) else {
        return "PREPARE_FAILED";
    };
    let Ok(value) = serde_json::from_reader::<_, serde_json::Value>(file.take(4096)) else {
        return "PREPARE_FAILED";
    };
    if value["scope"] != "avatar_startup_failure_not_ready" {
        return "PREPARE_FAILED";
    }
    safe_preparation_code(value["preparation_code"].as_str().unwrap_or_default())
}
pub struct Resident {
    child: Child,
    raw: Sender,
    key: hmac::Key,
    writes: Option<mpsc::SyncSender<Vec<u8>>>,
    events: mpsc::Receiver<Result<wire::Event, Error>>,
    sequence: u64,
    event_sequence: u64,
    payload: u64,
    last_task: u64,
    last_epoch: u64,
    active: Option<Task>,
    broken: bool,
    alive: Arc<AtomicBool>,
    config: Config,
    _file: tempfile::NamedTempFile,
    pub ready: wire::Event,
}
fn read_event(input: &mut impl Read) -> Result<wire::Event, Error> {
    let mut prefix = [0; 4];
    input.read_exact(&mut prefix).map_err(|_| Error::IoFailed)?;
    let size = u32::from_le_bytes(prefix) as usize;
    if !(1..=16384).contains(&size) {
        return Err(Error::ProtocolError);
    }
    let mut bytes = vec![0; size];
    input.read_exact(&mut bytes).map_err(|_| Error::IoFailed)?;
    wire::Event::decode(bytes.as_slice()).map_err(|_| Error::ProtocolError)
}
fn header(e: &wire::Event, previous: u64, portrait: &[u8]) -> Result<(), Error> {
    if e.version != wire::WireVersion::Current as u32
        || Some(e.sequence) != previous.checked_add(1)
        || e.model_loads != 1
        || e.portrait_sha256 != portrait
        || e.samples > 144000
        || e.frames > 150
        || e.code.len() > 64
        || !e.code.bytes().all(|c| c.is_ascii_uppercase() || c == b'_')
        || wire::Kind::try_from(e.kind)
            .ok()
            .is_none_or(|k| k == wire::Kind::Unspecified)
        || (e.kind == wire::Kind::Frame as i32) != e.frame.is_some()
        || (e.kind == wire::Kind::Frame as i32) == e.mapping_name.is_empty()
        || (e.kind == wire::Kind::Ready as i32) != e.warmup.is_some()
        || (e.kind == wire::Kind::RenderComplete as i32) != e.stats.is_some()
    {
        return Err(Error::ProtocolError);
    }
    Ok(())
}
fn ready(e: &wire::Event, c: &Config) -> Result<(), Error> {
    let w = e.warmup.as_ref().ok_or(Error::ProtocolError)?;
    if e.kind != wire::Kind::Ready as i32
        || e.task_id != 0
        || e.epoch != 0
        || e.request_sequence != 0
        || e.samples != 0
        || e.frames != 0
        || !e.sha256.is_empty()
        || e.elapsed_ms > u64::from(c.maintenance_timeout_ms)
        || w.samples != c.warmup_samples
        || w.frames_acked != (w.samples * 25).div_ceil(24000)
        || w.block_frames == 0
        || w.motion_frames == 0
        || w.block_frames.checked_add(w.motion_frames) != Some(c.frame_num)
        || w.blocks < 2
        || u64::from(w.blocks) != w.frames_acked.div_ceil(u64::from(w.block_frames.max(1)))
        || w.unique_graphs == 0
        || w.generated_kernels == 0
        || w.elapsed_ms > e.elapsed_ms
    {
        return Err(Error::ProtocolError);
    }
    Ok(())
}
fn source_bound(start: u64, samples: u64, eos: bool) -> Result<(), Error> {
    if start >= samples || !eos && start.saturating_add(960) > samples {
        return Err(Error::ProtocolError);
    }
    Ok(())
}
impl Drop for Resident {
    fn drop(&mut self) {
        self.abort();
    }
}
impl Resident {
    pub fn start(
        config: &Config,
        script: &Path,
        parent: &Operation,
        log: &Path,
    ) -> Result<Self, Error> {
        config.validate()?;
        if !script.is_absolute()
            || script.file_name().and_then(|n| n.to_str()) != Some("flashhead_stream_worker.py")
        {
            return Err(Error::InvalidArgument);
        }
        let startup = parent.maintenance_child(config.maintenance_timeout_ms)?;
        let raw = Sender::bind(&startup)?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, raw.key());
        let mut file =
            tempfile::NamedTempFile::new_in(&config.temp_dir).map_err(|_| Error::IoFailed)?;
        serde_json::to_writer(&mut file, config).map_err(|_| Error::IoFailed)?;
        file.flush().map_err(|_| Error::IoFailed)?;
        let mut command =
            crate::v561_avatar_process::command(config, script, file.path(), raw.key(), log)?;
        command.arg("--raw-port").arg(raw.port().to_string());
        let mut child = command.spawn().map_err(|_| Error::IoFailed)?;
        let Some(mut input) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::IoFailed);
        };
        let Some(mut output) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::IoFailed);
        };
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(2);
        let (et, events) = mpsc::sync_channel(8);
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
        let alive = Arc::new(AtomicBool::new(true));
        let reader_alive = alive.clone();
        std::thread::spawn(move || {
            loop {
                let e = read_event(&mut output);
                let failed = e.is_err();
                if failed {
                    reader_alive.store(false, Ordering::Release);
                }
                if et.send(e).is_err() || failed {
                    break;
                }
            }
            reader_alive.store(false, Ordering::Release);
        });
        let mut owner = Self {
            child,
            raw,
            key,
            writes: Some(tx),
            events,
            sequence: 0,
            event_sequence: 0,
            payload: 0,
            last_task: 0,
            last_epoch: 0,
            active: None,
            broken: false,
            alive,
            config: config.clone(),
            _file: file,
            ready: Default::default(),
        };
        let mut phase = "READY_EVENT";
        let mut preparation_code = "PREPARE_FAILED";
        let result = (|| {
            let e = owner.next(&startup)?;
            if e.kind == wire::Kind::Failed as i32
                && e.task_id == 0
                && e.epoch == 0
                && e.request_sequence == 0
            {
                preparation_code = safe_preparation_code(&e.code);
                return Err(Error::CapabilityUnavailable);
            }
            phase = "READY_VALIDATION";
            ready(&e, config)?;
            owner.ready = e;
            phase = "RAW_AUTH";
            owner.raw.accept(&startup)
        })();
        if let Err(error) = result {
            if error == Error::DeadlineExceeded {
                preparation_code = "PREPARATION_TIMEOUT";
            }
            let began = std::time::Instant::now();
            let mut status = owner.child.try_wait().ok().flatten();
            while status.is_none() && began.elapsed() < Duration::from_millis(200) {
                std::thread::sleep(Duration::from_millis(5));
                status = owner.child.try_wait().ok().flatten();
            }
            if let Ok(file) = std::fs::File::create_new(log.with_extension("startup-failure.json"))
            {
                let _ = serde_json::to_writer_pretty(
                    file,
                    &serde_json::json!({"scope":"avatar_startup_failure_not_ready","phase":phase,
                    "code":error.as_str_name(),"preparation_code":preparation_code,"exit_code":status.and_then(|s|s.code()),
                    "exit_hex":status.and_then(|s|s.code()).map(|c|format!("{:08X}",c as u32))}),
                );
            }
            return Err(error);
        }
        Ok(owner)
    }
    fn abort(&mut self) {
        self.broken = true;
        self.alive.store(false, Ordering::Release);
        if let Some(t) = self.active.take() {
            t.op.cancel();
        }
        self.raw.close();
        self.writes.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
    pub fn available(&self) -> bool {
        !self.broken && self.alive.load(Ordering::Acquire) && self.active.is_none()
    }
    pub(crate) fn liveness(&self) -> Arc<AtomicBool> {
        self.alive.clone()
    }
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
    fn portrait(&self) -> Vec<u8> {
        (0..64)
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&self.config.portrait_sha256[i..i + 2], 16)
                    .expect("validated portrait")
            })
            .collect()
    }
    fn send(&mut self, id: u64, epoch: u64, action: Action) -> Result<u64, Error> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Error::ResourceExhausted)?;
        let mut request = wire::Request {
            version: wire::WireVersion::Current as u32,
            sequence: self.sequence,
            task_id: id,
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
            .map_err(|_| Error::ResourceExhausted)?;
        Ok(self.sequence)
    }
    fn next(&mut self, op: &Operation) -> Result<wire::Event, Error> {
        loop {
            op.check()?;
            match self.events.recv_timeout(Duration::from_millis(10)) {
                Ok(result) => {
                    let e = self.authenticate(result?, op)?;
                    return Ok(e);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(Error::IoFailed),
            }
        }
    }
    fn settle<T>(&mut self, result: Result<T, Error>) -> Result<T, Error> {
        if let Err(error) = &result {
            if matches!(error, Error::Cancelled | Error::DeadlineExceeded) {
                if self.cancel().is_err() {
                    self.abort();
                }
            } else {
                self.abort();
            }
        }
        result
    }
    pub fn begin(
        &mut self,
        id: u64,
        epoch: u64,
        max: u64,
        parent: &Operation,
    ) -> Result<(), Error> {
        if !self.available()
            || id <= self.last_task
            || epoch <= self.last_epoch
            || !(1..=144000).contains(&max)
        {
            return Err(Error::InvalidArgument);
        }
        let op = parent.model_child(self.config.timeout_ms)?;
        let ingress = Ingress::new(epoch, max, self.config.width, self.config.height)?;
        let begin = match self.send(
            id,
            epoch,
            Action::Begin(wire::Begin {
                portrait_sha256: self.portrait(),
                block_frames: self
                    .ready
                    .warmup
                    .as_ref()
                    .ok_or(Error::ProtocolError)?
                    .block_frames,
                sample_rate: 24000,
                max_samples: max,
            }),
        ) {
            Ok(sequence) => sequence,
            Err(error) => {
                self.abort();
                return Err(error);
            }
        };
        self.active = Some(Task {
            id,
            epoch,
            begin,
            max,
            samples: 0,
            hash: Sha256::new(),
            op,
            ingress,
            eos: false,
        });
        self.last_task = id;
        self.last_epoch = epoch;
        let result = self
            .wait(wire::Kind::InputAccepted, begin, &mut |_| Ok(()))
            .map(|_| ());
        self.settle(result)
    }
    fn authenticate(&mut self, mut e: wire::Event, op: &Operation) -> Result<wire::Event, Error> {
        // Already removed from the bounded receiver: authenticate/advance the
        // control cursor even if cancellation raced receive. Never read pixels
        // after the final operation check fails.
        let mac = std::mem::take(&mut e.mac);
        hmac::verify(&self.key, &e.encode_to_vec(), &mac)
            .map_err(|_| Error::AuthenticationFailed)?;
        header(&e, self.event_sequence, &self.portrait())?;
        self.event_sequence = e.sequence;
        op.check()?;
        Ok(e)
    }
    fn frame(
        &mut self,
        e: &wire::Event,
        accept: &mut impl FnMut(Frame) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let t = self.active.as_mut().ok_or(Error::ProtocolError)?;
        t.op.check()?;
        if e.kind != wire::Kind::Frame as i32
            || e.task_id != t.id
            || e.epoch != t.epoch
            || e.request_sequence != t.begin
        {
            return Err(Error::ProtocolError);
        }
        let f = e.frame.clone().ok_or(Error::ProtocolError)?;
        source_bound(
            f.source_sample.ok_or(Error::ProtocolError)?,
            t.samples,
            t.eos,
        )?;
        let (frame, release) =
            t.ingress
                .read(f, &e.mapping_name, &self.config.mapping_reader, &t.op)?;
        t.op.check()?;
        accept(frame)?;
        t.op.check()?;
        let (id, epoch) = (t.id, t.epoch);
        self.send(id, epoch, Action::Release(release))?;
        Ok(())
    }
    /// Keep frame ownership moving while the only source producer waits on HTTP.
    /// Bounded to the actual three-slot pool so source delivery cannot starve.
    pub fn poll_frames(
        &mut self,
        mut accept: impl FnMut(Frame) -> Result<(), Error>,
    ) -> Result<usize, Error> {
        let result = (|| {
            let op = self.active.as_ref().ok_or(Error::ProtocolError)?.op.clone();
            op.check()?;
            let mut frames = 0;
            for _ in 0..3 {
                let event = match self.events.try_recv() {
                    Ok(value) => self.authenticate(value?, &op)?,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return Err(Error::IoFailed),
                };
                if event.kind == wire::Kind::Failed as i32 {
                    return Err(Error::CapabilityUnavailable);
                }
                self.frame(&event, &mut accept)?;
                frames += 1;
            }
            Ok(frames)
        })();
        self.settle(result)
    }
    fn wait(
        &mut self,
        kind: wire::Kind,
        sequence: u64,
        accept: &mut impl FnMut(Frame) -> Result<(), Error>,
    ) -> Result<wire::Event, Error> {
        loop {
            let op = self.active.as_ref().ok_or(Error::ProtocolError)?.op.clone();
            let e = self.next(&op)?;
            if e.kind == wire::Kind::Failed as i32 {
                return Err(Error::CapabilityUnavailable);
            }
            if e.kind == wire::Kind::Frame as i32 {
                self.frame(&e, accept)?;
                continue;
            }
            let t = self.active.as_ref().ok_or(Error::ProtocolError)?;
            if e.task_id != t.id
                || e.epoch != t.epoch
                || e.kind != kind as i32
                || e.request_sequence != sequence
                || e.samples != t.samples
                || e.sha256 != t.hash.clone().finalize().to_vec()
            {
                return Err(Error::ProtocolError);
            }
            return Ok(e);
        }
    }
    pub fn chunk(
        &mut self,
        pcm: &[u8],
        mut accept: impl FnMut(Frame) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let result = (|| {
            let t = self.active.as_ref().ok_or(Error::ProtocolError)?;
            t.op.check()?;
            if t.eos
                || pcm.is_empty()
                || !pcm.len().is_multiple_of(2)
                || pcm.len() > 76800
                || t.samples + (pcm.len() / 2) as u64 > t.max
            {
                return Err(Error::InvalidArgument);
            }
            let (id, epoch, op, start) = (t.id, t.epoch, t.op.clone(), t.samples);
            self.payload = self
                .payload
                .checked_add(1)
                .ok_or(Error::ResourceExhausted)?;
            let chunk = wire::Chunk {
                raw_payload_id: self.payload,
                start_sample: start,
                samples: (pcm.len() / 2) as u32,
                sha256: Sha256::digest(pcm).to_vec(),
            };
            let seq = self.send(id, epoch, Action::Chunk(chunk.clone()))?;
            self.raw.send(&chunk, pcm, &op)?;
            let t = self.active.as_mut().ok_or(Error::ProtocolError)?;
            t.samples += (pcm.len() / 2) as u64;
            t.hash.update(pcm);
            let e = self.wait(wire::Kind::ChunkConsumed, seq, &mut accept)?;
            if e.raw_payload_id != self.payload {
                return Err(Error::ProtocolError);
            }
            self.raw.consumed(e.raw_payload_id)
        })();
        self.settle(result)
    }
    /// Only a trusted root after provider completion verification may call this.
    pub fn finish_verified(
        &mut self,
        samples: u64,
        sha256: &[u8],
        mut accept: impl FnMut(Frame) -> Result<(), Error>,
    ) -> Result<wire::Event, Error> {
        let result = (|| {
            let t = self.active.as_ref().ok_or(Error::ProtocolError)?;
            t.op.check()?;
            if t.eos
                || samples == 0
                || samples != t.samples
                || sha256 != t.hash.clone().finalize().as_slice()
            {
                return Err(Error::ProtocolError);
            }
            let (id, epoch, begin) = (t.id, t.epoch, t.begin);
            let seq = self.send(
                id,
                epoch,
                Action::Eos(wire::Eos {
                    samples,
                    sha256: sha256.to_vec(),
                    provider_eos_verified: true,
                }),
            )?;
            self.active.as_mut().ok_or(Error::ProtocolError)?.eos = true;
            self.wait(wire::Kind::EosVerified, seq, &mut accept)?;
            let e = self.wait(wire::Kind::RenderComplete, begin, &mut accept)?;
            let t = self.active.as_ref().ok_or(Error::ProtocolError)?;
            let stats = e.stats.as_ref().ok_or(Error::ProtocolError)?;
            let frames = (samples * 25).div_ceil(24000);
            crate::v561_avatar_metrics::validate(
                stats,
                samples,
                frames,
                self.config.frame_num,
                e.elapsed_ms,
                self.config.sample_steps.unwrap_or(4),
            )?;
            if e.frames != frames
                || t.ingress.count() != frames
                || stats.retained_frames != frames
                || stats.transferred_frames != frames
                || stats.block_frames
                    != self
                        .ready
                        .warmup
                        .as_ref()
                        .ok_or(Error::ProtocolError)?
                        .block_frames
                || stats.generated_frames != u64::from(stats.blocks) * u64::from(stats.block_frames)
                || stats.generated_frames.checked_sub(frames) != Some(stats.tail_frames)
            {
                return Err(Error::ProtocolError);
            }
            self.active = None;
            Ok(e)
        })();
        self.settle(result)
    }
    pub fn cancel(&mut self) -> Result<(), Error> {
        let Some(t) = self.active.as_ref() else {
            return Ok(());
        };
        t.op.cancel();
        let (id, epoch, begin) = (t.id, t.epoch, t.begin);
        let cleanup = Operation::new(4000)?;
        let result = (|| {
            let request = self.send(id, epoch, Action::Cancel(wire::Cancel {}))?;
            loop {
                let e = self.next(&cleanup)?;
                if e.task_id != id || e.epoch != epoch {
                    return Err(Error::ProtocolError);
                }
                match wire::Kind::try_from(e.kind).map_err(|_| Error::ProtocolError)? {
                    wire::Kind::Frame => {} // authenticated late pixels are neither read nor released
                    wire::Kind::ChunkConsumed | wire::Kind::EosVerified => {} // prior bounded operation raced cancel
                    wire::Kind::Cancelled if e.request_sequence == request => {}
                    wire::Kind::RenderCancelled if e.request_sequence == begin => {
                        if e.code != "CANCELLED" {
                            return Err(Error::CapabilityUnavailable);
                        }
                        self.raw.cancelled()?;
                        self.active = None;
                        return Ok(());
                    }
                    _ => return Err(Error::ProtocolError),
                }
            }
        })();
        if result.is_err() {
            self.abort();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_failure_is_bounded_and_allowlisted() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(preparation_failure(dir.path()), "PREPARE_FAILED");
        let path = dir.path().join("worker.startup-failure.json");
        for (code, expected) in [
            ("CUDA_PREPARATION_FAILED", "CUDA_PREPARATION_FAILED"),
            ("private exception /secret", "PREPARE_FAILED"),
        ] {
            std::fs::write(
                &path,
                serde_json::json!({"scope":"avatar_startup_failure_not_ready",
                "preparation_code":code})
                .to_string(),
            )
            .unwrap();
            assert_eq!(preparation_failure(dir.path()), expected);
        }
        std::fs::write(path, vec![b'x'; 4097]).unwrap();
        assert_eq!(preparation_failure(dir.path()), "PREPARE_FAILED");
    }
    fn event() -> wire::Event {
        wire::Event {
            version: wire::WireVersion::Current as u32,
            sequence: 1,
            kind: wire::Kind::Ready as i32,
            model_loads: 1,
            portrait_sha256: vec![8; 32],
            elapsed_ms: 30000,
            warmup: Some(nai_contracts::v561_avatar::WarmupStats {
                samples: 76800,
                frames_acked: 80,
                blocks: 2,
                unique_graphs: 13,
                generated_kernels: 300,
                elapsed_ms: 20000,
                block_frames: 40,
                motion_frames: 9,
            }),
            ..Default::default()
        }
    }
    fn config() -> Config {
        Config {
            safetensors_backend: Default::default(),
            python: "unused".into(),
            upstream_dir: "unused".into(),
            dependency_dirs: vec![],
            model_dir: "unused".into(),
            wav2vec_dir: "unused".into(),
            portrait: "unused".into(),
            portrait_sha256: "08".repeat(32),
            portrait_authorized: true,
            mapping_reader: "unused".into(),
            temp_dir: "unused".into(),
            width: 768,
            height: 768,
            frame_num: 49,
            seed: 42,
            face_ratio: 2.8,
            timeout_ms: 45000,
            maintenance_timeout_ms: 240000,
            warmup_samples: 76800,
            sample_steps: None,
        }
    }
    #[test]
    fn ready_requires_real_work_geometry_and_separate_maintenance_limit() {
        assert!(ready(&event(), &config()).is_ok());
        for mode in 0..7 {
            let mut e = event();
            let w = e.warmup.as_mut().unwrap();
            match mode {
                0 => w.samples += 1,
                1 => w.blocks = 1,
                2 => w.unique_graphs = 0,
                3 => w.generated_kernels = 0,
                4 => w.motion_frames = 0,
                5 => w.block_frames += 1,
                _ => e.elapsed_ms = 240001,
            };
            assert!(ready(&e, &config()).is_err());
        }
    }
    #[test]
    fn header_rejects_old_version_replay_wrong_identity_and_wrong_payload_kind() {
        assert!(header(&event(), 0, &[8; 32]).is_ok());
        for mode in 0..7 {
            let mut e = event();
            match mode {
                0 => e.version = 1,
                1 => e.sequence = 2,
                2 => e.model_loads = 2,
                3 => e.portrait_sha256 = vec![9; 32],
                4 => e.mapping_name = "unexpected".into(),
                5 => e.stats = Some(Default::default()),
                _ => e.kind = 77,
            };
            assert!(header(&e, 0, &[8; 32]).is_err());
        }
    }
    #[test]
    fn source_bound_uses_actual_pcm_and_only_eos_allows_a_partial_frame() {
        assert!(source_bound(0, 960, false).is_ok());
        assert!(source_bound(0, 1, false).is_err());
        assert!(source_bound(0, 1, true).is_ok());
        assert!(source_bound(960, 960, true).is_err());
        assert!(source_bound(u64::MAX, 144000, false).is_err());
    }
}

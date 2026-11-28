//! One session-private content asset. No restored task, mapping ticket or playback lease.
use crate::{
    v561_avatar_worker::Config as Avatar,
    v561_stream_synthesis::{MediaSink, Production},
};
use nai_contracts::{
    v561_media::{FrameRef, PixelFormat},
    workbench::WorkbenchError as Error,
};
use nai_llm_openai::cosyvoice::Config;
use nai_media_core::v561_frame_scheduler::Frame;
use nai_runtime_core::foundation::Operation;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub max_bytes: u64,
    pub ttl_ms: u32,
    pub max_entries: usize,
    pub max_total_bytes: u64,
    pub prefetch_max_bytes: u64,
}
impl Policy {
    pub fn validate(self) -> Result<(), Error> {
        if !(4194304..=536870912).contains(&self.max_bytes)
            || !(1000..=3600000).contains(&self.ttl_ms)
            || !(1..=8).contains(&self.max_entries)
            || self.max_total_bytes < self.max_bytes
            || self.max_total_bytes > 2147483648
            || !(4194304..=536870912).contains(&self.prefetch_max_bytes)
        {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn file_hash(path: &Path, maximum: u64, op: &Operation) -> Result<[u8; 32], Error> {
    op.check()?;
    let mut f = File::open(path).map_err(|_| Error::IoFailed)?;
    if f.metadata().map_err(|_| Error::IoFailed)?.len() > maximum {
        return Err(Error::ResourceExhausted);
    }
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    let mut total = 0;
    loop {
        op.check()?;
        let n = f.read(&mut bytes).map_err(|_| Error::IoFailed)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > maximum {
            return Err(Error::ResourceExhausted);
        }
        hash.update(&bytes[..n]);
    }
    if total == 0 {
        return Err(Error::InvalidArgument);
    }
    Ok(hash.finalize().into())
}
/// Opaque live-worker context deliberately prevents cross-session/model sharing.
pub fn identity(
    nonce: &[u8; 32],
    avatar: &Avatar,
    tts: &Config,
    text: &str,
    op: &Operation,
) -> Result<[u8; 32], Error> {
    avatar.validate()?;
    op.check()?;
    if tts.lookahead.split(text)?.len() != 1 {
        return Err(Error::InvalidArgument);
    }
    let portrait = file_hash(&avatar.portrait, 33554432, op)?;
    if hex(&portrait) != avatar.portrait_sha256 {
        return Err(Error::PermissionDenied);
    }
    let reference = file_hash(&tts.reference_file, 10485760, op)?;
    let config=serde_json::to_vec(&serde_json::json!({"avatar":avatar,"endpoint":tts.endpoint,
        "reference_path":tts.reference_file,"prompt":tts.prompt_text,"deadline":tts.deadline_ms,"max_samples":tts.max_samples,
        "sentence_chars":tts.lookahead.max_sentence_chars,"sentence_samples":tts.lookahead.max_sentence_samples})).map_err(|_|Error::ProtocolError)?;
    let mut hash = Sha256::new();
    hash.update(nonce);
    hash.update(config);
    hash.update(text.as_bytes());
    hash.update(reference);
    hash.update(portrait);
    op.check()?;
    Ok(hash.finalize().into())
}
pub struct Builder {
    file: File,
    pcm: Zeroizing<Vec<u8>>,
    hashes: Vec<[u8; 32]>,
    sealed: Option<[u8; 32]>,
    key: [u8; 32],
    epoch: u64,
    width: u32,
    height: u32,
    policy: Policy,
    op: Operation,
}
pub struct Asset {
    file: Mutex<File>,
    pcm: Zeroizing<Vec<u8>>,
    hashes: Vec<[u8; 32]>,
    digest: [u8; 32],
    key: [u8; 32],
    width: u32,
    height: u32,
    until: Instant,
    ttl_ms: u32,
    last_epoch: AtomicU64,
    busy: AtomicBool,
}
pub struct Replay {
    pub samples: u64,
    pub first_pcm_ms: u128,
    pub first_frame_ms: Option<u128>,
}
/// Private immutable content snapshot, never an epoch or execution permit.
pub struct VerifiedFrames {
    key: [u8; 32],
    content: [u8; 32],
    until: Instant,
    frames: Vec<Vec<u8>>,
}
impl Builder {
    pub fn new(
        directory: &Path,
        key: [u8; 32],
        epoch: u64,
        width: u32,
        height: u32,
        policy: Policy,
        op: Operation,
    ) -> Result<Self, Error> {
        policy.validate()?;
        op.check()?;
        if !directory.is_absolute()
            || epoch == 0
            || ![width, height]
                .iter()
                .all(|v| (256..=1024).contains(v) && v.is_multiple_of(32))
        {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            file: tempfile::tempfile_in(directory).map_err(|_| Error::IoFailed)?,
            pcm: Zeroizing::new(Vec::new()),
            hashes: vec![],
            sealed: None,
            key,
            epoch,
            width,
            height,
            policy,
            op,
        })
    }
    fn budget(&self, pcm: u64, frames: u64) -> Result<(), Error> {
        let bytes = frames
            .checked_mul(u64::from(self.width) * u64::from(self.height) * 4)
            .and_then(|v| v.checked_add(pcm))
            .ok_or(Error::ResourceExhausted)?;
        if bytes > self.policy.max_bytes {
            return Err(Error::ResourceExhausted);
        }
        Ok(())
    }
    pub fn finish(self, production: &Production) -> Result<Asset, Error> {
        self.op.check()?;
        if production.render.kind != nai_contracts::v561_avatar_stream::Kind::RenderComplete as i32
            || production.render.epoch != self.epoch
        {
            return Err(Error::ProtocolError);
        }
        let digest = self.sealed.ok_or(Error::ProtocolError)?;
        if production.provider.samples != self.pcm.len() as u64 / 2
            || self.hashes.len() as u64 != production.provider.samples.div_ceil(960)
            || production.render.sha256 != digest
            || self.file.metadata().map_err(|_| Error::IoFailed)?.len()
                != self.hashes.len() as u64 * u64::from(self.width) * u64::from(self.height) * 4
        {
            return Err(Error::ProtocolError);
        }
        self.budget(self.pcm.len() as u64, self.hashes.len() as u64)?;
        Ok(Asset {
            file: Mutex::new(self.file),
            pcm: self.pcm,
            hashes: self.hashes,
            digest,
            key: self.key,
            width: self.width,
            height: self.height,
            until: Instant::now() + Duration::from_millis(self.policy.ttl_ms.into()),
            ttl_ms: self.policy.ttl_ms,
            last_epoch: AtomicU64::new(self.epoch),
            busy: AtomicBool::new(false),
        })
    }
}
impl MediaSink for Builder {
    fn pcm(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.op.check()?;
        if self.sealed.is_some()
            || bytes.is_empty()
            || !bytes.len().is_multiple_of(2)
            || bytes.len() > 288000 - self.pcm.len()
        {
            return Err(Error::ProtocolError);
        }
        self.budget(
            (self.pcm.len() + bytes.len()) as u64,
            self.hashes.len() as u64,
        )?;
        self.pcm.extend_from_slice(bytes);
        Ok(())
    }
    fn frame(&mut self, frame: Frame) -> Result<(), Error> {
        self.op.check()?;
        let index = self.hashes.len() as u64;
        let size = u64::from(self.width) * u64::from(self.height) * 4;
        if index >= 150
            || frame.metadata.epoch != self.epoch
            || frame.metadata.sequence != Some(index)
            || frame.metadata.source_sample != Some(index * 960)
            || frame.metadata.source_sample_rate != 24000
            || frame.metadata.width != self.width
            || frame.metadata.height != self.height
            || frame.metadata.stride_bytes != self.width * 4
            || frame.metadata.pixel_format != PixelFormat::Bgra8 as i32
            || frame.metadata.payload_bytes != size
            || frame.pixels.len() as u64 != size
            || index * 960 >= self.pcm.len() as u64 / 2
            || (self.sealed.is_none() && index * 960 + 960 > self.pcm.len() as u64 / 2)
        {
            return Err(Error::ProtocolError);
        }
        let hash: [u8; 32] = Sha256::digest(&frame.pixels).into();
        if frame.metadata.content_sha256 != hash {
            return Err(Error::ProtocolError);
        }
        self.budget(self.pcm.len() as u64, index + 1)?;
        self.file
            .write_all(&frame.pixels)
            .map_err(|_| Error::IoFailed)?;
        self.hashes.push(hash);
        Ok(())
    }
    fn verified_eos(&mut self, samples: u64, digest: &[u8]) -> Result<(), Error> {
        self.op.check()?;
        let actual: [u8; 32] = Sha256::digest(self.pcm.as_slice()).into();
        if self.sealed.is_some()
            || samples == 0
            || samples != self.pcm.len() as u64 / 2
            || digest != actual
        {
            return Err(Error::ProtocolError);
        }
        self.sealed = Some(actual);
        Ok(())
    }
}
impl Asset {
    pub(crate) fn standby_image(
        &self,
        op: &Operation,
    ) -> Result<crate::v561_standby::Image, Error> {
        op.check()?;
        if !self.valid() {
            return Err(Error::DeadlineExceeded);
        }
        let mut file = loop {
            op.check()?;
            match self.file.try_lock() {
                Ok(file) => break file,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(_) => return Err(Error::IoFailed),
            }
        };
        let size = self.width as usize * self.height as usize * 4;
        if file.metadata().map_err(|_| Error::IoFailed)?.len()
            != size as u64 * self.hashes.len() as u64
        {
            return Err(Error::ProtocolError);
        }
        let digest = *self.hashes.first().ok_or(Error::ProtocolError)?;
        file.seek(SeekFrom::Start(0)).map_err(|_| Error::IoFailed)?;
        let mut bytes = vec![0; size];
        file.read_exact(&mut bytes).map_err(|_| Error::IoFailed)?;
        op.check()?;
        crate::v561_standby::Image::verified(
            self.width,
            self.height,
            &bytes,
            digest,
            self.key,
            self.until,
        )
    }
    pub(crate) fn expires_at(&self) -> Instant {
        self.until
    }
    fn content_seal(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.digest);
        hash.update(self.width.to_le_bytes());
        hash.update(self.height.to_le_bytes());
        for frame in &self.hashes {
            hash.update(frame);
        }
        hash.finalize().into()
    }
    pub fn verify_ahead(&self, maximum: u64, op: &Operation) -> Result<VerifiedFrames, Error> {
        op.check()?;
        if !self.valid() {
            return Err(Error::DeadlineExceeded);
        }
        let size = self.width as usize * self.height as usize * 4;
        let total = (size as u64)
            .checked_mul(self.hashes.len() as u64)
            .ok_or(Error::ResourceExhausted)?;
        if total > maximum {
            return Err(Error::ResourceExhausted);
        }
        let mut file = loop {
            op.check()?;
            match self.file.try_lock() {
                Ok(file) => break file,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(std::sync::TryLockError::Poisoned(_)) => return Err(Error::IoFailed),
            }
        };
        if file.metadata().map_err(|_| Error::IoFailed)?.len() != total
            || Sha256::digest(self.pcm.as_slice()).as_slice() != self.digest
        {
            return Err(Error::ProtocolError);
        }
        file.seek(SeekFrom::Start(0)).map_err(|_| Error::IoFailed)?;
        let mut frames = Vec::with_capacity(self.hashes.len());
        for digest in &self.hashes {
            op.check()?;
            if !self.valid() {
                return Err(Error::DeadlineExceeded);
            }
            let mut bytes = vec![0; size];
            file.read_exact(&mut bytes).map_err(|_| Error::IoFailed)?;
            if Sha256::digest(&bytes).as_slice() != digest {
                return Err(Error::ProtocolError);
            }
            frames.push(bytes);
        }
        op.check()?;
        Ok(VerifiedFrames {
            key: self.key,
            content: self.content_seal(),
            until: self.until,
            frames,
        })
    }
    pub fn bytes(&self) -> u64 {
        self.width as u64 * self.height as u64 * 4 * self.hashes.len() as u64
            + self.pcm.len() as u64
    }
    pub fn samples(&self) -> u64 {
        self.pcm.len() as u64 / 2
    }
    pub(crate) fn identity_key(&self) -> [u8; 32] {
        self.key
    }
    pub fn valid(&self) -> bool {
        Instant::now() < self.until
    }
    pub fn matches(&self, key: &[u8; 32]) -> bool {
        self.valid() && key == &self.key
    }
    pub fn manifest(&self) -> serde_json::Value {
        serde_json::json!({"version":1,"scope":"private_session_prepared_asset_not_current_model_cache",
        "identity_sha256":hex(&self.key),"pcm_sha256":hex(&self.digest),"frame_sha256":self.hashes.iter().map(|h|hex(h)).collect::<Vec<_>>(),
        "width":self.width,"height":self.height,"samples":self.samples(),"frames":self.hashes.len(),"ttl_ms":self.ttl_ms,"model_api_calls":1})
    }
    pub fn replay(
        &self,
        key: &[u8; 32],
        epoch: u64,
        op: &Operation,
        sink: &mut dyn MediaSink,
        capacity: impl FnMut() -> Result<(), Error>,
    ) -> Result<Replay, Error> {
        self.replay_with_verified(key, epoch, op, sink, capacity, None)
    }
    pub fn replay_with_verified(
        &self,
        key: &[u8; 32],
        epoch: u64,
        op: &Operation,
        sink: &mut dyn MediaSink,
        mut capacity: impl FnMut() -> Result<(), Error>,
        verified: Option<VerifiedFrames>,
    ) -> Result<Replay, Error> {
        op.check()?;
        if !self.matches(key) || epoch == 0 {
            return Err(Error::PermissionDenied);
        }
        if verified.as_ref().is_some_and(|v| {
            v.key != self.key
                || v.content != self.content_seal()
                || Instant::now() >= v.until
                || v.frames.len() != self.hashes.len()
        }) {
            return Err(Error::PermissionDenied);
        }
        self.last_epoch
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (epoch > old).then_some(epoch)
            })
            .map_err(|_| Error::ProtocolError)?;
        if self.busy.swap(true, Ordering::AcqRel) {
            return Err(Error::ResourceExhausted);
        }
        struct Busy<'a>(&'a AtomicBool);
        impl Drop for Busy<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release)
            }
        }
        let _busy = Busy(&self.busy);
        let began = Instant::now();
        let size = self.width as usize * self.height as usize * 4;
        let mut file = self.file.lock().map_err(|_| Error::IoFailed)?;
        // Verify the entire anonymous content before admitting any sample.
        if file.metadata().map_err(|_| Error::IoFailed)?.len()
            != size as u64 * self.hashes.len() as u64
            || Sha256::digest(self.pcm.as_slice()).as_slice() != self.digest
        {
            return Err(Error::ProtocolError);
        }
        if verified.is_none() {
            file.seek(SeekFrom::Start(0)).map_err(|_| Error::IoFailed)?;
            let mut pixels = vec![0; size];
            for digest in &self.hashes {
                op.check()?;
                if !self.valid() {
                    return Err(Error::DeadlineExceeded);
                }
                file.read_exact(&mut pixels).map_err(|_| Error::IoFailed)?;
                if Sha256::digest(&pixels).as_slice() != digest {
                    return Err(Error::ProtocolError);
                }
            }
        }
        let mut first = None;
        let mut sent = 0;
        for bytes in self.pcm.chunks(9600) {
            op.check()?;
            sink.pcm(bytes)?;
            sent += bytes.len();
        }
        op.check()?;
        sink.verified_eos(sent as u64 / 2, &self.digest)?;
        let pcm_at = began.elapsed().as_millis();
        file.seek(SeekFrom::Start(0)).map_err(|_| Error::IoFailed)?;
        let mut prepared = verified.map(|v| v.frames.into_iter());
        let pool = format!("prepared-{}-{epoch}", hex(&self.key));
        for (index, digest) in self.hashes.iter().enumerate() {
            op.check()?;
            if !self.valid() {
                return Err(Error::DeadlineExceeded);
            }
            capacity()?;
            op.check()?;
            let bytes = if let Some(frames) = prepared.as_mut() {
                frames.next().ok_or(Error::ProtocolError)?
            } else {
                let mut bytes = vec![0; size];
                file.read_exact(&mut bytes).map_err(|_| Error::IoFailed)?;
                bytes
            };
            let sample = index as u64 * 960;
            let frame = Frame {
                metadata: FrameRef {
                    pool_id: pool.clone(),
                    epoch,
                    sequence: Some(index as u64),
                    source_sample: Some(sample),
                    source_sample_rate: 24000,
                    pts_ns: Some(sample * 1_000_000_000 / 24000),
                    width: self.width,
                    height: self.height,
                    stride_bytes: self.width * 4,
                    pixel_format: PixelFormat::Bgra8 as i32,
                    slot_index: Some(index as u32 % 3),
                    slot_generation: index as u64 / 3 + 1,
                    payload_bytes: size as u64,
                    content_sha256: digest.to_vec(),
                },
                pixels: bytes,
            };
            let verified = nai_media_core::v561_frame_scheduler::OwnedVerifiedFrame::verify(
                frame,
                nai_contracts::v561_media_validation::FrameLimits {
                    slots: 3,
                    max_width: self.width,
                    max_height: self.height,
                    max_payload_bytes: size as u64,
                    max_sample_rate: 24000,
                },
            )
            .map_err(|_| Error::ProtocolError)?;
            sink.verified_frame(verified)?;
            first.get_or_insert(began.elapsed().as_millis());
        }
        op.check()?;
        Ok(Replay {
            samples: self.samples(),
            first_pcm_ms: pcm_at,
            first_frame_ms: first,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn with_expiry(until: Instant) -> Asset {
        let mut a = asset();
        a.until = until;
        a
    }
    #[test]
    fn standby_read_rejects_corruption_expiry_and_cancel() {
        let op = Operation::model_task(1000).unwrap();
        let a = asset();
        assert!(a.standby_image(&op).is_ok());
        a.file.lock().unwrap().seek(SeekFrom::Start(0)).unwrap();
        a.file.lock().unwrap().write_all(&[99]).unwrap();
        assert!(a.standby_image(&op).is_err());
        let mut a = asset();
        a.until = Instant::now();
        assert!(a.standby_image(&op).is_err());
        let a = asset();
        op.cancel();
        assert!(matches!(a.standby_image(&op), Err(Error::Cancelled)));
    }
    #[test]
    fn verified_snapshot_is_bounded_owned_and_requires_fresh_binding() {
        let mut a = asset();
        let op = Operation::model_task(1000).unwrap();
        assert!(matches!(
            a.verify_ahead(1, &op),
            Err(Error::ResourceExhausted)
        ));
        let snapshot = a.verify_ahead(4194304, &op).unwrap();
        // The source is private; a verified snapshot must keep the exact old
        // pixels rather than re-read a subsequently modified source file.
        a.file.lock().unwrap().seek(SeekFrom::Start(0)).unwrap();
        a.file.lock().unwrap().write_all(&[99]).unwrap();
        let mut received = Received::default();
        a.replay_with_verified(&[9; 32], 9, &op, &mut received, || Ok(()), Some(snapshot))
            .unwrap();
        assert_eq!(
            received.hashes[0],
            Sha256::digest(vec![0; 256 * 256 * 4]).as_slice()
        );
        assert!(a.verify_ahead(4194304, &op).is_err());
        let other = asset();
        let token = other.verify_ahead(4194304, &op).unwrap();
        a.key = [8; 32];
        let mut none = Received::default();
        assert!(
            a.replay_with_verified(&[8; 32], 10, &op, &mut none, || Ok(()), Some(token))
                .is_err()
        );
        assert!(none.pcm.is_empty());
        let mut changed = asset();
        let token = changed.verify_ahead(4194304, &op).unwrap();
        changed.pcm[0] = 2;
        changed.digest = Sha256::digest(changed.pcm.as_slice()).into();
        assert!(
            changed
                .replay_with_verified(&[9; 32], 9, &op, &mut none, || Ok(()), Some(token))
                .is_err()
        );
        assert!(none.pcm.is_empty());
        let mut b = asset();
        let token = b.verify_ahead(4194304, &op).unwrap();
        b.until = Instant::now();
        assert!(
            b.replay_with_verified(&[9; 32], 9, &op, &mut none, || Ok(()), Some(token))
                .is_err()
        );
        let c = asset();
        let token = c.verify_ahead(4194304, &op).unwrap();
        op.cancel();
        assert!(
            c.replay_with_verified(&[9; 32], 9, &op, &mut none, || Ok(()), Some(token))
                .is_err()
        );
        assert!(none.pcm.is_empty());
        assert!(c.verify_ahead(4194304, &op).is_err());
    }
    #[test]
    fn waiting_for_private_file_lock_can_be_cancelled_without_publication() {
        let asset = std::sync::Arc::new(asset());
        let op = Operation::model_task(1000).unwrap();
        let locked = asset.file.lock().unwrap();
        let pending = asset.clone();
        let child = op.clone();
        let thread = std::thread::spawn(move || pending.verify_ahead(4194304, &child));
        std::thread::sleep(Duration::from_millis(10));
        let began = Instant::now();
        op.cancel();
        assert!(thread.join().unwrap().is_err());
        assert!(began.elapsed() < Duration::from_millis(250));
        drop(locked);
    }
    #[derive(Default)]
    struct Received {
        pcm: Vec<u8>,
        frames: Vec<FrameRef>,
        hashes: Vec<[u8; 32]>,
    }
    impl MediaSink for Received {
        fn pcm(&mut self, p: &[u8]) -> Result<(), Error> {
            self.pcm.extend_from_slice(p);
            Ok(())
        }
        fn verified_eos(&mut self, n: u64, h: &[u8]) -> Result<(), Error> {
            assert_eq!(n, self.pcm.len() as u64 / 2);
            assert_eq!(h, Sha256::digest(&self.pcm).as_slice());
            Ok(())
        }
        fn frame(&mut self, f: Frame) -> Result<(), Error> {
            self.hashes.push(Sha256::digest(&f.pixels).into());
            self.frames.push(f.metadata);
            Ok(())
        }
    }
    fn frame(n: u64, width: u32, height: u32) -> Frame {
        let pixels = vec![n as u8; width as usize * height as usize * 4];
        Frame {
            metadata: FrameRef {
                epoch: 8,
                sequence: Some(n),
                source_sample: Some(n * 960),
                source_sample_rate: 24000,
                width,
                height,
                stride_bytes: width * 4,
                pixel_format: PixelFormat::Bgra8 as i32,
                payload_bytes: pixels.len() as u64,
                content_sha256: Sha256::digest(&pixels).to_vec(),
                ..Default::default()
            },
            pixels,
        }
    }
    pub(crate) fn asset() -> Asset {
        let dir = tempfile::tempdir().unwrap();
        let op = Operation::model_task(1000).unwrap();
        let mut b = Builder::new(
            dir.path(),
            [9; 32],
            8,
            256,
            256,
            Policy {
                max_bytes: 4194304,
                max_entries: 4,
                max_total_bytes: 16777216,
                prefetch_max_bytes: 4194304,
                ttl_ms: 60000,
            },
            op,
        )
        .unwrap();
        let pcm = vec![1; 4000];
        let hash = Sha256::digest(&pcm);
        b.pcm(&pcm).unwrap();
        b.verified_eos(2000, &hash).unwrap();
        for n in 0..3 {
            b.frame(frame(n, 256, 256)).unwrap();
        }
        b.finish(&Production {
            provider: nai_llm_openai::cosyvoice::Summary {
                samples: 2000,
                first_pcm_ms: 1,
                complete_ms: 2,
                transport_eof_ms: 2,
            },
            render: nai_contracts::v561_avatar_stream::Event {
                kind: nai_contracts::v561_avatar_stream::Kind::RenderComplete as i32,
                epoch: 8,
                sha256: hash.to_vec(),
                ..Default::default()
            },
            first_pcm_ms: 1,
            provider_verified_ms: 2,
            first_frame_ms: Some(3),
            completed_ms: 4,
        })
        .unwrap()
    }
    #[test]
    fn two_replays_keep_content_but_create_new_epoch_and_logical_frames() {
        let a = asset();
        let mut first = Received::default();
        let mut second = Received::default();
        a.replay(
            &[9; 32],
            9,
            &Operation::model_task(1000).unwrap(),
            &mut first,
            || Ok(()),
        )
        .unwrap();
        a.replay(
            &[9; 32],
            10,
            &Operation::model_task(1000).unwrap(),
            &mut second,
            || Ok(()),
        )
        .unwrap();
        assert_eq!(first.pcm, second.pcm);
        assert_eq!(first.hashes, second.hashes);
        assert!(first.frames.iter().all(|f| f.epoch == 9));
        assert!(second.frames.iter().all(|f| f.epoch == 10));
        assert_ne!(first.frames[0].pool_id, second.frames[0].pool_id);
        assert!(
            a.replay(
                &[9; 32],
                10,
                &Operation::model_task(1000).unwrap(),
                &mut Received::default(),
                || Ok(())
            )
            .is_err()
        );
    }
    #[test]
    fn changed_identity_expiry_and_corruption_reject_before_any_audio() {
        for kind in 0..3 {
            let mut a = asset();
            let mut out = Received::default();
            let key = if kind == 0 { [8; 32] } else { [9; 32] };
            if kind == 1 {
                a.until = Instant::now() - Duration::from_secs(1)
            }
            if kind == 2 {
                let mut f = a.file.lock().unwrap();
                f.seek(SeekFrom::Start(0)).unwrap();
                f.write_all(&[255]).unwrap();
            }
            assert!(
                a.replay(
                    &key,
                    9,
                    &Operation::model_task(1000).unwrap(),
                    &mut out,
                    || Ok(())
                )
                .is_err()
            );
            assert!(out.pcm.is_empty());
            assert!(out.frames.is_empty());
        }
    }
    #[test]
    fn cancelled_delivery_has_no_late_frames_but_content_can_be_explicitly_rebound() {
        let a = asset();
        let op = Operation::model_task(1000).unwrap();
        let mut out = Received::default();
        assert!(matches!(
            a.replay(&[9; 32], 9, &op, &mut out, || {
                op.cancel();
                Ok(())
            }),
            Err(Error::Cancelled)
        ));
        assert!(out.frames.is_empty());
        a.replay(
            &[9; 32],
            10,
            &Operation::model_task(1000).unwrap(),
            &mut Received::default(),
            || Ok(()),
        )
        .unwrap();
    }
    #[test]
    fn byte_budget_missing_eos_bad_hash_and_cancel_cannot_publish() {
        let dir = tempfile::tempdir().unwrap();
        let op = Operation::model_task(1000).unwrap();
        op.cancel();
        assert!(
            Builder::new(
                dir.path(),
                [9; 32],
                8,
                256,
                256,
                Policy {
                    max_bytes: 4194304,
                    max_entries: 4,
                    max_total_bytes: 16777216,
                    prefetch_max_bytes: 4194304,
                    ttl_ms: 1000
                },
                op
            )
            .is_err()
        );
        let mut b = Builder::new(
            dir.path(),
            [9; 32],
            8,
            1024,
            1024,
            Policy {
                max_bytes: 4194304,
                max_entries: 4,
                max_total_bytes: 16777216,
                prefetch_max_bytes: 4194304,
                ttl_ms: 1000,
            },
            Operation::model_task(1000).unwrap(),
        )
        .unwrap();
        b.pcm(&vec![0; 1920]).unwrap();
        assert!(matches!(
            b.frame(frame(0, 1024, 1024)),
            Err(Error::ResourceExhausted)
        ));
        assert!(b.verified_eos(960, &[1; 32]).is_err());
        let mut f = frame(1, 1024, 1024);
        f.metadata.sequence = Some(0);
        assert!(b.frame(f).is_err());
        let digest = Sha256::digest(b.pcm.as_slice());
        let incomplete = Production {
            provider: nai_llm_openai::cosyvoice::Summary {
                samples: 960,
                first_pcm_ms: 1,
                complete_ms: 2,
                transport_eof_ms: 2,
            },
            render: nai_contracts::v561_avatar_stream::Event {
                kind: nai_contracts::v561_avatar_stream::Kind::RenderComplete as i32,
                epoch: 8,
                sha256: digest.to_vec(),
                ..Default::default()
            },
            first_pcm_ms: 1,
            provider_verified_ms: 2,
            first_frame_ms: None,
            completed_ms: 3,
        };
        assert!(
            b.finish(&incomplete).is_err(),
            "missing verified EOS must never publish"
        );
    }
    #[test]
    fn same_path_reference_changes_and_permission_revoke_do_not_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        for name in ["python.exe", "reader.exe", "portrait.jpg", "reference.wav"] {
            std::fs::write(path.join(name), b"original").unwrap();
        }
        let mut avatar = Avatar {
            python: path.join("python.exe"),
            upstream_dir: path.into(),
            dependency_dirs: vec![path.into()],
            model_dir: path.into(),
            wav2vec_dir: path.into(),
            portrait: path.join("portrait.jpg"),
            portrait_sha256: hex(&Sha256::digest(b"original")),
            portrait_authorized: true,
            mapping_reader: path.join("reader.exe"),
            temp_dir: path.into(),
            width: 768,
            height: 768,
            frame_num: 41,
            seed: 42,
            face_ratio: 2.8,
            timeout_ms: 45000,
            maintenance_timeout_ms: 240000,
            warmup_samples: 76800,
            sample_steps: Some(4),
            safetensors_backend: Default::default(),
        };
        let cfg = Config {
            endpoint: "http://127.0.0.1:50000/inference_zero_shot".into(),
            reference_file: path.join("reference.wav"),
            prompt_text: "<|endofprompt|>参考".into(),
            deadline_ms: 45000,
            max_samples: 144000,
            lookahead: Default::default(),
        };
        let op = Operation::model_task(1000).unwrap();
        let original = identity(&[1; 32], &avatar, &cfg, "测试。", &op).unwrap();
        assert_ne!(
            original,
            identity(&[2; 32], &avatar, &cfg, "测试。", &op).unwrap()
        );
        std::fs::write(&cfg.reference_file, b"different").unwrap();
        assert_ne!(
            original,
            identity(&[1; 32], &avatar, &cfg, "测试。", &op).unwrap()
        );
        avatar.portrait_authorized = false;
        assert!(identity(&[1; 32], &avatar, &cfg, "测试。", &op).is_err());
    }
}

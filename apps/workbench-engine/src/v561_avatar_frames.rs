//! Authenticated metadata is supplied by the worker owner; mapping access is read-only.
//! A named mapping is not an ACL/handle-broker certification.
use nai_contracts::{
    v561_media::{FrameRef, FrameRelease},
    v561_media_validation::{FrameLimits, validate_frame},
    workbench::WorkbenchError as Error,
};
use nai_media_core::v561_frame_scheduler::Frame;
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

struct Reader {
    child: Child,
    tx: mpsc::SyncSender<(u64, u64)>,
    rx: mpsc::Receiver<Result<Vec<u8>, Error>>,
}
impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Reader {
    fn start(binary: &Path, name: &str, bytes: u64) -> Result<Self, Error> {
        let mut command = Command::new(binary);
        command.env_clear();
        for key in ["SystemRoot", "WINDIR"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command
            .arg(name)
            .arg(bytes.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| Error::IoFailed)?;
        let mut input = child.stdin.take().ok_or(Error::IoFailed)?;
        let mut output = child.stdout.take().ok_or(Error::IoFailed)?;
        let (tx, requests) = mpsc::sync_channel::<(u64, u64)>(1);
        let (results, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            for (offset, size) in requests {
                let result = (|| {
                    input
                        .write_all(&offset.to_le_bytes())
                        .and_then(|_| input.write_all(&size.to_le_bytes()))
                        .and_then(|_| input.flush())
                        .map_err(|_| Error::IoFailed)?;
                    let mut pixels = vec![0; size as usize];
                    output
                        .read_exact(&mut pixels)
                        .map_err(|_| Error::IoFailed)?;
                    Ok(pixels)
                })();
                let failed = result.is_err();
                if results.send(result).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self { child, tx, rx })
    }
    fn read(&self, offset: u64, size: u64, op: &Operation) -> Result<Vec<u8>, Error> {
        op.check()?;
        self.tx
            .try_send((offset, size))
            .map_err(|_| Error::ResourceExhausted)?;
        let start = Instant::now();
        loop {
            op.check()?;
            if start.elapsed() > Duration::from_secs(1) {
                return Err(Error::DeadlineExceeded);
            }
            match self.rx.recv_timeout(Duration::from_millis(10)) {
                Ok(value) => {
                    op.check()?;
                    return value;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(Error::IoFailed),
            }
        }
    }
}

pub struct Ingress {
    epoch: u64,
    samples: u64,
    width: u32,
    height: u32,
    pool: Option<(String, String)>,
    count: u64,
    reader: Option<Reader>,
}
impl Ingress {
    pub fn new(epoch: u64, samples: u64, width: u32, height: u32) -> Result<Self, Error> {
        if epoch == 0
            || !(1..=144000).contains(&samples)
            || ![width, height].iter().all(|n| (256..=1024).contains(n))
        {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            epoch,
            samples,
            width,
            height,
            pool: None,
            count: 0,
            reader: None,
        })
    }
    pub fn count(&self) -> u64 {
        self.count
    }
    fn validate(&self, frame: &FrameRef, name: &str) -> Result<(), Error> {
        let bytes = u64::from(self.width) * u64::from(self.height) * 4;
        let limits = FrameLimits {
            slots: 3,
            max_width: self.width,
            max_height: self.height,
            max_payload_bytes: bytes,
            max_sample_rate: 24000,
        };
        validate_frame(frame, limits).map_err(|_| Error::ProtocolError)?;
        if name.is_empty()
            || name.len() > 128
            || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || frame.pool_id.len() != 32
            || !frame.pool_id.bytes().all(|c| c.is_ascii_hexdigit())
            || frame.epoch != self.epoch
            || frame.sequence != Some(self.count)
            || self.count >= (self.samples * 25).div_ceil(24000)
            || frame.source_sample != Some(self.count * 960)
            || frame.source_sample_rate != 24000
            || frame.pts_ns != Some(self.count * 40000000)
            || frame.width != self.width
            || frame.height != self.height
            || frame.payload_bytes != bytes
            || frame.content_sha256.len() != 32
            || frame.slot_index != Some((self.count % 3) as u32)
            || frame.slot_generation != self.count / 3 + 1
            || self
                .pool
                .as_ref()
                .is_some_and(|(pool, mapping)| pool != &frame.pool_id || mapping != name)
        {
            return Err(Error::ProtocolError);
        }
        Ok(())
    }
    fn own(
        &mut self,
        metadata: FrameRef,
        name: &str,
        pixels: Vec<u8>,
    ) -> Result<(Frame, FrameRelease), Error> {
        self.validate(&metadata, name)?;
        if pixels.len() as u64 != metadata.payload_bytes
            || Sha256::digest(&pixels).as_slice() != metadata.content_sha256
        {
            return Err(Error::ProtocolError);
        }
        let release = FrameRelease {
            pool_id: metadata.pool_id.clone(),
            epoch: self.epoch,
            sequence: metadata.sequence,
            slot_index: metadata.slot_index,
            slot_generation: metadata.slot_generation,
        };
        self.pool = Some((metadata.pool_id.clone(), name.to_owned()));
        self.count += 1;
        Ok((Frame { metadata, pixels }, release))
    }
    pub fn read(
        &mut self,
        metadata: FrameRef,
        name: &str,
        binary: &Path,
        op: &Operation,
    ) -> Result<(Frame, FrameRelease), Error> {
        op.check()?;
        self.validate(&metadata, name)?;
        if self.reader.is_none() {
            self.reader = Some(Reader::start(binary, name, metadata.payload_bytes * 3)?);
        }
        let pixels = self.reader.as_ref().ok_or(Error::IoFailed)?.read(
            u64::from(metadata.slot_index.ok_or(Error::ProtocolError)?) * metadata.payload_bytes,
            metadata.payload_bytes,
            op,
        )?;
        op.check()?;
        self.own(metadata, name, pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> FrameRef {
        FrameRef {
            pool_id: "a".repeat(32),
            epoch: 7,
            sequence: Some(0),
            source_sample: Some(0),
            source_sample_rate: 24000,
            pts_ns: Some(0),
            width: 256,
            height: 256,
            stride_bytes: 1024,
            pixel_format: 1,
            slot_index: Some(0),
            slot_generation: 1,
            payload_bytes: 262144,
            content_sha256: Sha256::digest(vec![0; 262144]).to_vec(),
        }
    }
    #[test]
    fn wrong_epoch_sequence_mapping_and_slot_are_rejected() {
        let mut ingress = Ingress::new(7, 24000, 256, 256).unwrap();
        let original = frame();
        for mode in 0..4 {
            let mut f = original.clone();
            match mode {
                0 => f.epoch = 8,
                1 => f.sequence = Some(1),
                2 => f.slot_generation = 2,
                _ => f.source_sample = Some(960),
            };
            assert!(ingress.validate(&f, "wnsm_valid").is_err());
        }
        assert!(ingress.validate(&original, "Global\\other").is_err());
        ingress
            .own(original.clone(), "wnsm_valid", vec![0; 262144])
            .unwrap();
        assert!(ingress.validate(&original, "wnsm_valid").is_err());
    }
    #[test]
    fn changed_pixels_never_advance_or_release() {
        let mut ingress = Ingress::new(7, 24000, 256, 256).unwrap();
        assert!(ingress.own(frame(), "wnsm_valid", vec![1; 262144]).is_err());
        assert_eq!(ingress.count(), 0);
    }
}

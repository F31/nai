//! Task-local played PCM source, not a persistent cache or playback permission.
use nai_contracts::{v561_avatar::Render, workbench::WorkbenchError as Error};
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::{io::Write, path::Path};

pub struct Source {
    file: tempfile::NamedTempFile,
    operation: Operation,
    maximum: u64,
    samples: u64,
    hash: Sha256,
    sealed: Option<(u64, Vec<u8>)>,
    failed: bool,
    started: bool,
}
impl Source {
    pub fn new(directory: &Path, maximum: u64, operation: Operation) -> Result<Self, Error> {
        operation.check()?;
        if !directory.is_absolute() || !(1..=144000).contains(&maximum) {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            file: tempfile::NamedTempFile::new_in(directory).map_err(|_| Error::IoFailed)?,
            operation,
            maximum,
            samples: 0,
            hash: Sha256::new(),
            sealed: None,
            failed: false,
            started: false,
        })
    }
    pub(crate) fn begin(&mut self, maximum: u64) -> Result<(), Error> {
        self.operation.check()?;
        if self.started
            || self.failed
            || self.sealed.is_some()
            || self.samples != 0
            || maximum == 0
            || maximum > self.maximum
        {
            self.failed = true;
            return Err(Error::InvalidArgument);
        }
        self.started = true;
        Ok(())
    }
    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.operation.check()?;
        if self.failed
            || self.sealed.is_some()
            || bytes.is_empty()
            || !bytes.len().is_multiple_of(2)
        {
            self.failed = true;
            return Err(Error::InvalidArgument);
        }
        let count = (bytes.len() / 2) as u64;
        if count > self.maximum - self.samples {
            self.failed = true;
            return Err(Error::ResourceExhausted);
        }
        if self.file.write_all(bytes).is_err() {
            self.failed = true;
            return Err(Error::IoFailed);
        }
        self.hash.update(bytes);
        self.samples += count;
        Ok(())
    }
    /// Called only after the authenticated device completed and provider EOS verified.
    pub(crate) fn seal(&mut self, epoch: u64, samples: u64, digest: &[u8]) -> Result<(), Error> {
        self.operation.check()?;
        if self.failed
            || self.sealed.is_some()
            || epoch == 0
            || samples == 0
            || samples != self.samples
            || digest != self.hash.clone().finalize().as_slice()
        {
            self.failed = true;
            return Err(Error::ProtocolError);
        }
        if self.file.flush().is_err() {
            self.failed = true;
            return Err(Error::IoFailed);
        }
        self.sealed = Some((epoch, digest.to_vec()));
        Ok(())
    }
    pub fn render(&self) -> Result<(u64, Render), Error> {
        self.operation.check()?;
        if self.failed {
            return Err(Error::ProtocolError);
        }
        let (epoch, digest) = self.sealed.as_ref().ok_or(Error::CapabilityUnavailable)?;
        Ok((
            *epoch,
            Render {
                pcm_path: self
                    .file
                    .path()
                    .to_str()
                    .ok_or(Error::InvalidArgument)?
                    .to_owned(),
                pcm_sha256: digest.clone(),
                samples: self.samples,
                sample_rate: 24000,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_cannot_be_reused_or_oversubscribed() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = Source::new(dir.path(), 24, Operation::model_task(1000).unwrap()).unwrap();
        source.begin(24).unwrap();
        source.append(&[1, 0]).unwrap();
        source.seal(7, 1, &Sha256::digest([1, 0])).unwrap();
        assert!(source.begin(24).is_err());
        assert!(source.render().is_err());
        let mut source = Source::new(dir.path(), 24, Operation::model_task(1000).unwrap()).unwrap();
        assert!(source.begin(25).is_err());
        assert!(source.begin(24).is_err());
    }
    #[test]
    fn only_verified_source_is_available_and_drop_removes_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = Source::new(dir.path(), 24, Operation::model_task(1000).unwrap()).unwrap();
        let path = source.file.path().to_owned();
        assert!(source.render().is_err());
        source.append(&[1, 0, 2, 0]).unwrap();
        source.seal(7, 2, &Sha256::digest([1, 0, 2, 0])).unwrap();
        let (epoch, render) = source.render().unwrap();
        assert_eq!((epoch, render.samples), (7, 2));
        assert_eq!(std::fs::read(&render.pcm_path).unwrap(), [1, 0, 2, 0]);
        drop(source);
        assert!(!path.exists());
    }
    #[test]
    fn identity_failure_is_terminal() {
        let dir = tempfile::tempdir().unwrap();
        for wrong_samples in [false, true] {
            let mut source =
                Source::new(dir.path(), 24, Operation::model_task(1000).unwrap()).unwrap();
            source.append(&[1, 0]).unwrap();
            let digest = if wrong_samples {
                Sha256::digest([1, 0]).to_vec()
            } else {
                vec![0; 32]
            };
            assert!(
                source
                    .seal(7, if wrong_samples { 2 } else { 1 }, &digest)
                    .is_err()
            );
            assert!(source.seal(7, 1, &Sha256::digest([1, 0])).is_err());
            assert!(source.render().is_err());
        }
    }
    #[test]
    fn cancellation_revokes_even_sealed_source() {
        let dir = tempfile::tempdir().unwrap();
        let op = Operation::model_task(1000).unwrap();
        let mut source = Source::new(dir.path(), 24, op.clone()).unwrap();
        source.append(&[1, 0]).unwrap();
        source.seal(7, 1, &Sha256::digest([1, 0])).unwrap();
        op.cancel();
        assert_eq!(source.render(), Err(Error::Cancelled));
    }
    #[test]
    fn odd_and_overflow_inputs_never_publish() {
        let dir = tempfile::tempdir().unwrap();
        for bytes in [vec![1], vec![1; 4]] {
            let mut source =
                Source::new(dir.path(), 1, Operation::model_task(1000).unwrap()).unwrap();
            assert!(source.append(&bytes).is_err());
            assert!(source.render().is_err());
        }
    }
}

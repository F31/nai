//! Finite source reservoir. Submitted samples are not device-played samples.
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Full,
    Order,
    Underrun,
    Cancelled,
}

pub struct PcmGate {
    operation: Operation,
    bytes: Zeroizing<Vec<u8>>,
    hash: Sha256,
    maximum: u64,
    prefill: u64,
    covered: u64,
    next_frame: u64,
    submitted: u64,
    sealed: bool,
    started: bool,
}
impl PcmGate {
    pub fn new(maximum: u64, prefill: u64, operation: Operation) -> Result<Self, Error> {
        if maximum == 0 || maximum > 144000 || prefill == 0 || prefill > maximum {
            return Err(Error::Invalid);
        }
        Ok(Self {
            operation,
            bytes: Zeroizing::new(Vec::new()),
            hash: Sha256::new(),
            maximum,
            prefill,
            covered: 0,
            next_frame: 0,
            submitted: 0,
            sealed: false,
            started: false,
        })
    }
    fn check(&mut self) -> Result<(), Error> {
        if self.operation.check().is_err() {
            self.bytes.zeroize();
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    fn fail<T>(&mut self, error: Error) -> Result<T, Error> {
        self.operation.cancel();
        self.bytes.zeroize();
        Err(error)
    }
    pub fn received(&self) -> u64 {
        self.bytes.len() as u64 / 2
    }
    pub fn maximum(&self) -> u64 {
        self.maximum
    }
    pub fn submitted(&self) -> u64 {
        self.submitted
    }
    pub fn append(&mut self, pcm: &[u8]) -> Result<(), Error> {
        self.check()?;
        if self.sealed || pcm.is_empty() || !pcm.len().is_multiple_of(2) {
            return self.fail(Error::Invalid);
        }
        if pcm.len() as u64 / 2 > self.maximum - self.received() {
            return self.fail(Error::Full);
        }
        self.hash.update(pcm);
        self.bytes.extend_from_slice(pcm);
        Ok(())
    }
    /// Called only after source identity and pixel digest were checked by owner.
    pub fn frame(&mut self, source_sample: u64) -> Result<(), Error> {
        self.check()?;
        if source_sample != self.next_frame
            || source_sample >= self.received()
            || (!self.sealed && source_sample + 960 > self.received())
        {
            return self.fail(Error::Order);
        }
        self.next_frame += 960;
        self.covered = self.next_frame.min(self.received());
        Ok(())
    }
    pub fn seal(&mut self, samples: u64, digest: &[u8]) -> Result<(), Error> {
        self.check()?;
        if self.sealed
            || samples == 0
            || samples != self.received()
            || digest != self.hash.clone().finalize().as_slice()
        {
            return self.fail(Error::Invalid);
        }
        self.sealed = true;
        Ok(())
    }
    /// `required` means an actual device demand, never a speculative poll.
    /// Waiting for initial prefill returns None. After start, a short demand is
    /// terminal unless this is the verified final source tail.
    pub fn take(&mut self, maximum: u64, required: bool) -> Result<Option<Vec<u8>>, Error> {
        self.check()?;
        if maximum == 0 || maximum > self.maximum {
            return self.fail(Error::Invalid);
        }
        let final_coverage = self.sealed && self.covered == self.received();
        if !self.started {
            if self.covered < self.prefill && !final_coverage {
                return Ok(None);
            }
            self.started = true;
        }
        let available = self.covered - self.submitted;
        if available < maximum && required && !final_coverage {
            return self.fail(Error::Underrun);
        }
        if available == 0 {
            return Ok(None);
        }
        let count = available.min(maximum);
        let start = self.submitted as usize * 2;
        let end = start + count as usize * 2;
        let result = self.bytes[start..end].to_vec();
        self.bytes[start..end].zeroize();
        self.submitted += count;
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiny_cached_tail_uses_its_actual_bound_not_a_full_transport_chunk() {
        let mut g = gate(100, 100);
        let pcm = vec![1; 200];
        g.append(&pcm).unwrap();
        g.seal(100, &Sha256::digest(&pcm)).unwrap();
        g.frame(0).unwrap();
        assert_eq!(g.take(4800.min(g.maximum()), false).unwrap(), Some(pcm));
    }
    fn gate(max: u64, prefill: u64) -> PcmGate {
        PcmGate::new(max, prefill, Operation::model_task(1000).unwrap()).unwrap()
    }
    #[test]
    fn prefill_and_visual_frontier_bound_audio_without_clock_invention() {
        let mut g = gate(4000, 1920);
        let pcm = vec![7; 6000];
        g.append(&pcm).unwrap();
        assert!(g.take(960, false).unwrap().is_none());
        g.frame(0).unwrap();
        assert!(g.take(960, true).unwrap().is_none());
        g.frame(960).unwrap();
        assert_eq!(g.take(1920, true).unwrap().unwrap(), pcm[..3840]);
        assert!(g.take(960, false).unwrap().is_none());
        assert_eq!(g.submitted(), 1920);
        assert!(g.bytes[..3840].iter().all(|b| *b == 0));
        g.seal(3000, &Sha256::digest(&pcm)).unwrap();
    }
    #[test]
    fn real_underrun_cancels_and_prevents_late_recovery() {
        let mut g = gate(4000, 960);
        g.append(&vec![0; 6000]).unwrap();
        g.frame(0).unwrap();
        g.take(960, true).unwrap();
        assert_eq!(g.take(1, true), Err(Error::Underrun));
        assert_eq!(g.frame(960), Err(Error::Cancelled));
        assert!(g.bytes.iter().all(|b| *b == 0));
    }
    #[test]
    fn short_final_partial_frame_requires_verified_eos() {
        let pcm = vec![3; 200];
        let mut g = gate(4000, 1920);
        g.append(&pcm).unwrap();
        g.seal(100, &Sha256::digest(&pcm)).unwrap();
        g.frame(0).unwrap();
        assert_eq!(g.take(960, true).unwrap(), Some(pcm.clone()));
        assert!(g.take(960, true).unwrap().is_none());
        let mut unsealed = gate(4000, 960);
        unsealed.append(&pcm).unwrap();
        assert_eq!(unsealed.frame(0), Err(Error::Order));
    }
    #[test]
    fn overflow_wrong_hash_and_frame_holes_are_terminal() {
        for fault in 0..4 {
            let mut g = gate(960, 960);
            g.append(&vec![0; 1920]).unwrap();
            let result = match fault {
                0 => g.append(&[0, 0]),
                1 => g.seal(960, &[0; 32]),
                2 => g.frame(960),
                _ => g.append(&[0]),
            };
            assert!(result.is_err());
            assert_eq!(g.take(1, false), Err(Error::Cancelled));
        }
    }
    #[test]
    fn cancelled_parent_never_releases_buffered_audio() {
        let mut g = gate(960, 960);
        g.append(&vec![1; 1920]).unwrap();
        g.frame(0).unwrap();
        g.operation.cancel();
        assert_eq!(g.take(960, false), Err(Error::Cancelled));
    }
}

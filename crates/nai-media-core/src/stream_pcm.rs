//! Bounded PCM ingress and device-frame/source-sample mapping. No device/HTTP I/O.
use nai_contracts::v561_stream_audio::{Begin, Chunk, Seal};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidStream;

/// Diagnostic only: does not change pull admission or authorize silence.
pub fn shortfall_reason(
    source: u64,
    requested: usize,
    received: u64,
    play_until: u64,
) -> &'static str {
    let Some(end) = u64::try_from(requested)
        .ok()
        .and_then(|n| source.checked_add(n))
    else {
        return "INVALID_RANGE";
    };
    if requested == 0 || received < source {
        return "INVALID_RANGE";
    }
    match (received < end, play_until < end) {
        (true, true) => "PCM_AND_VIDEO_GATE",
        (true, false) => "PCM_SUPPLY",
        (false, true) => "VIDEO_GATE",
        (false, false) => "NO_SOURCE_SHORTFALL",
    }
}

/// Once playback starts, a device pull must be backed by source audio.
/// Only the verified final tail may be shorter than a device buffer.
pub fn validate_device_pull(
    requested: usize,
    actual: usize,
    final_tail: bool,
) -> Result<(), InvalidStream> {
    if requested == 0 || actual > requested || (actual < requested && !final_tail) {
        Err(InvalidStream)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod device_pull_tests {
    use super::*;
    #[test]
    fn shortfall_distinguishes_supply_from_authorization_without_mutating_buffer() {
        assert_eq!(shortfall_reason(48000, 240, 96000, 48000), "VIDEO_GATE");
        assert_eq!(shortfall_reason(48000, 240, 48000, 96000), "PCM_SUPPLY");
        assert_eq!(
            shortfall_reason(48000, 240, 48000, 48000),
            "PCM_AND_VIDEO_GATE"
        );
        assert_eq!(
            shortfall_reason(48000, 240, 48240, 48240),
            "NO_SOURCE_SHORTFALL"
        );
        assert_eq!(
            shortfall_reason(u64::MAX, 240, u64::MAX, u64::MAX),
            "INVALID_RANGE"
        );
        assert_eq!(shortfall_reason(48000, 240, 47999, 96000), "INVALID_RANGE");
    }
    #[test]
    fn verified_sentence_boundary_allows_wait_but_middle_short_read_still_fails() {
        let mut buffer = Buffer::new(&Begin {
            sample_rate: 24000,
            capacity_samples: 4800,
            max_samples: 9600,
            prefill_samples: 2400,
        })
        .unwrap();
        let bytes = [1, 0, 2, 0];
        buffer
            .push(
                &Chunk {
                    chunk_sequence: 1,
                    source_sample: 0,
                    samples: 2,
                    sha256: Sha256::digest(bytes).to_vec(),
                },
                &bytes,
            )
            .unwrap();
        assert!(!buffer.verified_tail(2));
        buffer.mark_verified_boundary(2).unwrap();
        let (at, samples) = buffer.take(240, 2);
        assert!(
            validate_device_pull(
                240,
                samples.len(),
                buffer.verified_tail(at + samples.len() as u64)
            )
            .is_ok()
        );
        let (at, samples) = buffer.take(240, 2);
        assert!(samples.is_empty());
        assert!(validate_device_pull(240, 0, buffer.verified_tail(at)).is_ok());
        buffer
            .push(
                &Chunk {
                    chunk_sequence: 2,
                    source_sample: 2,
                    samples: 2,
                    sha256: Sha256::digest(bytes).to_vec(),
                },
                &bytes,
            )
            .unwrap();
        let (at, samples) = buffer.take(240, 4);
        assert!(!buffer.verified_tail(at + samples.len() as u64));
        assert!(validate_device_pull(240, samples.len(), false).is_err());
    }
    #[test]
    fn unsealed_gaps_fail_and_only_verified_final_tail_may_be_short() {
        assert!(!authorized_prefill_ready(
            4800,
            false,
            2400,
            std::time::Duration::ZERO,
            0
        ));
        assert!(authorized_prefill_ready(
            4800,
            false,
            2400,
            std::time::Duration::ZERO,
            2400
        ));
        assert!(validate_device_pull(240, 240, false).is_ok());
        assert!(validate_device_pull(240, 0, false).is_err());
        assert!(validate_device_pull(240, 239, false).is_err());
        assert!(validate_device_pull(240, 239, true).is_ok());
        assert!(validate_device_pull(240, 0, true).is_ok());
        assert!(validate_device_pull(240, 241, true).is_err());
        assert!(validate_device_pull(0, 0, true).is_err());
    }
}

/// A short final sentence must not wait forever for a prefill larger than itself.
/// The wall-time bound equals the configured prefill's source-audio duration.
pub fn prefill_ready(received: u64, sealed: bool, target: u32, wait: std::time::Duration) -> bool {
    received > 0
        && (sealed
            || received >= u64::from(target)
            || wait >= std::time::Duration::from_secs_f64(f64::from(target) / 24000.0))
}

pub fn authorized_prefill_ready(
    received: u64,
    sealed: bool,
    target: u32,
    wait: std::time::Duration,
    play_until: u64,
) -> bool {
    prefill_ready(received, sealed, target, wait) && play_until >= received.min(u64::from(target))
}

pub struct Buffer {
    queue: VecDeque<i16>,
    capacity: usize,
    maximum: u64,
    sequence: u64,
    received: u64,
    dequeued: u64,
    hash: Sha256,
    sealed: bool,
    boundaries: VecDeque<u64>,
}
impl Buffer {
    pub fn new(begin: &Begin) -> Result<Self, InvalidStream> {
        Self::with_maximum(begin, 480000)
    }

    pub fn new_script(begin: &Begin) -> Result<Self, InvalidStream> {
        Self::with_maximum(begin, crate::continuous_audio::MAX_SCRIPT_SAMPLES)
    }

    fn with_maximum(begin: &Begin, maximum: u64) -> Result<Self, InvalidStream> {
        if begin.sample_rate != 24000
            || begin.capacity_samples == 0
            || begin.capacity_samples > 96000
            || begin.max_samples == 0
            || begin.max_samples > maximum
            || begin.prefill_samples == 0
            || begin.prefill_samples > begin.capacity_samples
        {
            return Err(InvalidStream);
        }
        Ok(Self {
            queue: VecDeque::new(),
            capacity: begin.capacity_samples as usize,
            maximum: begin.max_samples,
            sequence: 0,
            received: 0,
            dequeued: 0,
            hash: Sha256::new(),
            sealed: false,
            boundaries: VecDeque::with_capacity(2),
        })
    }
    pub fn push(&mut self, meta: &Chunk, bytes: &[u8]) -> Result<(), InvalidStream> {
        let end = self
            .received
            .checked_add(u64::from(meta.samples))
            .ok_or(InvalidStream)?;
        if self.sealed
            || meta.chunk_sequence != self.sequence + 1
            || meta.source_sample != self.received
            || meta.samples == 0
            || meta.samples > 4800
            || bytes.len() != meta.samples as usize * 2
            || end > self.maximum
            || meta.samples as usize > self.free()
            || meta.sha256.len() != 32
            || Sha256::digest(bytes).as_slice() != meta.sha256
        {
            return Err(InvalidStream);
        }
        self.queue.extend(
            bytes
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]])),
        );
        self.hash.update(bytes);
        self.received = end;
        self.sequence = meta.chunk_sequence;
        Ok(())
    }
    pub fn seal(&mut self, end: &Seal) -> Result<(), InvalidStream> {
        if self.sealed
            || self.received == 0
            || end.total_samples != self.received
            || end.sha256.len() != 32
            || self.hash.clone().finalize().as_slice() != end.sha256
        {
            return Err(InvalidStream);
        }
        self.sealed = true;
        Ok(())
    }
    pub fn take(&mut self, maximum: usize, play_until: u64) -> (u64, Vec<i16>) {
        let at = self.dequeued;
        let count = maximum
            .min(self.queue.len())
            .min(play_until.saturating_sub(at) as usize);
        let samples = self.queue.drain(..count).collect();
        self.dequeued += count as u64;
        while self
            .boundaries
            .front()
            .is_some_and(|end| *end < self.dequeued)
        {
            self.boundaries.pop_front();
        }
        (at, samples)
    }
    /// Only the authenticated segment tracker may establish this boundary,
    /// after validating the segment's actual bytes and hash.
    pub fn mark_verified_boundary(&mut self, end: u64) -> Result<(), InvalidStream> {
        if self.sealed
            || end == 0
            || end != self.received
            || self.boundaries.len() >= 2
            || self
                .boundaries
                .back()
                .is_some_and(|previous| *previous >= end)
        {
            return Err(InvalidStream);
        }
        self.boundaries.push_back(end);
        Ok(())
    }
    pub fn verified_tail(&self, end: u64) -> bool {
        (self.sealed && end == self.received) || self.boundaries.contains(&end)
    }
    pub fn free(&self) -> usize {
        self.capacity - self.queue.len()
    }
    pub fn received(&self) -> u64 {
        self.received
    }
    pub fn sealed(&self) -> bool {
        self.sealed
    }
}

struct Span {
    device: u64,
    source: u64,
    samples: u64,
}
#[derive(Default)]
pub struct Timeline {
    spans: VecDeque<Span>,
    submitted: u64,
    played: u64,
    device_played: u64,
    last_device_end: u64,
}
impl Timeline {
    pub fn commit(&mut self, device: u64, source: u64, samples: u64) -> Result<(), InvalidStream> {
        if samples == 0
            || source != self.submitted
            || device < self.last_device_end
            || self.spans.len() >= 64
        {
            return Err(InvalidStream);
        }
        self.last_device_end = device
            .checked_add(samples.checked_mul(2).ok_or(InvalidStream)?)
            .ok_or(InvalidStream)?;
        self.submitted = source.checked_add(samples).ok_or(InvalidStream)?;
        self.spans.push_back(Span {
            device,
            source,
            samples,
        });
        Ok(())
    }
    pub fn observe(&mut self, device: u64) -> Result<u64, InvalidStream> {
        if device < self.device_played {
            return Err(InvalidStream);
        }
        self.device_played = device;
        while let Some(span) = self.spans.front() {
            let done = device.saturating_sub(span.device).min(span.samples * 2) / 2;
            self.played = self.played.max(span.source + done);
            if done < span.samples {
                break;
            }
            self.spans.pop_front();
        }
        Ok(self.played)
    }
    pub fn submitted(&self) -> u64 {
        self.submitted
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn extended_script_keeps_the_pcm_queue_bounded() {
        let begin = nai_contracts::v561_stream_audio::Begin {
            sample_rate: 24000,
            capacity_samples: 96000,
            max_samples: crate::continuous_audio::MAX_SCRIPT_SAMPLES,
            prefill_samples: 2400,
        };
        assert!(super::Buffer::new(&begin).is_err());
        assert_eq!(super::Buffer::new_script(&begin).unwrap().free(), 96000);
        assert!(
            super::Buffer::new_script(&nai_contracts::v561_stream_audio::Begin {
                max_samples: begin.max_samples + 1,
                ..begin
            })
            .is_err()
        );
    }
    use super::*;
    #[test]
    fn short_pcm_prefill_has_a_bounded_wait_without_advancing_empty_source() {
        use std::time::Duration;
        assert!(!prefill_ready(0, false, 2400, Duration::from_secs(1)));
        assert!(!prefill_ready(480, false, 2400, Duration::from_millis(99)));
        assert!(prefill_ready(480, false, 2400, Duration::from_millis(100)));
        assert!(prefill_ready(2400, false, 2400, Duration::ZERO));
        assert!(prefill_ready(480, true, 2400, Duration::ZERO));
    }
    fn begin() -> Begin {
        Begin {
            sample_rate: 24000,
            capacity_samples: 8,
            max_samples: 16,
            prefill_samples: 2,
        }
    }
    fn chunk(sequence: u64, at: u64, data: &[u8]) -> Chunk {
        Chunk {
            chunk_sequence: sequence,
            source_sample: at,
            samples: (data.len() / 2) as u32,
            sha256: Sha256::digest(data).to_vec(),
        }
    }
    #[test]
    fn ring_is_bounded_range_and_hash_checked_then_sealed() {
        let mut b = Buffer::new(&begin()).unwrap();
        let data = [1, 0, 2, 0];
        b.push(&chunk(1, 0, &data), &data).unwrap();
        assert!(b.push(&chunk(1, 0, &data), &data).is_err());
        assert!(b.push(&chunk(2, 2, &data), &[9, 0, 2, 0]).is_err());
        assert_eq!(b.take(5, 1), (0, vec![1]));
        assert_eq!(b.take(5, 1), (1, vec![]));
        b.seal(&Seal {
            total_samples: 2,
            sha256: Sha256::digest(data).to_vec(),
        })
        .unwrap();
        assert!(b.push(&chunk(2, 2, &data), &data).is_err());
        assert!(b.seal(&Seal::default()).is_err());
    }
    #[test]
    fn overflow_partial_and_wrong_end_are_rejected() {
        let mut b = Buffer::new(&begin()).unwrap();
        let data = vec![0; 16];
        b.push(&chunk(1, 0, &data), &data).unwrap();
        assert!(b.push(&chunk(2, 8, &[0, 0]), &[0, 0]).is_err());
        assert!(
            b.seal(&Seal {
                total_samples: 7,
                sha256: Sha256::digest(&data).to_vec()
            })
            .is_err()
        );
        let mut c = chunk(2, 8, &[0, 0]);
        c.samples = 1;
        assert!(b.push(&c, &[0]).is_err());
    }
    #[test]
    fn gaps_do_not_advance_source_and_completed_spans_retire() {
        let mut t = Timeline::default();
        t.commit(100, 0, 10).unwrap();
        assert_eq!(t.observe(90).unwrap(), 0);
        assert_eq!(t.observe(120).unwrap(), 10);
        assert_eq!(t.observe(1000).unwrap(), 10);
        t.commit(1000, 10, 5).unwrap();
        assert_eq!(t.observe(1006).unwrap(), 13);
        assert!(t.observe(1005).is_err());
        assert_eq!(t.observe(1010).unwrap(), 15);
        for i in 0..1000 {
            t.commit(2000 + i * 2, 15 + i, 1).unwrap();
            t.observe(2002 + i * 2).unwrap();
        }
        assert!(t.spans.is_empty());
    }
}

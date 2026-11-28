//! V5.6.1 bounded frame selection. The caller authenticates producer and clock
//! source and supplies an owned copy of each slot before releasing that slot.
use nai_contracts::{
    v561_media::{ClockState, FrameRef, PlaybackClockObservation},
    v561_media_validation::{FrameLimits, validate_clock, validate_frame},
};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Stale,
    Full,
    Clock,
    Digest,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Offer {
    Queued,
    DiscardedLate,
}

pub struct Frame {
    pub metadata: FrameRef,
    pub pixels: Vec<u8>,
}
/// Private, immutable owned bytes. Integrity alone grants no playback authority.
pub struct OwnedVerifiedFrame {
    frame: Frame,
    digest: [u8; 32],
}
impl OwnedVerifiedFrame {
    pub fn verify(frame: Frame, limits: FrameLimits) -> Result<Self, Error> {
        validate_frame(&frame.metadata, limits).map_err(|_| Error::Invalid)?;
        if frame.pixels.len() as u64 != frame.metadata.payload_bytes
            || frame.metadata.content_sha256.len() != 32
        {
            return Err(Error::Invalid);
        }
        let digest: [u8; 32] = Sha256::digest(&frame.pixels).into();
        if digest.as_slice() != frame.metadata.content_sha256 {
            return Err(Error::Digest);
        }
        Ok(Self { frame, digest })
    }
    pub fn metadata(&self) -> &FrameRef {
        &self.frame.metadata
    }
    pub(crate) fn pixels(&self) -> &[u8] {
        &self.frame.pixels
    }
    /// Consuming conversion loses the checksum credential before any mutation.
    pub fn into_frame(self) -> Frame {
        self.frame
    }
}

pub struct Scheduler {
    pool: String,
    epoch: u64,
    device_generation: u64,
    limits: FrameLimits,
    capacity: usize,
    /// Maximum acceptable lateness in the playback clock's sample units.
    max_lag_samples: u64,
    frames: VecDeque<Frame>,
    current: Option<Frame>,
    generations: Vec<u64>,
    last_sequence: Option<u64>,
    last_source: Option<u64>,
    source_rate: Option<u32>,
    clock_rate: Option<u32>,
    last_clock_sequence: Option<u64>,
    played: Option<u64>,
    stopped: bool,
    coalesced_due_frames: u64,
}

impl Scheduler {
    pub fn has_capacity(&self) -> bool {
        !self.stopped && self.frames.len() < self.capacity
    }
    /// Read-only owned pixels selected by the last clock. This is not freshness
    /// or playback authority; the caller must recheck its trusted clock and task.
    pub fn selected_frame(&self) -> Option<&Frame> {
        if self.stopped {
            None
        } else {
            self.current.as_ref()
        }
    }
    pub fn new(
        pool: String,
        epoch: u64,
        device_generation: u64,
        limits: FrameLimits,
        capacity: usize,
        max_lag_samples: u64,
    ) -> Result<Self, Error> {
        if pool.is_empty()
            || epoch == 0
            || device_generation == 0
            || capacity == 0
            || limits.slots > 256
            || capacity > 256
            || max_lag_samples == 0
        {
            return Err(Error::Invalid);
        }
        let generations = vec![0; limits.slots as usize];
        Ok(Self {
            pool,
            epoch,
            device_generation,
            limits,
            capacity,
            max_lag_samples,
            frames: VecDeque::new(),
            current: None,
            generations,
            last_sequence: None,
            last_source: None,
            source_rate: None,
            clock_rate: None,
            last_clock_sequence: None,
            played: None,
            stopped: false,
            coalesced_due_frames: 0,
        })
    }

    /// Frames overtaken by a later due frame within one clock observation.
    /// This does not include late offers, expired frames, or cleared queues.
    pub fn coalesced_due_frames(&self) -> u64 {
        self.coalesced_due_frames
    }

    /// Frames still waiting for their source sample at the latest observation.
    pub fn queued_frames(&self) -> usize {
        self.frames.len()
    }

    /// Last queued frame's lead over the latest device clock, in clock samples.
    /// Excludes the frame duration and returns None until a running clock exists.
    pub fn queued_lead_samples(&self) -> Option<u64> {
        let played = self.played?;
        let clock_rate = self.clock_rate?;
        let Some(last) = self.frames.back() else {
            return Some(0);
        };
        let frame_sample = u128::from(last.metadata.source_sample?) * u128::from(clock_rate)
            / u128::from(last.metadata.source_sample_rate);
        Some(u64::try_from(frame_sample.saturating_sub(u128::from(played))).unwrap_or(u64::MAX))
    }

    /// Copies must already be private to the consumer; metadata validation does
    /// not authenticate the sender or prove the slot has not been overwritten.
    pub fn offer(&mut self, metadata: FrameRef, pixels: Vec<u8>) -> Result<Offer, Error> {
        if self.stopped {
            return Err(Error::Stale);
        }
        validate_frame(&metadata, self.limits).map_err(|_| Error::Invalid)?;
        if pixels.len() as u64 != metadata.payload_bytes {
            return Err(Error::Invalid);
        }
        let digest = (!metadata.content_sha256.is_empty()).then(|| Sha256::digest(&pixels).into());
        self.offer_with_digest(metadata, pixels, digest)
    }
    pub fn offer_verified(&mut self, verified: OwnedVerifiedFrame) -> Result<Offer, Error> {
        self.offer_with_digest(
            verified.frame.metadata,
            verified.frame.pixels,
            Some(verified.digest),
        )
    }
    fn offer_with_digest(
        &mut self,
        metadata: FrameRef,
        pixels: Vec<u8>,
        digest: Option<[u8; 32]>,
    ) -> Result<Offer, Error> {
        if self.stopped {
            return Err(Error::Stale);
        }
        validate_frame(&metadata, self.limits).map_err(|_| Error::Invalid)?;
        let seq = metadata.sequence.ok_or(Error::Invalid)?;
        let source = metadata.source_sample.ok_or(Error::Invalid)?;
        let slot = metadata.slot_index.ok_or(Error::Invalid)? as usize;
        if metadata.epoch != self.epoch
            || metadata.pool_id != self.pool
            || self.last_sequence.is_some_and(|v| seq <= v)
            || self.last_source.is_some_and(|v| source <= v)
            || metadata.slot_generation <= self.generations[slot]
            || self
                .source_rate
                .is_some_and(|v| v != metadata.source_sample_rate)
        {
            return Err(Error::Stale);
        }
        if pixels.len() as u64 != metadata.payload_bytes {
            return Err(Error::Invalid);
        }
        if !metadata.content_sha256.is_empty()
            && digest.as_ref().map(|d| d.as_slice()) != Some(metadata.content_sha256.as_slice())
        {
            return Err(Error::Digest);
        }
        let too_late = self
            .played
            .zip(self.clock_rate)
            .is_some_and(|(played, clock_rate)| {
                let source_rate = u128::from(metadata.source_sample_rate);
                u128::from(played) * source_rate
                    > u128::from(source) * u128::from(clock_rate)
                        + u128::from(self.max_lag_samples) * source_rate
            });
        if !too_late && self.frames.len() == self.capacity {
            return Err(Error::Full);
        }
        self.last_sequence = Some(seq);
        self.last_source = Some(source);
        self.generations[slot] = metadata.slot_generation;
        self.source_rate = Some(metadata.source_sample_rate);
        if too_late {
            return Ok(Offer::DiscardedLate);
        }
        self.frames.push_back(Frame { metadata, pixels });
        Ok(Offer::Queued)
    }

    /// None means show the safe frame. A stalled/unavailable clock never advances.
    /// The caller must independently enforce observation freshness and peer identity.
    pub fn present(&mut self, clock: &PlaybackClockObservation) -> Result<Option<&Frame>, Error> {
        if self.stopped {
            return Err(Error::Stale);
        }
        if validate_clock(clock, self.limits.max_sample_rate).is_err()
            || clock.epoch != self.epoch
            || clock.device_generation != self.device_generation
            || self
                .last_clock_sequence
                .is_some_and(|v| clock.observation_sequence.unwrap_or(0) <= v)
        {
            self.stop();
            return Err(Error::Clock);
        }
        self.last_clock_sequence = clock.observation_sequence;
        if clock.state != ClockState::Running as i32 {
            self.frames.clear();
            self.current = None;
            return Ok(None);
        }
        if self
            .clock_rate
            .is_some_and(|rate| rate != clock.sample_rate)
        {
            self.stop();
            return Err(Error::Clock);
        }
        self.clock_rate = Some(clock.sample_rate);
        let played = clock.played_sample.ok_or(Error::Clock)?;
        if self.played.is_some_and(|v| played < v) {
            self.stop();
            return Err(Error::Clock);
        }
        self.played = Some(played);
        let mut due_in_observation = 0u64;
        while self.frames.front().is_some_and(|f| {
            f.metadata.source_sample.is_some_and(|s| {
                u128::from(s) * u128::from(clock.sample_rate)
                    <= u128::from(played) * u128::from(f.metadata.source_sample_rate)
            })
        }) {
            if due_in_observation > 0 {
                self.coalesced_due_frames = self.coalesced_due_frames.saturating_add(1);
            }
            due_in_observation += 1;
            self.current = self.frames.pop_front();
        }
        if self.current.as_ref().is_some_and(|f| {
            let source = u128::from(f.metadata.source_sample.unwrap_or(0));
            let rate = u128::from(f.metadata.source_sample_rate);
            let played_in_source = u128::from(played) * rate;
            let frame_in_clock = source * u128::from(clock.sample_rate);
            played_in_source.saturating_sub(frame_in_clock)
                > u128::from(self.max_lag_samples) * rate
        }) {
            self.current = None;
        }
        Ok(self.current.as_ref())
    }

    /// Take an owned snapshot of the most recent clock selection for another
    /// consumer. The caller must still enforce clock freshness and output authority.
    pub fn copy_selected_verified(
        &self,
        after_sequence: Option<u64>,
    ) -> Option<OwnedVerifiedFrame> {
        let selected = self.current.as_ref()?;
        let sequence = selected.metadata.sequence?;
        if after_sequence.is_some_and(|last| sequence <= last) {
            return None;
        }
        Self::copy_verified(selected)
    }

    /// Retain the stream origin before the first running observation coalesces
    /// due frames. This snapshot grants no presentation or playback authority.
    pub fn copy_verified_origin(&self) -> Option<OwnedVerifiedFrame> {
        self.current
            .iter()
            .chain(self.frames.front())
            .find(|frame| {
                frame.metadata.sequence == Some(0) && frame.metadata.source_sample == Some(0)
            })
            .and_then(Self::copy_verified)
    }

    fn copy_verified(selected: &Frame) -> Option<OwnedVerifiedFrame> {
        // Admission checked this digest against these private, unchanged bytes.
        let digest = selected
            .metadata
            .content_sha256
            .as_slice()
            .try_into()
            .ok()?;
        Some(OwnedVerifiedFrame {
            frame: Frame {
                metadata: selected.metadata.clone(),
                pixels: selected.pixels.clone(),
            },
            digest,
        })
    }

    pub fn stop(&mut self) {
        self.stopped = true;
        self.frames.clear();
        self.current = None;
        self.played = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn verified(seq: u64, sample: u64) -> OwnedVerifiedFrame {
        let pixels = vec![1; 16];
        let mut metadata = frame(seq, sample);
        metadata.content_sha256 = Sha256::digest(&pixels).to_vec();
        OwnedVerifiedFrame::verify(Frame { metadata, pixels }, scheduler().limits).unwrap()
    }
    #[test]
    fn export_only_new_verified_clock_selection() {
        let mut scheduler = scheduler();
        scheduler.offer_verified(verified(0, 0)).unwrap();
        scheduler.offer_verified(verified(1, 960)).unwrap();
        assert!(scheduler.copy_selected_verified(None).is_none());
        scheduler.present(&clock(1, 0)).unwrap();
        let selected = scheduler.copy_selected_verified(None).unwrap();
        assert_eq!(selected.metadata().sequence, Some(0));
        assert_eq!(selected.pixels(), &[1; 16]);
        assert!(scheduler.copy_selected_verified(Some(0)).is_none());
        scheduler.present(&clock(2, 960)).unwrap();
        assert_eq!(
            scheduler
                .copy_selected_verified(Some(0))
                .unwrap()
                .metadata()
                .sequence,
            Some(1)
        );
        scheduler.stop();
        assert!(scheduler.copy_selected_verified(None).is_none());
    }
    #[test]
    fn retains_verified_origin_before_first_running_clock_jumps() {
        let mut scheduler = scheduler();
        scheduler.offer_verified(verified(0, 0)).unwrap();
        scheduler.offer_verified(verified(1, 960)).unwrap();
        let origin = scheduler.copy_verified_origin().unwrap();
        assert_eq!(origin.metadata().sequence, Some(0));
        assert_eq!(
            scheduler
                .present(&clock(1, 960))
                .unwrap()
                .unwrap()
                .metadata
                .sequence,
            Some(1)
        );
        assert!(scheduler.copy_verified_origin().is_none());
        assert_eq!(origin.pixels(), &[1; 16]);
    }
    #[test]
    fn leased_output_clears_idle_video_and_denies_audio_on_timeout_or_stop() {
        use crate::{v561_frame_lease::Identity, v561_leased_output::LeasedOutput};
        use std::{
            num::NonZeroU64,
            time::{Duration, Instant},
        };
        let identity = Identity {
            epoch: NonZeroU64::new(1).unwrap(),
            generation: NonZeroU64::new(2).unwrap(),
        };
        let wrong = Identity {
            generation: NonZeroU64::new(3).unwrap(),
            ..identity
        };
        let now = Instant::now();
        let timeout = Duration::from_millis(500);
        for explicit_stop in [false, true] {
            let mut output = LeasedOutput::new(identity, now, timeout).unwrap();
            assert!(!output.permits_audio(identity, now));
            assert!(output.accept(identity, verified(0, 0), now));
            assert!(output.accept(identity, verified(1, 960), now));
            assert_eq!(output.frame(now).unwrap().metadata().sequence, Some(1));
            assert!(output.permits_audio(identity, now));
            assert!(!output.permits_audio(wrong, now));
            let mut video = vec![255; output.frame(now).unwrap().pixels().len()];
            assert!(output.write_video(&mut video, now));
            assert_eq!(video, output.frame(now).unwrap().pixels());
            let mut audio = [0; 2];
            assert!(output.write_audio(identity, &[123, -456], &mut audio, now));
            assert_eq!(audio, [123, -456]);
            assert!(!output.write_audio(wrong, &[123, -456], &mut audio, now));
            assert_eq!(audio, [0; 2]);
            let revoked_at = if explicit_stop {
                output.stop();
                now
            } else {
                now + timeout
            };
            video.fill(255);
            audio.fill(123);
            assert!(!output.write_video(&mut video, revoked_at));
            assert!(!output.write_audio(identity, &[123, -456], &mut audio, revoked_at));
            assert!(video.iter().all(|&byte| byte == 0));
            assert_eq!(audio, [0; 2]);
            assert!(output.frame(revoked_at).is_none());
            assert!(!output.permits_audio(identity, revoked_at));
            assert!(!output.accept(identity, verified(2, 1920), revoked_at));
        }
    }
    #[test]
    fn lease_accepts_verified_first_frame_and_rejects_wrong_epoch() {
        use crate::v561_frame_lease::{FrameLease, Identity};
        use std::{
            num::NonZeroU64,
            time::{Duration, Instant},
        };
        let generation = NonZeroU64::new(4).unwrap();
        let now = Instant::now();
        let mut lease = FrameLease::new(
            Identity {
                epoch: NonZeroU64::new(1).unwrap(),
                generation,
            },
            now,
            Duration::from_millis(500),
        )
        .unwrap();
        assert!(lease.observe_verified(generation, &verified(0, 0), now));
        assert!(!lease.observe_verified(generation, &verified(0, 0), now));
        let mut other = verified(1, 960).into_frame();
        other.metadata.epoch = 2;
        let other = OwnedVerifiedFrame::verify(other, scheduler().limits).unwrap();
        assert!(!lease.observe_verified(generation, &other, now));
        assert!(!lease.permits(now + Duration::from_millis(500)));
    }
    #[test]
    fn owned_checksum_token_preserves_context_order_stop_and_capacity_checks() {
        let mut s = scheduler();
        s.offer_verified(verified(0, 0)).unwrap();
        assert_eq!(s.offer_verified(verified(0, 0)), Err(Error::Stale));
        s.offer_verified(verified(1, 960)).unwrap();
        assert_eq!(s.offer_verified(verified(2, 1920)), Err(Error::Full));
        let mut other = verified(2, 1920).into_frame();
        other.metadata.epoch = 2;
        let token = OwnedVerifiedFrame::verify(other, s.limits).unwrap();
        assert_eq!(s.offer_verified(token), Err(Error::Stale));
        s.stop();
        assert_eq!(s.offer_verified(verified(2, 1920)), Err(Error::Stale));
    }
    #[test]
    fn mutable_conversion_loses_token_and_changed_pixels_require_new_hash() {
        let mut unverified = verified(0, 0).into_frame();
        unverified.pixels[0] = 9;
        assert!(matches!(
            OwnedVerifiedFrame::verify(unverified, scheduler().limits),
            Err(Error::Digest)
        ));
        assert!(matches!(
            OwnedVerifiedFrame::verify(
                Frame {
                    metadata: frame(0, 0),
                    pixels: vec![1; 16]
                },
                scheduler().limits
            ),
            Err(Error::Invalid)
        ));
    }
    use nai_contracts::v561_media::PixelFormat;
    fn scheduler() -> Scheduler {
        Scheduler::new(
            "pool".into(),
            1,
            2,
            FrameLimits {
                slots: 2,
                max_width: 2,
                max_height: 2,
                max_payload_bytes: 16,
                max_sample_rate: 48000,
            },
            2,
            4800,
        )
        .unwrap()
    }
    fn frame(seq: u64, sample: u64) -> FrameRef {
        FrameRef {
            pool_id: "pool".into(),
            epoch: 1,
            sequence: Some(seq),
            source_sample: Some(sample),
            source_sample_rate: 24000,
            pts_ns: Some(0),
            width: 2,
            height: 2,
            stride_bytes: 8,
            pixel_format: PixelFormat::Bgra8 as i32,
            slot_index: Some((seq % 2) as u32),
            slot_generation: seq / 2 + 1,
            payload_bytes: 16,
            content_sha256: vec![],
        }
    }
    fn clock(seq: u64, sample: u64) -> PlaybackClockObservation {
        PlaybackClockObservation {
            epoch: 1,
            device_generation: 2,
            observation_sequence: Some(seq),
            state: ClockState::Running as i32,
            played_sample: Some(sample),
            sample_rate: 24000,
        }
    }
    #[test]
    fn selects_only_reached_frame_and_bounds_queue() {
        let mut s = scheduler();
        s.offer(frame(0, 0), vec![0; 16]).unwrap();
        s.offer(frame(1, 960), vec![1; 16]).unwrap();
        assert_eq!(s.queued_lead_samples(), None);
        assert_eq!(s.offer(frame(2, 1920), vec![2; 16]), Err(Error::Full));
        assert_eq!(s.present(&clock(0, 0)).unwrap().unwrap().pixels[0], 0);
        assert_eq!(s.present(&clock(1, 500)).unwrap().unwrap().pixels[0], 0);
        assert_eq!(s.queued_lead_samples(), Some(460));
        assert_eq!(s.queued_frames(), 1);
        assert_eq!(s.present(&clock(2, 960)).unwrap().unwrap().pixels[0], 1);
        assert_eq!(s.queued_frames(), 0);
        assert_eq!(s.queued_lead_samples(), Some(0));
        assert_eq!(s.coalesced_due_frames(), 0);
    }
    #[test]
    fn counts_frames_overtaken_in_one_clock_observation() {
        let mut s = scheduler();
        s.offer(frame(0, 0), vec![0; 16]).unwrap();
        s.offer(frame(1, 960), vec![1; 16]).unwrap();
        assert_eq!(s.present(&clock(0, 960)).unwrap().unwrap().pixels[0], 1);
        assert_eq!(s.coalesced_due_frames(), 1);
        assert_eq!(s.present(&clock(1, 1000)).unwrap().unwrap().pixels[0], 1);
        assert_eq!(s.coalesced_due_frames(), 1);
    }
    #[test]
    fn stale_clock_or_stop_revokes_future_frames() {
        let mut s = scheduler();
        s.offer(frame(0, 0), vec![0; 16]).unwrap();
        assert!(s.present(&clock(1, 10)).unwrap().is_some());
        assert_eq!(s.present(&clock(2, 9)).err(), Some(Error::Clock));
        assert!(s.selected_frame().is_none());
        assert_eq!(s.offer(frame(1, 960), vec![0; 16]), Err(Error::Stale));
    }
    #[test]
    fn expired_frame_and_pause_show_safe_frame() {
        let mut s = scheduler();
        s.offer(frame(0, 0), vec![0; 16]).unwrap();
        assert!(s.present(&clock(0, 5000)).unwrap().is_none());
        s.offer(frame(1, 6000), vec![0; 16]).unwrap();
        let mut c = clock(1, 5000);
        c.state = ClockState::Paused as i32;
        assert!(s.present(&c).unwrap().is_none());
        assert!(s.present(&clock(2, 6000)).unwrap().is_none());
    }
    #[test]
    fn digest_and_epoch_are_checked() {
        let mut s = scheduler();
        let mut f = frame(0, 0);
        f.content_sha256 = vec![1; 32];
        assert_eq!(s.offer(f, vec![0; 16]), Err(Error::Digest));
        let mut f = frame(0, 0);
        f.epoch = 2;
        assert_eq!(s.offer(f, vec![0; 16]), Err(Error::Stale));
    }
    #[test]
    fn old_slot_generation_is_rejected() {
        let mut s = scheduler();
        s.offer(frame(0, 0), vec![0; 16]).unwrap();
        assert!(s.present(&clock(0, 0)).unwrap().is_some());
        let mut stale = frame(2, 1920);
        stale.slot_generation = 1;
        assert_eq!(s.offer(stale, vec![0; 16]), Err(Error::Stale));
    }
    #[test]
    fn flashhead_16k_frames_align_with_24k_playback() {
        let mut s = scheduler();
        let mut f = frame(0, 640);
        f.source_sample_rate = 16000;
        s.offer(f, vec![1; 16]).unwrap();
        assert!(s.present(&clock(0, 959)).unwrap().is_none());
        assert!(s.present(&clock(1, 960)).unwrap().is_some());
        assert!(s.present(&clock(2, 5760)).unwrap().is_some());
        assert!(s.present(&clock(3, 5761)).unwrap().is_none());
    }

    #[test]
    fn late_frames_are_discarded_without_replay_or_queue_growth() {
        let mut s = scheduler();
        assert!(s.present(&clock(0, 6000)).unwrap().is_none());
        assert_eq!(s.offer(frame(0, 0), vec![0; 16]), Ok(Offer::DiscardedLate));
        assert_eq!(s.offer(frame(0, 0), vec![0; 16]), Err(Error::Stale));
        assert_eq!(s.offer(frame(1, 5000), vec![1; 16]), Ok(Offer::Queued));
        assert_eq!(s.present(&clock(1, 6000)).unwrap().unwrap().pixels[0], 1);
    }
}

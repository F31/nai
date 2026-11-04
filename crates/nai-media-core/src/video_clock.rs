//! Cue-local window submission clock. No display/compositor or PlaybackAck claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoClockError {
    Invalid,
    Regression,
    Bounds,
    StaleSubmission,
}

#[derive(Debug, Clone, Copy)]
pub struct VideoFrame {
    index: u32,
    position: u64,
}
impl VideoFrame {
    pub fn index(self) -> u32 {
        self.index
    }
}

pub struct VideoCursor {
    total: u64,
    rate: u32,
    num: u32,
    den: u32,
    frames: u32,
    observed: u64,
    copied: u64,
    submitted: Option<VideoFrame>,
    invalid: bool,
}
impl VideoCursor {
    fn index_at(&self, played: u64) -> u32 {
        (u128::from(played) * u128::from(self.num) / (u128::from(self.rate) * u128::from(self.den)))
            .min(u128::from(self.frames - 1)) as u32
    }
    pub fn new(
        total: u64,
        rate: u32,
        num: u32,
        den: u32,
        frames: u32,
    ) -> Result<Self, VideoClockError> {
        if total == 0 || rate == 0 || num == 0 || den == 0 || frames == 0 {
            return Err(VideoClockError::Invalid);
        }
        Ok(Self {
            total,
            rate,
            num,
            den,
            frames,
            observed: 0,
            copied: 0,
            submitted: None,
            invalid: false,
        })
    }
    pub fn select(&mut self, played: u64, copied: u64) -> Result<VideoFrame, VideoClockError> {
        let error = if self.invalid {
            Some(VideoClockError::Invalid)
        } else if played < self.observed || copied < self.copied {
            Some(VideoClockError::Regression)
        } else if played > copied || copied > self.total {
            Some(VideoClockError::Bounds)
        } else {
            None
        };
        if let Some(error) = error {
            self.invalid = true;
            return Err(error);
        }
        self.observed = played;
        self.copied = copied;
        let index = self.index_at(played);
        Ok(VideoFrame {
            index,
            position: played,
        })
    }
    /// Call only after the matching window update succeeded and its lease was rechecked.
    pub fn submitted(&mut self, frame: VideoFrame) -> Result<(), VideoClockError> {
        if self.invalid
            || frame.position != self.observed
            || frame.index != self.index_at(self.observed)
        {
            self.invalid = true;
            return Err(VideoClockError::StaleSubmission);
        }
        self.submitted = Some(frame);
        Ok(())
    }
    pub fn tail_submitted(&self) -> bool {
        !self.invalid
            && self
                .submitted
                .is_some_and(|f| f.position == self.total && f.index == self.frames - 1)
    }
    pub fn tail(&self) -> Option<(u32, u64)> {
        self.tail_submitted().then(|| {
            let frame = self.submitted.unwrap();
            (frame.index, frame.position)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cursor() -> VideoCursor {
        VideoCursor::new(48000, 48000, 25, 1, 25).unwrap()
    }
    #[test]
    fn copied_ahead_does_not_advance_video_and_tail_requires_successful_submission() {
        let mut c = cursor();
        let first = c.select(0, 48000).unwrap();
        assert_eq!(first.index(), 0);
        c.submitted(first).unwrap();
        assert!(!c.tail_submitted());
        let middle = c.select(24000, 48000).unwrap();
        assert_eq!(middle.index(), 12);
        c.submitted(middle).unwrap();
        assert!(!c.tail_submitted());
        let last = c.select(48000, 48000).unwrap();
        assert_eq!(last.index(), 24);
        assert!(!c.tail_submitted());
        c.submitted(last).unwrap();
        assert_eq!(c.tail(), Some((24, 48000)));
    }
    #[test]
    fn invalid_position_poisoning_and_fresh_cue_do_not_share_tail() {
        for (played, copied) in [(48001, 48000), (24000, 23999), (0, 48001)] {
            let mut c = cursor();
            assert!(c.select(played, copied).is_err());
            assert!(c.select(48000, 48000).is_err());
            assert!(c.tail().is_none());
        }
        let mut c = cursor();
        c.select(24000, 48000).unwrap();
        assert!(c.select(23999, 48000).is_err());
        assert!(!c.tail_submitted());
        assert!(cursor().tail().is_none());
        let mut c = cursor();
        c.select(0, 24000).unwrap();
        assert!(c.select(0, 23999).is_err());
    }
    #[test]
    fn rational_rate_repeat_and_stale_window_update() {
        let mut c = VideoCursor::new(48000, 48000, 30000, 1001, 30).unwrap();
        let first = c.select(24000, 48000).unwrap();
        assert_eq!(first.index(), 14);
        assert_eq!(c.select(24000, 48000).unwrap().index(), 14);
        c.select(48000, 48000).unwrap();
        assert_eq!(c.submitted(first), Err(VideoClockError::StaleSubmission));
        assert!(!c.tail_submitted());
        let mut forged = cursor();
        forged.select(0, 48000).unwrap();
        assert!(
            forged
                .submitted(VideoFrame {
                    index: 1,
                    position: 0
                })
                .is_err()
        );
        for args in [
            (0, 48000, 25, 1, 25),
            (1, 0, 25, 1, 1),
            (1, 48000, 0, 1, 1),
            (1, 48000, 25, 0, 1),
            (1, 48000, 25, 1, 0),
        ] {
            assert!(VideoCursor::new(args.0, args.1, args.2, args.3, args.4).is_err());
        }
    }
}

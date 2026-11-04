//! One current frame chosen from samples the device has already played.
//!
//! The submitted cursor is not an input. The board stores an opaque frame id,
//! not pixels, and it grants no playback permission. A frame that arrives after
//! its deadline, or after playback has reached its source sample, is dropped
//! and is not chased. No eligible frame, a mismatched epoch, or revocation
//! yields standby. The caller owns the approved still and the research renderer.

use std::collections::BTreeMap;

/// Arrival of one frame from an independent renderer. `deadline` and `now` share
/// one monotonic millisecond clock. Arrival at the deadline is still on time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameDelivery {
    pub epoch: u64,
    pub source_sample: u64,
    pub frame_id: u64,
    pub deadline: u64,
    pub now: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    Accepted,
    Late,
    Full,
    Duplicate,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Frame { frame_id: u64, source_sample: u64 },
    Standby,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardError {
    Invalid,
}

#[derive(Debug)]
pub struct PlayedFrameBoard {
    epoch: u64,
    capacity: usize,
    held: BTreeMap<u64, u64>,
    shown: Option<(u64, u64)>,
    played: Option<u64>,
    revoked: bool,
}

impl PlayedFrameBoard {
    /// `capacity` is the number of frames waiting ahead of playback, from 1 to 16.
    pub fn open(epoch: u64, capacity: usize) -> Result<Self, BoardError> {
        if epoch == 0 || !(1..=16).contains(&capacity) {
            return Err(BoardError::Invalid);
        }
        Ok(Self {
            epoch,
            capacity,
            held: BTreeMap::new(),
            shown: None,
            played: None,
            revoked: false,
        })
    }

    pub fn offer(&mut self, delivery: FrameDelivery) -> Offer {
        if self.revoked || delivery.epoch != self.epoch {
            return Offer::Revoked;
        }
        if delivery.now > delivery.deadline {
            return Offer::Late;
        }
        if self
            .played
            .is_some_and(|played| delivery.source_sample <= played)
        {
            return Offer::Late;
        }
        if self.held.contains_key(&delivery.source_sample)
            || self
                .shown
                .is_some_and(|(sample, _)| sample == delivery.source_sample)
        {
            return Offer::Duplicate;
        }
        if self.held.len() >= self.capacity {
            return Offer::Full;
        }
        self.held.insert(delivery.source_sample, delivery.frame_id);
        Offer::Accepted
    }

    /// Shows the latest accepted frame whose source sample playback has reached.
    /// Frames skipped by that choice are discarded. Playback moving backwards
    /// revokes the board.
    pub fn present(&mut self, epoch: u64, played_samples: u64) -> Shown {
        if self.revoked || epoch != self.epoch {
            self.revoke();
            return Shown::Standby;
        }
        if self
            .played
            .is_some_and(|previous| played_samples < previous)
        {
            self.revoke();
            return Shown::Standby;
        }
        self.played = Some(played_samples);
        if let Some((sample, frame_id)) = self
            .held
            .range(..=played_samples)
            .next_back()
            .map(|(sample, frame_id)| (*sample, *frame_id))
        {
            self.held.retain(|held_sample, _| *held_sample > sample);
            self.shown = Some((sample, frame_id));
        }
        match self.shown {
            Some((sample, frame_id)) if sample <= played_samples => Shown::Frame {
                frame_id,
                source_sample: sample,
            },
            _ => Shown::Standby,
        }
    }

    /// Queues upcoming 800-sample windows from the played position. The submitted
    /// cursor is not an argument. Full, late, or duplicate offers are ignored.
    pub fn offer_windows(&mut self, played: u64, sample_count: u64, now_ms: u64) {
        let first = played / 800;
        for offset in 0..self.capacity as u64 {
            let index = first.saturating_add(offset);
            let source = match index.checked_mul(800) {
                Some(source) => source,
                None => break,
            };
            if source >= sample_count {
                break;
            }
            match self.offer(FrameDelivery {
                epoch: self.epoch,
                source_sample: source,
                frame_id: index.saturating_add(1),
                deadline: now_ms.saturating_add(250),
                now: now_ms,
            }) {
                Offer::Revoked => break,
                Offer::Accepted | Offer::Duplicate | Offer::Late | Offer::Full => {}
            }
        }
    }

    /// Drops every waiting frame and the current picture. Returns how many were waiting.
    pub fn revoke(&mut self) -> usize {
        let waiting = self.held.len();
        self.held.clear();
        self.shown = None;
        self.revoked = true;
        waiting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delivery(source_sample: u64, frame_id: u64) -> FrameDelivery {
        FrameDelivery {
            epoch: 1,
            source_sample,
            frame_id,
            deadline: 1_000,
            now: 0,
        }
    }

    #[test]
    fn open_rejects_a_zero_epoch_or_unbounded_queue() {
        assert!(matches!(
            PlayedFrameBoard::open(0, 8),
            Err(BoardError::Invalid)
        ));
        assert!(matches!(
            PlayedFrameBoard::open(1, 0),
            Err(BoardError::Invalid)
        ));
        assert!(matches!(
            PlayedFrameBoard::open(1, 17),
            Err(BoardError::Invalid)
        ));
    }

    #[test]
    fn prefill_waits_until_playback_reaches_its_sample() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(0, 10)), Offer::Accepted);
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Accepted);
        assert_eq!(
            board.present(1, 0),
            Shown::Frame {
                frame_id: 10,
                source_sample: 0
            }
        );
        assert_eq!(
            board.present(1, 960),
            Shown::Frame {
                frame_id: 10,
                source_sample: 0
            }
        );
        assert_eq!(
            board.present(1, 1_920),
            Shown::Frame {
                frame_id: 12,
                source_sample: 1_920
            }
        );
    }

    #[test]
    fn out_of_order_prefill_shows_only_the_reached_sample() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Accepted);
        assert_eq!(board.offer(delivery(960, 11)), Offer::Accepted);
        assert_eq!(board.present(1, 0), Shown::Standby);
        assert_eq!(
            board.present(1, 960),
            Shown::Frame {
                frame_id: 11,
                source_sample: 960
            }
        );
        assert_eq!(
            board.present(1, 1_920),
            Shown::Frame {
                frame_id: 12,
                source_sample: 1_920
            }
        );
    }

    #[test]
    fn a_frame_arriving_after_playback_passed_it_is_not_chased() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(0, 10)), Offer::Accepted);
        assert_eq!(
            board.present(1, 0),
            Shown::Frame {
                frame_id: 10,
                source_sample: 0
            }
        );
        assert_eq!(
            board.present(1, 5_000),
            Shown::Frame {
                frame_id: 10,
                source_sample: 0
            }
        );
        assert_eq!(board.offer(delivery(960, 11)), Offer::Late);
        assert_eq!(
            board.present(1, 5_000),
            Shown::Frame {
                frame_id: 10,
                source_sample: 0
            }
        );
    }

    #[test]
    fn a_missed_deadline_never_enters_the_queue() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        let mut late = delivery(960, 11);
        late.now = 11;
        late.deadline = 10;
        assert_eq!(board.offer(late), Offer::Late);
        let mut exact = delivery(960, 11);
        exact.now = 10;
        exact.deadline = 10;
        assert_eq!(board.offer(exact), Offer::Accepted);
        assert_eq!(board.present(1, 0), Shown::Standby);
    }

    #[test]
    fn the_queue_stays_bounded_and_frees_a_slot_after_playback() {
        let mut board = PlayedFrameBoard::open(1, 1).unwrap();
        assert_eq!(board.offer(delivery(960, 11)), Offer::Accepted);
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Full);
        assert_eq!(
            board.present(1, 960),
            Shown::Frame {
                frame_id: 11,
                source_sample: 960
            }
        );
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Accepted);
    }

    #[test]
    fn eight_waiting_frames_match_the_measured_prefill_bound() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        for index in 0..8 {
            let sample = index * 960;
            assert_eq!(board.offer(delivery(sample, index)), Offer::Accepted);
        }
        assert_eq!(board.offer(delivery(8 * 960, 8)), Offer::Full);
    }

    #[test]
    fn a_jumped_playback_discards_the_intermediate_frames() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(0, 10)), Offer::Accepted);
        assert_eq!(board.offer(delivery(960, 11)), Offer::Accepted);
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Accepted);
        assert_eq!(
            board.present(1, 1_920),
            Shown::Frame {
                frame_id: 12,
                source_sample: 1_920
            }
        );
        assert_eq!(board.offer(delivery(960, 11)), Offer::Late);
    }

    #[test]
    fn a_different_epoch_or_revoke_returns_to_standby() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(0, 10)), Offer::Accepted);
        let mut other = delivery(960, 11);
        other.epoch = 2;
        assert_eq!(board.offer(other), Offer::Revoked);
        assert_eq!(
            board.present(1, 0),
            Shown::Frame {
                frame_id: 10,
                source_sample: 0
            }
        );
        assert_eq!(board.present(2, 960), Shown::Standby);
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Revoked);
        assert_eq!(board.present(1, 1_920), Shown::Standby);

        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(960, 11)), Offer::Accepted);
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Accepted);
        assert_eq!(board.revoke(), 2);
        assert_eq!(board.present(1, 1_920), Shown::Standby);
        assert_eq!(board.offer(delivery(2_880, 13)), Offer::Revoked);
    }

    #[test]
    fn playback_moving_backwards_revokes_the_board() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(0, 10)), Offer::Accepted);
        assert_eq!(
            board.present(1, 960),
            Shown::Frame {
                frame_id: 10,
                source_sample: 0
            }
        );
        assert_eq!(board.present(1, 959), Shown::Standby);
        assert_eq!(board.offer(delivery(1_920, 12)), Offer::Revoked);
    }

    #[test]
    fn the_same_source_sample_is_not_replaced() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        assert_eq!(board.offer(delivery(960, 11)), Offer::Accepted);
        assert_eq!(board.offer(delivery(960, 99)), Offer::Duplicate);
        assert_eq!(
            board.present(1, 960),
            Shown::Frame {
                frame_id: 11,
                source_sample: 960
            }
        );
        assert_eq!(board.offer(delivery(960, 99)), Offer::Late);
    }

    #[test]
    fn offer_windows_presents_from_played_samples() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        board.offer_windows(0, 8_000, 0);
        assert_eq!(
            board.present(1, 0),
            Shown::Frame {
                frame_id: 1,
                source_sample: 0
            }
        );
        assert_eq!(
            board.present(1, 799),
            Shown::Frame {
                frame_id: 1,
                source_sample: 0
            }
        );
        board.offer_windows(800, 8_000, 0);
        assert_eq!(
            board.present(1, 800),
            Shown::Frame {
                frame_id: 2,
                source_sample: 800
            }
        );
    }

    #[test]
    fn offer_windows_starts_at_the_played_position() {
        let mut board = PlayedFrameBoard::open(1, 8).unwrap();
        board.offer_windows(1_600, 8_000, 0);
        assert_eq!(
            board.present(1, 1_600),
            Shown::Frame {
                frame_id: 3,
                source_sample: 1_600
            }
        );
    }
}

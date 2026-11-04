//! Source-sample submission position only; never device-completion or authorization evidence.
#[derive(Debug, PartialEq)]
pub struct Position {
    pub index: usize,
    pub fraction: f32,
}
pub struct SubmissionCursor {
    rate: u32,
    samples: u64,
    frames: u64,
}
impl SubmissionCursor {
    pub fn new(rate: u32, samples: usize) -> Option<Self> {
        if !(8000..=192000).contains(&rate) || samples == 0 || samples > 480000 {
            return None;
        }
        Some(Self {
            rate,
            samples: samples as u64,
            frames: 0,
        })
    }
    pub fn next(&mut self, allowed: bool) -> Option<Position> {
        if !allowed {
            return None;
        }
        let scaled = self.frames * 24000;
        let index = scaled / u64::from(self.rate);
        if index >= self.samples {
            return None;
        }
        let fraction = (scaled % u64::from(self.rate)) as f32 / self.rate as f32;
        self.frames += 1;
        Some(Position {
            index: index as usize,
            fraction,
        })
    }
    pub fn submitted(&self) -> u64 {
        (self.frames * 24000 / u64::from(self.rate)).min(self.samples)
    }
}
pub struct Render<'a> {
    pub pcm: &'a [i16],
    pub cursor: &'a mut SubmissionCursor,
    pub allowed: &'a dyn Fn(u64) -> bool,
    pub progress: &'a std::sync::atomic::AtomicU64,
    pub channels: usize,
}
impl Render<'_> {
    pub fn fill<T: Copy>(
        &mut self,
        data: &mut [T],
        mut now: impl FnMut() -> u64,
        convert: impl Fn(f32) -> T,
    ) {
        use std::sync::atomic::Ordering;
        for frame in data.chunks_mut(self.channels) {
            let time = now();
            let allowed = (self.allowed)(time);
            let sample = if let Some(p) = self.cursor.next(allowed) {
                let a = f32::from(self.pcm[p.index]);
                let b = f32::from(self.pcm[(p.index + 1).min(self.pcm.len() - 1)]);
                self.progress
                    .store(self.cursor.submitted(), Ordering::Release);
                (a + (b - a) * p.fraction) / 32768.0 * 0.5
            } else {
                0.0
            };
            frame.fill(convert(sample));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mute_and_mid_buffer_stop_do_not_advance_remaining_frames() {
        let mut c = SubmissionCursor::new(48000, 1601).unwrap();
        assert!(c.next(false).is_none());
        for frame in 0..12 {
            let p = c.next(frame < 4);
            assert_eq!(p.is_some(), frame < 4);
        }
        assert_eq!(c.submitted(), 2);
    }
    #[test]
    fn rates_and_partial_tail_map_exactly_without_a_pixel_or_pcm_queue() {
        for rate in [8000, 24000, 44100, 48000, 192000] {
            let mut c = SubmissionCursor::new(rate, 1601).unwrap();
            let mut count = 0;
            let mut old = 0;
            while let Some(p) = c.next(true) {
                assert!(p.index >= old && p.index < 1601);
                old = p.index;
                assert!((0.0..1.0).contains(&p.fraction));
                count += 1;
            }
            assert_eq!(count, (1601u64 * u64::from(rate)).div_ceil(24000));
            assert_eq!(c.submitted(), 1601);
            assert!(c.next(true).is_none());
        }
        assert!(SubmissionCursor::new(0, 1).is_none());
        assert!(SubmissionCursor::new(24000, 480001).is_none());
    }
}

#[cfg(test)]
mod callback_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    #[test]
    fn stop_and_expiry_cut_the_remaining_frames_inside_the_actual_render_block() {
        for stop_case in [false, true] {
            let pcm = vec![12000; 16];
            let mut c = SubmissionCursor::new(24000, pcm.len()).unwrap();
            let stop = AtomicBool::new(false);
            let deadline = AtomicU64::new(if stop_case { 100 } else { 4 });
            let progress = AtomicU64::new(0);
            let mut data = [-1.0f32; 16];
            let mut frame = 0;
            Render {
                pcm: &pcm,
                cursor: &mut c,
                allowed: &|now| {
                    !stop.load(Ordering::Acquire) && now < deadline.load(Ordering::Acquire)
                },
                progress: &progress,
                channels: 1,
            }
            .fill(
                &mut data,
                || {
                    let now = frame;
                    frame += 1;
                    if stop_case && now == 4 {
                        stop.store(true, Ordering::Release);
                    }
                    now
                },
                |v| v,
            );
            assert!(data[..4].iter().all(|v| *v > 0.0));
            assert!(data[4..].iter().all(|v| *v == 0.0));
            assert_eq!(progress.load(Ordering::Acquire), 4);
        }
    }
}

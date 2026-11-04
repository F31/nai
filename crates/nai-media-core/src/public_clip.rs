//! Exact source-sample timeline and bounded RGB32LE mouth frames; no playback authority.
use crate::public_avatar::{BuiltinAvatar, HEIGHT, WIDTH};
use nai_contracts::workbench::{PublicMediaAudio, PublicMediaFrameInfo, WorkbenchError as Error};
use sha2::{Digest, Sha256};
pub const SAMPLE_RATE: u32 = 24000;
pub const FRAME_SAMPLES: u64 = 800;
pub const MAX_SAMPLES: usize = 480000;
pub const RENDERER_ID: &str = "rms-sample-v1";
type Result<T> = std::result::Result<T, Error>;
type Check<'a> = dyn Fn() -> Result<()> + 'a;
pub fn expected_frames(samples: u64) -> Result<u64> {
    if samples == 0 || samples > MAX_SAMPLES as u64 {
        return Err(Error::InvalidArgument);
    }
    Ok(samples.div_ceil(FRAME_SAMPLES))
}
pub fn pcm_hash(pcm: &[i16], check: &Check<'_>) -> Result<String> {
    let mut h = Sha256::new();
    let mut bytes = zeroize::Zeroizing::new([0u8; 8192]);
    for block in pcm.chunks(4096) {
        check()?;
        for (n, b) in block.iter().zip(bytes.chunks_exact_mut(2)) {
            b.copy_from_slice(&n.to_le_bytes());
        }
        h.update(&bytes[..block.len() * 2]);
    }
    check()?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
pub struct RenderedPublicFrame {
    pub info: PublicMediaFrameInfo,
    pub pixels: Vec<u32>,
}
pub struct PublicClipRenderer<'a> {
    pcm: &'a [i16],
    avatar: &'a BuiltinAvatar,
    next_window: u64,
    last_cursor: u64,
    level: u32,
    ended: bool,
    invalid: bool,
}
impl<'a> PublicClipRenderer<'a> {
    pub fn new(
        pcm: &'a [i16],
        audio: &PublicMediaAudio,
        avatar: &'a BuiltinAvatar,
        check: &Check<'_>,
    ) -> Result<Self> {
        check()?;
        expected_frames(pcm.len() as u64)?;
        if audio.sample_count != pcm.len() as u64
            || audio.sample_rate != SAMPLE_RATE
            || audio.channels != 1
        {
            return Err(Error::InvalidArgument);
        }
        if pcm_hash(pcm, check)? != audio.pcm_sha256 {
            return Err(Error::IntegrityFailed);
        }
        avatar.observe(check)?;
        Ok(Self {
            pcm,
            avatar,
            next_window: 0,
            last_cursor: 0,
            level: 0,
            ended: false,
            invalid: false,
        })
    }
    /// Caller supplies consumed/source sample position, never packet arrival or wall-clock time.
    /// Skipped windows update the envelope without allocating frames or a pixel queue.
    pub fn frame_at(
        &mut self,
        cursor: u64,
        check: &Check<'_>,
    ) -> Result<Option<RenderedPublicFrame>> {
        let result = self.render(cursor, check);
        if result.is_err() {
            self.invalid = true;
        }
        result
    }
    fn render(&mut self, cursor: u64, check: &Check<'_>) -> Result<Option<RenderedPublicFrame>> {
        if self.invalid {
            return Err(Error::Cancelled);
        }
        check()?;
        if cursor < self.last_cursor || self.ended && cursor < self.pcm.len() as u64 {
            return Err(Error::RevisionConflict);
        }
        if cursor > self.pcm.len() as u64 {
            return Err(Error::InvalidArgument);
        }
        self.last_cursor = cursor;
        if cursor == self.pcm.len() as u64 {
            self.ended = true;
            return Ok(None);
        }
        let index = cursor / FRAME_SAMPLES;
        while self.next_window <= index {
            check()?;
            let start = (self.next_window * FRAME_SAMPLES) as usize;
            let end = (start + FRAME_SAMPLES as usize).min(self.pcm.len());
            let sum: u64 = self.pcm[start..end]
                .iter()
                .map(|n| {
                    let n = i64::from(*n);
                    (n * n) as u64
                })
                .sum();
            let rms = (sum / (end - start) as u64).isqrt();
            let target = ((rms.saturating_sub(200) * 65535) / 5800).min(65535) as u32;
            let tau = if target > self.level { 960 } else { 1920 };
            let n = (end - start) as u32;
            if target > self.level {
                self.level += (target - self.level) * n / (n + tau);
            } else {
                self.level -= (self.level - target) * n / (n + tau);
            }
            self.next_window += 1;
        }
        let pixels = self.avatar.mouth_pixels(self.level as u16, check)?;
        let mut h = Sha256::new();
        crate::public_avatar::update_pixels(&mut h, &pixels, check)?;
        let start = index * FRAME_SAMPLES;
        check()?;
        Ok(Some(RenderedPublicFrame {
            info: PublicMediaFrameInfo {
                frame_index: index + 1,
                sample_start: start,
                sample_count: (self.pcm.len() as u64 - start).min(FRAME_SAMPLES) as u32,
                sample_rate: SAMPLE_RATE,
                width: WIDTH as u32,
                height: HEIGHT as u32,
                pixels_sha256: h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
                openness_q16: self.level,
            },
            pixels,
        }))
    }
}
/// Canonical RGB32LE digest of a complete public window buffer.
pub fn composited_hash(pixels: &[u32], check: &dyn Fn() -> Result<()>) -> Result<String> {
    if pixels.len() != WIDTH * HEIGHT {
        return Err(Error::IntegrityFailed);
    }
    let mut h = Sha256::new();
    crate::public_avatar::update_pixels(&mut h, pixels, check)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn audio(pcm: &[i16]) -> PublicMediaAudio {
        PublicMediaAudio {
            artifact_id: "00000000-0000-4000-8000-000000000001".into(),
            pcm_sha256: pcm_hash(pcm, &|| Ok(())).unwrap(),
            sample_count: pcm.len() as u64,
            sample_rate: 24000,
            channels: 1,
        }
    }
    #[test]
    fn batched_hash_preserves_canonical_bytes_across_block_and_pixel_boundaries() {
        let pcm = [i16::MIN, -1, 0, 1, i16::MAX].repeat(1001);
        let bytes: Vec<_> = pcm.iter().flat_map(|n| n.to_le_bytes()).collect();
        let expected: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(pcm_hash(&pcm, &|| Ok(())).unwrap(), expected);
        let pixels: Vec<u32> = (0..1031).map(|n| n * 117).collect();
        let bytes: Vec<_> = pixels.iter().flat_map(|n| n.to_le_bytes()).collect();
        let mut h = Sha256::new();
        crate::public_avatar::update_pixels(&mut h, &pixels, &|| Ok(())).unwrap();
        assert_eq!(h.finalize().to_vec(), Sha256::digest(bytes).to_vec());
    }
    #[test]
    fn sample_clock_tail_and_duplicate_frame_are_exact() {
        let pcm = vec![0; 1601];
        let a = audio(&pcm);
        let avatar = BuiltinAvatar::new(&|| Ok(())).unwrap();
        let mut r = PublicClipRenderer::new(&pcm, &a, &avatar, &|| Ok(())).unwrap();
        let first = r.frame_at(0, &|| Ok(())).unwrap().unwrap();
        let same = r.frame_at(799, &|| Ok(())).unwrap().unwrap();
        assert_eq!(first.info, same.info);
        assert_eq!(first.pixels, same.pixels);
        let second = r.frame_at(800, &|| Ok(())).unwrap().unwrap();
        let last = r.frame_at(1600, &|| Ok(())).unwrap().unwrap();
        assert_eq!(
            (
                first.info.sample_start,
                second.info.sample_start,
                last.info.sample_start
            ),
            (0, 800, 1600)
        );
        assert_eq!(last.info.sample_count, 1);
        assert_eq!(expected_frames(1601).unwrap(), 3);
        assert_eq!(first.info.openness_q16, 0);
        assert!(r.frame_at(1601, &|| Ok(())).unwrap().is_none());
        assert!(r.frame_at(0, &|| Ok(())).is_err());
    }
    #[test]
    fn actual_loud_pcm_opens_mouth_and_skipping_matches_sequential_envelope() {
        let mut pcm = vec![12000; 1600];
        pcm.extend(vec![0; 1600]);
        let a = audio(&pcm);
        let avatar = BuiltinAvatar::new(&|| Ok(())).unwrap();
        let mut full = PublicClipRenderer::new(&pcm, &a, &avatar, &|| Ok(())).unwrap();
        let first = full.frame_at(0, &|| Ok(())).unwrap().unwrap();
        assert!(first.info.openness_q16 > 0);
        assert_ne!(first.pixels, avatar.idle_pixels());
        full.frame_at(800, &|| Ok(())).unwrap();
        full.frame_at(1600, &|| Ok(())).unwrap();
        let end = full.frame_at(2400, &|| Ok(())).unwrap().unwrap();
        assert!(end.info.openness_q16 < first.info.openness_q16);
        let mut skip = PublicClipRenderer::new(&pcm, &a, &avatar, &|| Ok(())).unwrap();
        let jumped = skip.frame_at(2400, &|| Ok(())).unwrap().unwrap();
        assert_eq!(end.info, jumped.info);
        assert_eq!(end.pixels, jumped.pixels);
    }
    #[test]
    fn cancellation_regression_corruption_and_capacity_fail_closed() {
        let pcm = vec![1; 1600];
        let a = audio(&pcm);
        let avatar = BuiltinAvatar::new(&|| Ok(())).unwrap();
        let mut r = PublicClipRenderer::new(&pcm, &a, &avatar, &|| Ok(())).unwrap();
        assert_eq!(
            r.frame_at(0, &|| Err(Error::Cancelled)).err(),
            Some(Error::Cancelled)
        );
        assert_eq!(r.frame_at(0, &|| Ok(())).err(), Some(Error::Cancelled));
        let mut r = PublicClipRenderer::new(&pcm, &a, &avatar, &|| Ok(())).unwrap();
        r.frame_at(800, &|| Ok(())).unwrap();
        assert!(r.frame_at(1, &|| Ok(())).is_err());
        let mut wrong = a;
        wrong.pcm_sha256 = "f".repeat(64);
        assert!(PublicClipRenderer::new(&pcm, &wrong, &avatar, &|| Ok(())).is_err());
        assert_eq!(expected_frames(480000).unwrap(), 600);
        assert!(expected_frames(480001).is_err());
        assert!(expected_frames(0).is_err());
    }
}

//! Bounded local buffer observations, never proof of OS presentation or capture.
use serde::Serialize;
use std::{
    collections::VecDeque,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    Frame,
    Standby,
    Idle,
    SafeBlack,
}
#[derive(Serialize)]
struct Row {
    utc_ms: u128,
    elapsed_ms: u128,
    phase: Phase,
    source_size: (usize, usize),
    native_size: (usize, usize),
    focused: bool,
    sampled_pixels: u32,
    nonblack_samples: u32,
    max_rgb: u32,
}
pub(crate) struct Witness {
    path: PathBuf,
    origin: Instant,
    last: Option<Instant>,
    rows: VecDeque<Row>,
    dropped: u64,
    seen_frame: bool,
}
fn sample(pixels: &[u32], width: usize, height: usize) -> Option<(u32, u32, u32)> {
    if width == 0 || height == 0 || width.checked_mul(height)? != pixels.len() {
        return None;
    }
    let (mut nonblack, mut max_rgb) = (0, 0);
    for y in 0..16 {
        for x in 0..16 {
            let px = ((2 * x + 1) * width / 32).min(width - 1);
            let py = ((2 * y + 1) * height / 32).min(height - 1);
            let value = pixels[py * width + px];
            let largest = ((value >> 16) & 255)
                .max((value >> 8) & 255)
                .max(value & 255);
            nonblack += u32::from(largest > 16);
            max_rgb = max_rgb.max(largest);
        }
    }
    Some((256, nonblack, max_rgb))
}
impl Witness {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            origin: Instant::now(),
            last: None,
            rows: VecDeque::new(),
            dropped: 0,
            seen_frame: false,
        }
    }
    pub(crate) fn observe(
        &mut self,
        pixels: &[u32],
        size: (usize, usize),
        native: (usize, usize),
        focused: bool,
        phase: Phase,
    ) {
        self.observe_at(Instant::now(), pixels, size, native, focused, phase);
    }
    fn observe_at(
        &mut self,
        now: Instant,
        pixels: &[u32],
        size: (usize, usize),
        native: (usize, usize),
        focused: bool,
        phase: Phase,
    ) {
        let urgent = ((native.0 == 0 || native.1 == 0)
            && self
                .rows
                .back()
                .is_none_or(|r| r.native_size.0 > 0 && r.native_size.1 > 0))
            || (phase == Phase::Frame && !self.seen_frame)
            || (phase == Phase::SafeBlack
                && self.rows.back().is_none_or(|r| r.phase != Phase::SafeBlack));
        if !urgent
            && self
                .last
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(1))
        {
            return;
        }
        let Some((sampled_pixels, nonblack_samples, max_rgb)) = sample(pixels, size.0, size.1)
        else {
            return;
        };
        self.last = Some(now);
        self.seen_frame |= phase == Phase::Frame;
        if self.rows.len() == 128 {
            self.rows.pop_front();
            self.dropped += 1;
        }
        self.rows.push_back(Row {
            utc_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis()),
            elapsed_ms: now.saturating_duration_since(self.origin).as_millis(),
            phase,
            source_size: size,
            native_size: native,
            focused,
            sampled_pixels,
            nonblack_samples,
            max_rgb,
        });
    }
}
impl Drop for Witness {
    fn drop(&mut self) {
        if let Ok(file) = std::fs::File::create_new(&self.path) {
            let _ = serde_json::to_writer(
                file,
                &serde_json::json!({"scope":"submitted_buffer_samples_not_os_or_obs_proof","version":1,"dropped_rows":self.dropped,"rows":self.rows}),
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_and_near_black_do_not_claim_visible_rgb() {
        assert_eq!(sample(&[0xff000000; 16], 4, 4), Some((256, 0, 0)));
        assert_eq!(sample(&[0x00101010; 16], 4, 4), Some((256, 0, 16)));
        assert_eq!(sample(&[0x0000ff00; 16], 4, 4), Some((256, 256, 255)));
        assert_eq!(sample(&[1; 15], 4, 4), None);
        assert_eq!(sample(&[], 0, 0), None);
    }
    #[test]
    fn cadence_and_ring_are_bounded_without_hiding_zero_size_native() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("witness.json");
        let mut w = Witness::new(path.clone());
        let origin = w.origin;
        for i in 0..200 {
            w.observe_at(
                origin + Duration::from_secs(i),
                &[0x00ffffff],
                (1, 1),
                (0, 0),
                false,
                Phase::Frame,
            );
        }
        w.observe_at(
            origin + Duration::from_secs(199),
            &[0],
            (1, 1),
            (1, 1),
            true,
            Phase::Frame,
        );
        assert_eq!(w.rows.len(), 128);
        assert_eq!(w.dropped, 72);
        assert_eq!(w.rows.back().unwrap().native_size, (0, 0));
        drop(w);
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["rows"].as_array().unwrap().len(), 128);
        assert_eq!(value["rows"][127]["nonblack_samples"], 256);
    }
    #[test]
    fn first_real_frame_and_explicit_safety_black_survive_sampling_cadence() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Witness::new(dir.path().join("w.json"));
        let at = w.origin;
        w.observe_at(at, &[0], (1, 1), (1, 1), false, Phase::Standby);
        w.observe_at(
            at + Duration::from_millis(10),
            &[0x00ffffff],
            (1, 1),
            (1, 1),
            false,
            Phase::Frame,
        );
        w.observe_at(
            at + Duration::from_millis(20),
            &[0],
            (1, 1),
            (1, 1),
            false,
            Phase::SafeBlack,
        );
        assert_eq!(w.rows.len(), 3);
        assert_eq!(w.rows[1].nonblack_samples, 256);
        assert_eq!(w.rows[2].nonblack_samples, 0);
    }
}

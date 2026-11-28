//! Immutable still content and an independently revocable local display view.
//! No audio, FrameRef, device clock, old task or execution permit is retained.
use nai_contracts::workbench::WorkbenchError as Error;
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

pub(crate) struct Image {
    pixels: Vec<u32>,
    until: Instant,
    width: u32,
    height: u32,
    source: [u8; 32],
    digest: [u8; 32],
}
impl Image {
    #[cfg(test)]
    pub(crate) fn expires_at(&self) -> Instant {
        self.until
    }
    pub(crate) fn verified(
        width: u32,
        height: u32,
        bytes: &[u8],
        digest: [u8; 32],
        source: [u8; 32],
        until: Instant,
    ) -> Result<Self, Error> {
        if ![width, height]
            .iter()
            .all(|v| (256..=1024).contains(v) && v.is_multiple_of(32))
            || bytes.len() != width as usize * height as usize * 4
            || Sha256::digest(bytes).as_slice() != digest
            || Instant::now() >= until
        {
            return Err(Error::ProtocolError);
        }
        let pixels = bytes
            .chunks_exact(4)
            .map(|p| u32::from_le_bytes([p[0], p[1], p[2], 0]))
            .collect();
        Ok(Self {
            pixels,
            until,
            width,
            height,
            source,
            digest,
        })
    }
    pub(crate) fn restrict_until(&mut self, until: Instant) {
        self.until = self.until.min(until);
    }
    pub(crate) fn metadata(&self) -> serde_json::Value {
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        serde_json::json!({"scope":"session_private_static_standby_not_media_authority", "width":self.width,"height":self.height,
            "source_identity_sha256":hex(&self.source),"bgra_sha256":hex(&self.digest)})
    }
}
pub(crate) struct View {
    image: Arc<Image>,
    generation: Arc<AtomicU64>,
    expected: u64,
    worker_alive: Arc<AtomicBool>,
    enabled: AtomicBool,
}
impl View {
    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (self.image.width as usize, self.image.height as usize)
    }
    pub(crate) fn bind(
        image: Arc<Image>,
        generation: Arc<AtomicU64>,
        expected: u64,
        worker_alive: Arc<AtomicBool>,
    ) -> Self {
        Self {
            image,
            generation,
            expected,
            worker_alive,
            enabled: AtomicBool::new(true),
        }
    }
    pub(crate) fn revoke(&self) {
        self.enabled.store(false, Ordering::Release);
    }
    pub(crate) fn pixels(&self) -> Option<&[u32]> {
        let valid = self.expected != 0
            && self.enabled.load(Ordering::Acquire)
            && self.worker_alive.load(Ordering::Acquire)
            && self.generation.load(Ordering::Acquire) == self.expected
            && Instant::now() < self.image.until;
        if !valid {
            self.revoke();
            return None;
        }
        Some(self.image.pixels.as_slice())
    }
}
pub(crate) fn fill(screen: &mut [u32], view: Option<&View>) -> bool {
    if let Some(pixels) = view
        .and_then(View::pixels)
        .filter(|p| p.len() == screen.len())
    {
        screen.copy_from_slice(pixels);
        true
    } else {
        screen.fill(0);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn image() -> Image {
        let bytes = vec![17; 256 * 256 * 4];
        Image::verified(
            256,
            256,
            &bytes,
            Sha256::digest(&bytes).into(),
            [9; 32],
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap()
    }
    #[test]
    fn malformed_size_hash_and_expiry_never_create_still() {
        let b = vec![17; 256 * 256 * 4];
        let until = Instant::now() + Duration::from_secs(10);
        assert!(Image::verified(256, 256, &b[..b.len() - 1], [0; 32], [9; 32], until).is_err());
        assert!(Image::verified(256, 256, &b, [0; 32], [9; 32], until).is_err());
        assert!(
            Image::verified(
                256,
                256,
                &b,
                Sha256::digest(&b).into(),
                [9; 32],
                Instant::now()
            )
            .is_err()
        );
        assert_eq!(image().pixels[0], 0x00111111);
    }
    #[test]
    fn stop_generation_and_local_revoke_cannot_auto_restore() {
        let image = Arc::new(image());
        let g = Arc::new(AtomicU64::new(1));
        let alive = Arc::new(AtomicBool::new(true));
        let old = View::bind(image.clone(), g.clone(), 1, alive.clone());
        assert!(old.pixels().is_some());
        g.store(2, Ordering::Release);
        assert!(old.pixels().is_none());
        let new = View::bind(image, g, 2, alive);
        assert!(new.pixels().is_some());
        new.revoke();
        assert!(new.pixels().is_none());
        assert!(old.pixels().is_none());
    }
    #[test]
    fn worker_loss_or_plan_expiry_denies_view() {
        let g = Arc::new(AtomicU64::new(1));
        let alive = Arc::new(AtomicBool::new(true));
        let view = View::bind(Arc::new(image()), g.clone(), 1, alive.clone());
        alive.store(false, Ordering::Release);
        assert!(view.pixels().is_none());
        alive.store(true, Ordering::Release);
        assert!(view.pixels().is_none()); // a lost witness cannot resurrect an old view
        let mut expired = image();
        expired.restrict_until(Instant::now());
        assert!(
            View::bind(Arc::new(expired), g, 1, alive)
                .pixels()
                .is_none()
        );
    }
    #[test]
    fn revoked_or_mismatched_view_fills_safe_black() {
        let view = View::bind(
            Arc::new(image()),
            Arc::new(AtomicU64::new(1)),
            1,
            Arc::new(AtomicBool::new(true)),
        );
        let mut screen = vec![0; 256 * 256];
        assert!(fill(&mut screen, Some(&view)));
        assert_eq!(screen[0], 0x00111111);
        let mut wrong = vec![9; 3];
        assert!(!fill(&mut wrong, Some(&view)));
        assert_eq!(wrong, vec![0; 3]);
        view.revoke();
        assert!(!fill(&mut screen, Some(&view)));
        assert!(screen.iter().all(|p| *p == 0));
    }
}

//! Original bounded geometric artwork, with its own embedded license.
//! Resource admission only: no speech, output permission, files or device access.
use nai_contracts::workbench::{PublicMediaVisual, WorkbenchError as Error};
use sha2::{Digest, Sha256};
pub const WIDTH: usize = 512;
pub const HEIGHT: usize = 288;
pub const RESOURCE_ID: &str = "d074f2a8-a618-4b43-9e24-679bf43a0001";
pub const NAME: &str = "内置简洁2D形象";
pub const LICENSE_ID: &str = "MIT";
pub const LICENSE_TEXT: &str = include_str!("../../../assets/public-avatar/LICENSE.txt");
type Result<T> = std::result::Result<T, Error>;
type Check<'a> = dyn Fn() -> Result<()> + 'a;
pub struct BuiltinAvatar {
    closed: Vec<u32>,
    open: Vec<u32>,
    metadata: PublicMediaVisual,
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn ellipse(x: i32, y: i32, cx: i32, cy: i32, rx: i32, ry: i32) -> bool {
    let dx = i64::from(x - cx);
    let dy = i64::from(y - cy);
    dx * dx * i64::from(ry * ry) + dy * dy * i64::from(rx * rx)
        <= i64::from(rx * rx) * i64::from(ry * ry)
}
fn draw(mouth_open: bool, check: &Check<'_>) -> Result<Vec<u32>> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(WIDTH * HEIGHT)
        .map_err(|_| Error::ResourceExhausted)?;
    for y in 0..HEIGHT as i32 {
        check()?;
        for x in 0..WIDTH as i32 {
            let mut c = 0x111827;
            if ellipse(x, y, 256, 254, 104, 83) {
                c = 0x3b82f6;
            }
            if (242..=270).contains(&x) && (146..=191).contains(&y) {
                c = 0xe7ac82;
            }
            if ellipse(x, y, 256, 103, 61, 65) {
                c = 0x26364b;
            }
            if ellipse(x, y, 256, 111, 52, 55) {
                c = 0xf3c7a4;
            }
            if ellipse(x, y, 249, 59, 58, 24) {
                c = 0x26364b;
            }
            if ellipse(x, y, 235, 106, 5, 6) || ellipse(x, y, 277, 106, 5, 6) {
                c = 0x162238;
            }
            if (254..=258).contains(&x) && (112..=120).contains(&y) {
                c = 0xd59b79;
            }
            if mouth_open {
                if ellipse(x, y, 256, 134, 13, 9) {
                    c = 0x672e37;
                }
                if ellipse(x, y, 256, 138, 8, 3) {
                    c = 0xe58d98;
                }
            } else if (245..=267).contains(&x) && (133..=135).contains(&y) {
                c = 0x914c50;
            }
            if (254..=258).contains(&x) && (195..=231).contains(&y) {
                c = 0xbed9ff;
            }
            pixels.push(c);
        }
    }
    check()?;
    Ok(pixels)
}
pub(crate) fn update_pixels(h: &mut Sha256, pixels: &[u32], check: &Check<'_>) -> Result<()> {
    let mut bytes = [0u8; 2048];
    for row in pixels.chunks(512) {
        check()?;
        for (p, b) in row.iter().zip(bytes.chunks_exact_mut(4)) {
            b.copy_from_slice(&p.to_le_bytes());
        }
        h.update(&bytes[..row.len() * 4]);
    }
    Ok(())
}
fn package(closed: &[u32], open: &[u32], check: &Check<'_>) -> Result<String> {
    if closed.len() != WIDTH * HEIGHT || open.len() != WIDTH * HEIGHT {
        return Err(Error::IntegrityFailed);
    }
    let mut h = Sha256::new();
    h.update(b"nai-original-geometric-avatar-rgb888-v1\0");
    for n in [WIDTH as u32, HEIGHT as u32, 30] {
        h.update(n.to_le_bytes());
    }
    for pixels in [closed, open] {
        update_pixels(&mut h, pixels, check)?;
    }
    check()?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
impl BuiltinAvatar {
    pub fn new(check: &Check<'_>) -> Result<Self> {
        check()?;
        let closed = draw(false, check)?;
        let open = draw(true, check)?;
        let metadata = PublicMediaVisual {
            artifact_id: RESOURCE_ID.into(),
            package_sha256: package(&closed, &open, check)?,
            license_sha256: digest(LICENSE_TEXT.as_bytes()),
            width: WIDTH as u32,
            height: HEIGHT as u32,
            fps: 30,
        };
        Ok(Self {
            closed,
            open,
            metadata,
        })
    }
    /// Rehashes actual immutable textures and the embedded license; no cached Ready boolean.
    pub fn observe(&self, check: &Check<'_>) -> Result<PublicMediaVisual> {
        check()?;
        if package(&self.closed, &self.open, check)? != self.metadata.package_sha256
            || digest(LICENSE_TEXT.as_bytes()) != self.metadata.license_sha256
        {
            return Err(Error::IntegrityFailed);
        }
        check()?;
        Ok(self.metadata.clone())
    }
    /// Neutral fixed artwork only; no business data, voice or implicit media permission.
    pub fn idle_pixels(&self) -> &[u32] {
        &self.closed
    }
    /// Bounded integer blend of original textures. No clock, device or authorization inferred.
    pub fn mouth_pixels(&self, openness: u16, check: &Check<'_>) -> Result<Vec<u32>> {
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(WIDTH * HEIGHT)
            .map_err(|_| Error::ResourceExhausted)?;
        let level = u32::from(openness);
        for (row, (closed, open)) in self
            .closed
            .chunks(WIDTH)
            .zip(self.open.chunks(WIDTH))
            .enumerate()
        {
            check()?;
            if row >= HEIGHT {
                return Err(Error::IntegrityFailed);
            }
            for (a, b) in closed.iter().zip(open) {
                if level == 0 || a == b {
                    pixels.push(*a);
                    continue;
                }
                if level == 65535 {
                    pixels.push(*b);
                    continue;
                }
                let mut p = 0;
                for shift in [0, 8, 16] {
                    let a = (a >> shift) & 255;
                    let b = (b >> shift) & 255;
                    p |= ((a * (65535 - level) + b * level + 32767) / 65535) << shift;
                }
                pixels.push(p);
            }
        }
        check()?;
        Ok(pixels)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_pixels_and_independent_license_are_stable_and_bounded() {
        let a = BuiltinAvatar::new(&|| Ok(())).unwrap();
        let b = BuiltinAvatar::new(&|| Ok(())).unwrap();
        let m = a.observe(&|| Ok(())).unwrap();
        assert_eq!(m, b.observe(&|| Ok(())).unwrap());
        assert_eq!((m.width, m.height, m.fps), (512, 288, 30));
        assert_eq!(a.idle_pixels().len(), 512 * 288);
        assert_eq!(a.closed[0], 0x111827);
        assert_eq!(a.closed[106 * WIDTH + 235], 0x162238);
        assert_ne!(a.closed[134 * WIDTH + 256], a.open[134 * WIDTH + 256]);
        assert!(a.closed.iter().all(|p| *p <= 0xffffff));
        assert!(LICENSE_TEXT.contains("Permission is hereby granted"));
        assert!(LICENSE_TEXT.contains("specifically to the original geometric avatar"));
    }
    #[test]
    fn corruption_and_cancellation_refuse_observation_and_drawing() {
        let mut a = BuiltinAvatar::new(&|| Ok(())).unwrap();
        a.open[134 * WIDTH + 256] ^= 1;
        assert_eq!(a.observe(&|| Ok(())), Err(Error::IntegrityFailed));
        assert_eq!(a.observe(&|| Err(Error::Cancelled)), Err(Error::Cancelled));
        let calls = std::cell::Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() > 5 {
                Err(Error::DeadlineExceeded)
            } else {
                Ok(())
            }
        };
        assert!(matches!(
            BuiltinAvatar::new(&check),
            Err(Error::DeadlineExceeded)
        ));
        assert!(calls.get() <= 6);
    }
}

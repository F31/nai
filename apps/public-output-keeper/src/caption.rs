//! Fixed Windows system font only. Font parsing/rasterization never enters the SDK callback.
use fontdue::{Font, FontSettings};
use nai_contracts::workbench::{PublicCaptionState as State, WorkbenchError as Error};
use std::sync::{Arc, OnceLock};
#[derive(Debug)]
pub struct Failure {
    pub code: Error,
    pub caption: State,
}
impl From<Error> for Failure {
    fn from(code: Error) -> Self {
        Self {
            code,
            caption: State::Unspecified,
        }
    }
}
fn fail(code: Error, caption: State) -> Failure {
    Failure { code, caption }
}
struct SystemFont {
    font: Font,
    name: String,
    sha: String,
}
static FONT: OnceLock<Result<Arc<SystemFont>, Failure>> = OnceLock::new();
fn system_font() -> Result<Arc<SystemFont>, Failure> {
    let cached = FONT.get_or_init(|| {
        let root = std::env::var_os("SystemRoot")
            .ok_or_else(|| fail(Error::CapabilityUnavailable, State::FontUnavailable))?;
        let root = std::path::PathBuf::from(root);
        if !root.is_absolute() {
            return Err(fail(Error::CapabilityUnavailable, State::FontUnavailable));
        }
        for name in ["simhei.ttf", "msyh.ttc"] {
            let path = root.join("Fonts").join(name);
            let Ok(m) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            use std::os::windows::fs::MetadataExt;
            if !m.is_file() || m.len() > 32 * 1024 * 1024 || m.file_attributes() & 0x400 != 0 {
                continue;
            }
            let bytes = std::fs::read(&path)
                .map_err(|_| fail(Error::CapabilityUnavailable, State::FontUnavailable))?;
            use sha2::{Digest, Sha256};
            let sha = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            let font = Font::from_bytes(
                bytes,
                FontSettings {
                    collection_index: 0,
                    scale: 20.0,
                    load_substitutions: false,
                },
            )
            .map_err(|_| fail(Error::CapabilityUnavailable, State::FontUnavailable))?;
            return Ok(Arc::new(SystemFont {
                font,
                name: format!("windows-system-font:{name}"),
                sha,
            }));
        }
        Err(fail(Error::CapabilityUnavailable, State::FontUnavailable))
    });
    cached
        .as_ref()
        .map(Arc::clone)
        .map_err(|e| fail(e.code, e.caption))
}
pub struct Caption {
    pub sha: String,
    pub font: String,
    pub font_sha: String,
    alpha: Vec<u8>,
    height: usize,
}
impl Caption {
    pub fn new(
        text: &str,
        hash: &str,
        check: &dyn Fn() -> Result<(), Error>,
    ) -> Result<Self, Failure> {
        check()?;
        nai_media_core::public_caption::validate_source(text, hash).map_err(|e| {
            fail(
                e,
                if e == Error::IntegrityFailed {
                    State::IntegrityRejected
                } else {
                    State::TextRejected
                },
            )
        })?;
        let system = system_font()?;
        check()?;
        let font = &system.font;
        let mut alpha = vec![0u8; 512 * 88];
        let mut x = 12usize;
        let mut row = 0usize;
        for c in text.chars() {
            check()?;
            if c == '\n' {
                row += 1;
                x = 12;
                if row >= 3 {
                    return Err(fail(Error::ResourceExhausted, State::LayoutOverflow));
                }
                continue;
            }
            if !font.has_glyph(c) {
                return Err(fail(Error::CapabilityUnavailable, State::MissingGlyph));
            }
            let m = font.metrics(c, 20.0);
            if !m.advance_width.is_finite()
                || m.advance_width < 0.0
                || m.advance_width > 64.0
                || m.width > 64
                || m.height > 64
            {
                return Err(fail(Error::CapabilityUnavailable, State::FontUnavailable));
            }
            let advance = m.advance_width.ceil() as usize;
            if x + advance > 500 {
                row += 1;
                x = 12;
            }
            if row >= 3 {
                return Err(fail(Error::ResourceExhausted, State::LayoutOverflow));
            }
            let (m, bitmap) = font.rasterize(c, 20.0);
            let left = x as i32 + m.xmin;
            let top = (28 + row * 24) as i32 - m.height as i32 - m.ymin;
            if bitmap.len() != m.width * m.height
                || left < 0
                || top < 0
                || left + m.width as i32 > 512
                || top + m.height as i32 > 88
            {
                return Err(fail(Error::ResourceExhausted, State::LayoutOverflow));
            }
            for y in 0..m.height {
                for px in 0..m.width {
                    alpha[(top as usize + y) * 512 + left as usize + px] = bitmap[y * m.width + px];
                }
            }
            x += advance;
        }
        let height = (row + 1) * 24 + 16;
        alpha.truncate(512 * height);
        check()?;
        Ok(Self {
            sha: hash.into(),
            font: system.name.clone(),
            font_sha: system.sha.clone(),
            alpha,
            height,
        })
    }
    pub fn compose(
        &self,
        pixels: &mut [u32],
        check: &dyn Fn() -> Result<(), Error>,
    ) -> Result<(), Error> {
        if pixels.len() != 512 * 288 {
            return Err(Error::IntegrityFailed);
        }
        for y in 0..self.height {
            check()?;
            for x in 0..512 {
                let i = (288 - self.height + y) * 512 + x;
                let a = u32::from(self.alpha[y * 512 + x]);
                let p = pixels[i];
                let mut out = 0;
                for shift in [0, 8, 16] {
                    let bg = ((p >> shift) & 255) * 2 / 5;
                    out |= ((bg * (255 - a) + 255 * a + 127) / 255) << shift;
                }
                pixels[i] = out;
            }
        }
        check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_system_font_renders_complete_chinese_and_binds_composite() {
        let text = "容量为500毫升。\n长度为10厘米。";
        let hash = nai_contracts::public_caption_hash(text);
        let c = Caption::new(text, &hash, &|| Ok(())).unwrap();
        assert_eq!(c.sha, hash);
        assert_eq!(c.font_sha.len(), 64);
        assert!(c.alpha.iter().filter(|a| **a > 0).count() > 100);
        assert!(c.height <= 88);
        let avatar = nai_media_core::public_avatar::BuiltinAvatar::new(&|| Ok(())).unwrap();
        let mut pixels = avatar.idle_pixels().to_vec();
        let original = pixels.clone();
        c.compose(&mut pixels, &|| Ok(())).unwrap();
        assert_ne!(pixels, original);
        assert_eq!(&pixels[..200 * 512], &original[..200 * 512]);
        if let Some(path) = std::env::var_os("NAI_CAPTION_QA_OUT") {
            let out = std::fs::File::create(path).unwrap();
            let mut encoder = png::Encoder::new(out, 512, 288);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let data: Vec<u8> = pixels
                .iter()
                .flat_map(|p| {
                    [
                        ((p >> 16) & 255) as u8,
                        ((p >> 8) & 255) as u8,
                        (p & 255) as u8,
                    ]
                })
                .collect();
            writer.write_image_data(&data).unwrap();
        }
    }
    #[test]
    fn changed_text_missing_glyph_overflow_and_cancel_never_silently_replace() {
        let t = "容量为500毫升。";
        assert!(matches!(
            Caption::new(t, &"a".repeat(64), &|| Ok(())),
            Err(Failure {
                caption: State::IntegrityRejected,
                ..
            })
        ));
        let t = "\u{10ffff}";
        assert!(matches!(
            Caption::new(t, &nai_contracts::public_caption_hash(t), &|| Ok(())),
            Err(Failure {
                caption: State::MissingGlyph,
                ..
            })
        ));
        let t = "中".repeat(256);
        assert!(matches!(
            Caption::new(&t, &nai_contracts::public_caption_hash(&t), &|| Ok(())),
            Err(Failure {
                caption: State::LayoutOverflow,
                ..
            })
        ));
        assert!(matches!(
            Caption::new(
                "test",
                &nai_contracts::public_caption_hash("test"),
                &|| Err(Error::Cancelled)
            ),
            Err(Failure {
                code: Error::Cancelled,
                ..
            })
        ));
    }
}

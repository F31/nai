//! Private preview bytes, separate from JSON/protobuf control messages.
//! A preview is never a rights grant or an output authorization.

use std::sync::{Arc, Mutex};

const MAX_PREVIEW_BYTES: usize = 16 * 1024 * 1024;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

struct Preview {
    workspace_id: String,
    package_sha256: String,
    license_sha256: String,
    bytes: Arc<[u8]>,
    content_type: &'static str,
}

#[derive(Default)]
pub(crate) struct PreviewSlot(Mutex<Option<Preview>>);

fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl PreviewSlot {
    /// Caller must supply freshly admitted pixels and trusted workspace identity.
    pub(crate) fn install(
        &self,
        workspace_id: &str,
        package_sha256: &str,
        license_sha256: &str,
        bytes: Vec<u8>,
    ) -> Result<(), &'static str> {
        if workspace_id.trim().is_empty()
            || workspace_id.len() > 128
            || !digest(package_sha256)
            || !digest(license_sha256)
            || bytes.len() > MAX_PREVIEW_BYTES
            || !bytes.starts_with(PNG_SIGNATURE)
        {
            return Err("AVATAR_PREVIEW_REJECTED");
        }
        let mut slot = self.0.lock().map_err(|_| "AVATAR_PREVIEW_UNAVAILABLE")?;
        *slot = Some(Preview {
            workspace_id: workspace_id.to_owned(),
            package_sha256: package_sha256.to_owned(),
            license_sha256: license_sha256.to_owned(),
            bytes: bytes.into(),
            content_type: "image/png",
        });
        Ok(())
    }

    /// Stage a private photo only after matching its bounded identity.
    pub(crate) fn install_photo(
        &self,
        workspace_id: &str,
        sha: &str,
        bytes: Vec<u8>,
    ) -> Result<(), &'static str> {
        if workspace_id.trim().is_empty() || workspace_id.len() > 128 || !digest(sha) {
            return Err("AVATAR_PREVIEW_REJECTED");
        }
        let meta =
            nai_avatar_files::photo::inspect_jpeg(&bytes).map_err(|_| "AVATAR_PREVIEW_REJECTED")?;
        if meta.sha256 != sha {
            return Err("AVATAR_PREVIEW_REJECTED");
        }
        let mut slot = self.0.lock().map_err(|_| "AVATAR_PREVIEW_UNAVAILABLE")?;
        *slot = Some(Preview {
            workspace_id: workspace_id.to_owned(),
            package_sha256: sha.to_owned(),
            license_sha256: sha.to_owned(),
            bytes: bytes.into(),
            content_type: "image/jpeg",
        });
        Ok(())
    }
    /// Stage a private source video only after matching its bounded identity.
    pub(crate) fn install_video(
        &self,
        workspace_id: &str,
        sha: &str,
        bytes: Vec<u8>,
    ) -> Result<(), &'static str> {
        if workspace_id.trim().is_empty() || workspace_id.len() > 128 || !digest(sha) {
            return Err("AVATAR_PREVIEW_REJECTED");
        }
        let meta =
            nai_avatar_files::video::inspect_mp4(&bytes).map_err(|_| "AVATAR_PREVIEW_REJECTED")?;
        if meta.sha256 != sha {
            return Err("AVATAR_PREVIEW_REJECTED");
        }
        let mut slot = self.0.lock().map_err(|_| "AVATAR_PREVIEW_UNAVAILABLE")?;
        *slot = Some(Preview {
            workspace_id: workspace_id.to_owned(),
            package_sha256: sha.to_owned(),
            license_sha256: sha.to_owned(),
            bytes: bytes.into(),
            content_type: "video/mp4",
        });
        Ok(())
    }
    /// Exact identity and MIME are read under the same lock.
    fn read_with_type(
        &self,
        workspace_id: &str,
        package_sha256: &str,
        license_sha256: &str,
    ) -> Result<(Arc<[u8]>, &'static str), &'static str> {
        let slot = self.0.lock().map_err(|_| "AVATAR_PREVIEW_UNAVAILABLE")?;
        let preview = slot
            .as_ref()
            .filter(|p| {
                p.workspace_id == workspace_id
                    && p.package_sha256 == package_sha256
                    && p.license_sha256 == license_sha256
            })
            .ok_or("AVATAR_PREVIEW_UNAVAILABLE")?;
        Ok((preview.bytes.clone(), preview.content_type))
    }
    #[cfg(test)]
    pub(crate) fn read(
        &self,
        workspace_id: &str,
        package_sha256: &str,
        license_sha256: &str,
    ) -> Result<Arc<[u8]>, &'static str> {
        self.read_with_type(workspace_id, package_sha256, license_sha256)
            .map(|(bytes, _)| bytes)
    }

    /// Prevent future reads; bytes already returned to a caller cannot be recalled.
    pub(crate) fn clear(&self) -> Result<(), &'static str> {
        *self.0.lock().map_err(|_| "AVATAR_PREVIEW_UNAVAILABLE")? = None;
        Ok(())
    }
}

/// Private binary response; only the main webview and exact staged identity.
pub(crate) fn response(
    slot: &PreviewSlot,
    label: &str,
    method: &str,
    path: &str,
) -> tauri::http::Response<Vec<u8>> {
    let mut parts = path.strip_prefix('/').unwrap_or("").split('/');
    let workspace = parts.next().unwrap_or("");
    let package = parts.next().unwrap_or("");
    let rights = parts.next().unwrap_or("");
    let data = if label == "main" && method == "GET" && parts.next().is_none() {
        slot.read_with_type(workspace, package, rights).ok()
    } else {
        None
    };
    let mut builder = tauri::http::Response::builder()
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if let Some((bytes, content_type)) = data {
        builder = builder.status(200).header("Content-Type", content_type);
        builder
            .body(bytes.to_vec())
            .expect("static response headers")
    } else {
        builder
            .status(404)
            .body(Vec::new())
            .expect("static response headers")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_preview_requires_exact_identity_and_clears_on_replacement() {
        let slot = PreviewSlot::default();
        let package = "a".repeat(64);
        let license = "b".repeat(64);
        let next = "c".repeat(64);
        let png = PNG_SIGNATURE.to_vec();
        slot.install("workspace", &package, &license, png.clone())
            .unwrap();
        assert_eq!(&*slot.read("workspace", &package, &license).unwrap(), png);
        assert!(slot.read("other", &package, &license).is_err());
        assert!(slot.read("workspace", &package, &next).is_err());
        slot.install("workspace", &next, &license, png).unwrap();
        assert!(slot.read("workspace", &package, &license).is_err());
        slot.clear().unwrap();
        assert!(slot.read("workspace", &next, &license).is_err());
    }

    #[test]
    fn invalid_preview_does_not_replace_existing_slot() {
        let slot = PreviewSlot::default();
        let package = "a".repeat(64);
        let license = "b".repeat(64);
        slot.install("workspace", &package, &license, PNG_SIGNATURE.to_vec())
            .unwrap();
        assert!(
            slot.install(
                "workspace",
                &package,
                &license,
                vec![0; MAX_PREVIEW_BYTES + 1]
            )
            .is_err()
        );
        assert!(
            slot.install("workspace", &package, &license, b"not png".to_vec())
                .is_err()
        );
        assert!(
            slot.install(
                "workspace",
                &"A".repeat(64),
                &license,
                PNG_SIGNATURE.to_vec()
            )
            .is_err()
        );
        assert!(slot.read("workspace", &package, &license).is_ok());
    }

    #[test]
    fn photo_preview_is_private_and_delivered_as_jpeg() {
        let jpeg = vec![
            0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 0, 2, 0, 3, 1, 1, 0x11, 0, 0xff, 0xda, 0, 8, 1, 1, 0,
            0, 63, 0, 0, 0xff, 0xd9,
        ];
        let digest = nai_avatar_files::photo::inspect_jpeg(&jpeg).unwrap().sha256;
        let slot = PreviewSlot::default();
        slot.install_photo("workspace", &digest, jpeg.clone())
            .unwrap();
        let path = format!("/workspace/{digest}/{digest}");
        let answer = response(&slot, "main", "GET", &path);
        assert_eq!(answer.status(), 200);
        assert_eq!(answer.headers().get("content-type").unwrap(), "image/jpeg");
        assert_eq!(answer.body(), &jpeg);
        assert_eq!(response(&slot, "other", "GET", &path).status(), 404);
        assert!(
            slot.install_photo("workspace", &"a".repeat(64), jpeg)
                .is_err()
        );
        slot.clear().unwrap();
        assert_eq!(response(&slot, "main", "GET", &path).status(), 404);
    }

    fn fixture_mp4(timescale: u32, duration: u32) -> Vec<u8> {
        fn box_of(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut v = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
            v.extend(kind);
            v.extend(payload);
            v
        }
        let mut ftyp = Vec::from(*b"isom");
        ftyp.extend(0u32.to_be_bytes());
        ftyp.extend(b"isomiso2mp41");
        let mut mvhd = vec![0; 4];
        mvhd.extend([0u8; 8]);
        mvhd.extend(timescale.to_be_bytes());
        mvhd.extend(duration.to_be_bytes());
        let mut v = box_of(b"ftyp", &ftyp);
        v.extend(box_of(b"moov", &box_of(b"mvhd", &mvhd)));
        v.extend(box_of(b"mdat", b"x"));
        v
    }

    #[test]
    fn video_preview_is_private_and_delivered_as_mp4() {
        let mp4 = fixture_mp4(1000, 5000);
        let digest = nai_avatar_files::video::inspect_mp4(&mp4).unwrap().sha256;
        let slot = PreviewSlot::default();
        slot.install_video("workspace", &digest, mp4.clone())
            .unwrap();
        let path = format!("/workspace/{digest}/{digest}");
        let answer = response(&slot, "main", "GET", &path);
        assert_eq!(answer.status(), 200);
        assert_eq!(answer.headers().get("content-type").unwrap(), "video/mp4");
        assert_eq!(answer.body(), &mp4);
        assert_eq!(response(&slot, "other", "GET", &path).status(), 404);
        assert!(
            slot.install_video("workspace", &"a".repeat(64), mp4)
                .is_err()
        );
        slot.clear().unwrap();
        assert_eq!(response(&slot, "main", "GET", &path).status(), 404);
    }

    #[test]
    fn binary_endpoint_only_serves_exact_staged_identity_to_main_get() {
        let slot = PreviewSlot::default();
        let package = "a".repeat(64);
        let rights = "b".repeat(64);
        slot.install("workspace", &package, &rights, PNG_SIGNATURE.to_vec())
            .unwrap();
        let path = format!("/workspace/{package}/{rights}");
        assert_eq!(response(&slot, "main", "GET", &path).status(), 200);
        assert_eq!(response(&slot, "other", "GET", &path).status(), 404);
        assert_eq!(response(&slot, "main", "POST", &path).status(), 404);
        assert_eq!(
            response(&slot, "main", "GET", &format!("{path}/extra")).status(),
            404
        );
        slot.clear().unwrap();
        assert_eq!(response(&slot, "main", "GET", &path).status(), 404);
    }
}

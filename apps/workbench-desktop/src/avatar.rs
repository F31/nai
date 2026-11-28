use nai_avatar_2d::{AssetBytes, Avatar, LicenseGrants};
use nai_plugin_sdk::plugin::PluginContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

const TOTAL_LIMIT: usize = 16 * 1024 * 1024;
const FILE_LIMIT: usize = 8 * 1024 * 1024;

use nai_avatar_files::SelectedDirectory;

#[cfg(test)]
fn read_selected_file(root: &Path, relative: &str, limit: usize) -> Result<Vec<u8>, String> {
    SelectedDirectory::open(root)?.read(relative, limit)
}

/// Internal native preparation only: keep one selected root handle while
/// reading all files. This does not establish rights or public output authority.
pub(crate) fn admit_selected_directory(
    root: PathBuf,
    selected: &Path,
    acknowledge_rights: bool,
) -> Result<Admission, String> {
    if !acknowledge_rights {
        return Err("AVATAR_RIGHTS_CONFIRMATION_REQUIRED".into());
    }
    let directory = SelectedDirectory::open(selected)?;
    let read_text = |name| {
        String::from_utf8(directory.read(name, 65536)?)
            .map_err(|_| "AVATAR_MANIFEST_REJECTED".to_string())
    };
    let manifest = read_text("manifest.json")?;
    let config = read_text("config.json")?;
    let license = read_text("LICENSE.txt")?;
    let doc: Value = serde_json::from_str(&manifest).map_err(|_| "AVATAR_MANIFEST_REJECTED")?;
    let entries = doc["assets"]
        .as_array()
        .filter(|entries| entries.len() == 3)
        .ok_or("AVATAR_MANIFEST_REJECTED")?;
    let mut assets = Vec::with_capacity(entries.len());
    let mut total = 0usize;
    for entry in entries {
        let id = text(entry, "id")?;
        let path = text(entry, "path")?;
        let bytes = directory.read(&path, FILE_LIMIT.min(TOTAL_LIMIT.saturating_sub(total)))?;
        total = total
            .checked_add(bytes.len())
            .ok_or("AVATAR_SIZE_REJECTED")?;
        assets.push(AssetUpload { id, bytes });
    }
    admit(root, manifest, config, license, assets, true)
}

#[derive(Deserialize)]
pub struct AssetUpload {
    id: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Admission {
    pub avatar_id: String,
    pub version: String,
    pub package_sha256: String,
    pub license_sha256: String,
    pub width: u32,
    pub height: u32,
    pub preview_png: Vec<u8>,
}

/// Control-channel metadata only: image bytes remain in the native package.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionMetadata {
    avatar_id: String,
    version: String,
    package_sha256: String,
    license_sha256: String,
    width: u32,
    height: u32,
}

impl From<Admission> for AdmissionMetadata {
    fn from(value: Admission) -> Self {
        Self {
            avatar_id: value.avatar_id,
            version: value.version,
            package_sha256: value.package_sha256,
            license_sha256: value.license_sha256,
            width: value.width,
            height: value.height,
        }
    }
}

pub(crate) fn admit_native_choice(
    root: PathBuf,
    selected: Option<PathBuf>,
    workspace_id: &str,
    previews: &crate::avatar_preview::PreviewSlot,
) -> Result<Option<AdmissionMetadata>, String> {
    match selected {
        Some(path) => {
            let mut result = admit_selected_directory(root, &path, true)?;
            previews
                .install(
                    workspace_id,
                    &result.package_sha256,
                    &result.license_sha256,
                    std::mem::take(&mut result.preview_png),
                )
                .map_err(str::to_owned)?;
            Ok(Some(result.into()))
        }
        None => Ok(None),
    }
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn text(value: &Value, name: &str) -> Result<String, String> {
    value[name]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 128 && !v.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or_else(|| "AVATAR_MANIFEST_REJECTED".into())
}

fn preview_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| "AVATAR_PREVIEW_FAILED")?;
        writer
            .write_image_data(rgba)
            .map_err(|_| "AVATAR_PREVIEW_FAILED")?;
    }
    Ok(out)
}

fn atomic_install(root: &Path, digest: &str, files: &[(&str, &[u8])]) -> Result<(), String> {
    let parent = root.join("avatar-packages");
    fs::create_dir_all(&parent).map_err(|_| "AVATAR_STORE_FAILED")?;
    let target = parent.join(digest);
    if target.is_dir() {
        let directory = SelectedDirectory::open(&target).map_err(|_| "AVATAR_STORE_INTEGRITY")?;
        for (name, expected) in files {
            let actual = directory
                .read(name, expected.len())
                .map_err(|_| "AVATAR_STORE_INTEGRITY")?;
            if actual != *expected {
                return Err("AVATAR_STORE_INTEGRITY".into());
            }
        }
        return Ok(());
    }
    let temp = parent.join(format!(".{digest}.tmp-{}", std::process::id()));
    fs::create_dir(&temp).map_err(|_| "AVATAR_STORE_FAILED")?;
    for (name, bytes) in files {
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || *name == "."
            || *name == ".."
        {
            let _ = fs::remove_dir_all(&temp);
            return Err("AVATAR_PATH_REJECTED".into());
        }
        let mut file = fs::File::create(temp.join(name)).map_err(|_| "AVATAR_STORE_FAILED")?;
        file.write_all(bytes).map_err(|_| "AVATAR_STORE_FAILED")?;
        file.sync_all().map_err(|_| "AVATAR_STORE_FAILED")?;
    }
    fs::rename(&temp, &target).map_err(|_| "AVATAR_STORE_FAILED")?;
    Ok(())
}

pub fn admit(
    root: PathBuf,
    manifest_text: String,
    config_text: String,
    license_text: String,
    assets: Vec<AssetUpload>,
    acknowledge_rights: bool,
) -> Result<Admission, String> {
    if !acknowledge_rights {
        return Err("AVATAR_RIGHTS_CONFIRMATION_REQUIRED".into());
    }
    if manifest_text.len() > 65536
        || config_text.len() > 65536
        || license_text.trim().is_empty()
        || license_text.len() > 65536
        || assets.len() != 3
    {
        return Err("AVATAR_SIZE_REJECTED".into());
    }
    if assets.iter().any(|a| {
        a.id.is_empty() || a.id.len() > 128 || a.bytes.is_empty() || a.bytes.len() > FILE_LIMIT
    }) || assets.iter().map(|a| a.bytes.len()).sum::<usize>() > TOTAL_LIMIT
    {
        return Err("AVATAR_SIZE_REJECTED".into());
    }
    let manifest: Value =
        serde_json::from_str(&manifest_text).map_err(|_| "AVATAR_MANIFEST_REJECTED")?;
    let config: Value = serde_json::from_str(&config_text).map_err(|_| "AVATAR_CONFIG_REJECTED")?;
    let license_ref = text(&manifest, "license_ref")?;
    let mut grants = LicenseGrants::from([license_ref]);
    for asset in manifest["assets"]
        .as_array()
        .ok_or("AVATAR_MANIFEST_REJECTED")?
    {
        grants.insert(text(asset, "license_ref")?);
    }
    let borrowed: Vec<_> = assets
        .iter()
        .map(|a| AssetBytes {
            id: &a.id,
            bytes: &a.bytes,
        })
        .collect();
    let context = PluginContext::new(
        "nai-avatar-2d",
        "avatar-import",
        1,
        10_000,
        serde_json::json!({}),
        [],
        Instant::now() + Duration::from_secs(10),
        Arc::new(AtomicBool::new(false)),
        TOTAL_LIMIT as u64,
    );
    let avatar = Avatar::admit(&context, &manifest, &config, &borrowed, &grants)
        .map_err(|_| "AVATAR_ADMISSION_REJECTED")?;
    let frame = avatar
        .standby(&context, &grants)
        .map_err(|_| "AVATAR_ADMISSION_REJECTED")?;
    let mut package = Sha256::new();
    package.update(manifest_text.as_bytes());
    package.update(config_text.as_bytes());
    package.update(license_text.as_bytes());
    let mut installed = vec![
        ("manifest.json", manifest_text.as_bytes()),
        ("config.json", config_text.as_bytes()),
        ("LICENSE.txt", license_text.as_bytes()),
    ];
    let asset_ids = serde_json::to_vec(&assets.iter().map(|a| &a.id).collect::<Vec<_>>())
        .map_err(|_| "AVATAR_STORE_FAILED")?;
    installed.push(("asset-ids.json", &asset_ids));
    for asset in &assets {
        package.update(asset.id.as_bytes());
        package.update(&asset.bytes);
    }
    let package_sha256 = package
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    for (index, asset) in assets.iter().enumerate() {
        installed.push((
            match index {
                0 => "asset-0.rgba",
                1 => "asset-1.rgba",
                _ => "asset-2.rgba",
            },
            &asset.bytes,
        ));
    }
    let preview_png = preview_png(frame.width, frame.height, &frame.rgba)?;
    installed.push(("preview.png", &preview_png));
    atomic_install(&root, &package_sha256, &installed)?;
    Ok(Admission {
        avatar_id: text(&manifest, "avatar_id")?,
        version: text(&manifest, "version")?,
        package_sha256,
        license_sha256: hex(license_text.as_bytes()),
        width: frame.width,
        height: frame.height,
        preview_png,
    })
}

struct InstalledPackage {
    manifest: String,
    config: String,
    license: String,
    assets: Vec<AssetUpload>,
}

fn read_installed_package(root: &Path, digest: &str) -> Result<InstalledPackage, String> {
    let p = nai_avatar_files::read_installed(root, digest)?;
    Ok(InstalledPackage {
        manifest: p.manifest,
        config: p.config,
        license: p.license,
        assets: p
            .assets
            .into_iter()
            .map(|a| AssetUpload {
                id: a.id,
                bytes: a.bytes,
            })
            .collect(),
    })
}

#[cfg(test)]
fn preview(root: &Path, digest: &str) -> Result<Vec<u8>, String> {
    let package = read_installed_package(root, digest)?;
    let result = admit(
        root.to_path_buf(),
        package.manifest,
        package.config,
        package.license,
        package.assets,
        true,
    )?;
    if result.package_sha256 != digest {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    Ok(result.preview_png)
}

pub(crate) fn stage_existing_preview(
    root: &Path,
    workspace: &str,
    digest: &str,
    rights: &str,
    previews: &crate::avatar_preview::PreviewSlot,
) -> Result<AdmissionMetadata, String> {
    previews.clear().map_err(str::to_owned)?;
    let package = read_installed_package(root, digest)?;
    if hex(package.license.as_bytes()) != rights {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    let mut result = admit(
        root.to_path_buf(),
        package.manifest,
        package.config,
        package.license,
        package.assets,
        true,
    )?;
    if result.package_sha256 != digest || result.license_sha256 != rights {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    previews
        .install(
            workspace,
            digest,
            rights,
            std::mem::take(&mut result.preview_png),
        )
        .map_err(str::to_owned)?;
    Ok(result.into())
}

/// Internal copy to a caller-owned isolated backup root. This verifies content
/// identity but does not assert rights, migrate a database, or restore live data.
#[allow(dead_code)]
fn copy_package_to_backup(
    source: &Path,
    destination: &Path,
    digest: &str,
    expected_rights: &str,
) -> Result<AdmissionMetadata, String> {
    let package = read_installed_package(source, digest)?;
    if hex(package.license.as_bytes()) != expected_rights {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    let result = admit(
        destination.to_path_buf(),
        package.manifest,
        package.config,
        package.license,
        package.assets,
        true,
    )?;
    if result.package_sha256 != digest || result.license_sha256 != expected_rights {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    let copied = read_installed_package(destination, digest)?;
    if hex(copied.license.as_bytes()) != expected_rights {
        return Err("AVATAR_STORE_INTEGRITY".into());
    }
    Ok(result.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (String, String, Vec<AssetUpload>) {
        let data = [
            vec![10, 20, 30, 255, 0, 0, 255, 255],
            vec![0, 0, 0, 255],
            vec![255, 0, 0, 255],
        ];
        let manifest = json!({"schema_version":1,"avatar_id":"desktop-test","version":"1","kind":"2d","license_ref":"owned-test","assets":[
          {"id":"base","path":"base.rgba","sha256":hex(&data[0]),"license_ref":"owned-test"},{"id":"closed","path":"closed.rgba","sha256":hex(&data[1]),"license_ref":"owned-test"},{"id":"open","path":"open.rgba","sha256":hex(&data[2]),"license_ref":"owned-test"}],
          "model_binding":"software-rgba8-v1","resource_profile_ref":"test","supported_actions":["idle","speak"],"supported_emotions":["neutral"],"voice_binding":"any","native_width":2,"native_height":1,"output_width":2,"output_height":1,"fps_num":25,"fps_den":1,"base_duration_samples":"0","switch_points":[],"personalization_refs":{}});
        let config = json!({"schema_version":1,"pixel_format":"rgba8","sample_rate":8000,"max_buffer_samples":8000,"noise_gate":0.02,"full_open_rms":0.5,"attack_ms":10,"release_ms":100,"layers":[{"asset_id":"base","role":"base","width":2,"height":1,"x":0,"y":0},{"asset_id":"closed","role":"mouth_closed","width":1,"height":1,"x":1,"y":0},{"asset_id":"open","role":"mouth_open","width":1,"height":1,"x":1,"y":0}]});
        (
            manifest.to_string(),
            config.to_string(),
            ["base", "closed", "open"]
                .into_iter()
                .zip(data)
                .map(|(id, bytes)| AssetUpload {
                    id: id.into(),
                    bytes,
                })
                .collect(),
        )
    }
    #[test]
    fn selected_directory_admission_reads_manifest_assets_and_rejects_corruption() {
        let root =
            std::env::temp_dir().join(format!("nai-avatar-directory-{}", std::process::id()));
        let selected = root.join("selected");
        let store = root.join("store");
        fs::create_dir_all(&selected).unwrap();
        let (manifest, config, assets) = fixture();
        fs::write(selected.join("manifest.json"), &manifest).unwrap();
        fs::write(selected.join("config.json"), &config).unwrap();
        fs::write(selected.join("LICENSE.txt"), "test license").unwrap();
        for asset in &assets {
            fs::write(selected.join(format!("{}.rgba", asset.id)), &asset.bytes).unwrap();
        }
        assert_eq!(
            admit_selected_directory(store.clone(), &selected, false).unwrap_err(),
            "AVATAR_RIGHTS_CONFIRMATION_REQUIRED"
        );
        assert!(!store.exists());
        let previews = crate::avatar_preview::PreviewSlot::default();
        let result = admit_native_choice(
            store.clone(),
            Some(selected.clone()),
            "workspace",
            &previews,
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.avatar_id, "desktop-test");
        let bytes = previews
            .read("workspace", &result.package_sha256, &result.license_sha256)
            .unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(
            previews
                .read("other", &result.package_sha256, &result.license_sha256)
                .is_err()
        );
        let metadata = serde_json::to_value(result).unwrap();
        assert!(metadata.get("previewPng").is_none());
        assert!(metadata.get("path").is_none());
        fs::write(selected.join("base.rgba"), b"altered").unwrap();
        let other = root.join("rejected");
        assert!(admit_selected_directory(other.clone(), &selected, true).is_err());
        assert!(!other.exists());
        let mut malicious: Value = serde_json::from_str(&manifest).unwrap();
        malicious["assets"][0]["path"] = Value::String("../outside.rgba".into());
        fs::write(selected.join("manifest.json"), malicious.to_string()).unwrap();
        fs::write(root.join("outside.rgba"), &assets[0].bytes).unwrap();
        assert!(admit_selected_directory(other.clone(), &selected, true).is_err());
        assert!(!other.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelled_native_choice_never_creates_package_store() {
        let root = std::env::temp_dir().join(format!("nai-avatar-cancel-{}", std::process::id()));
        let previews = crate::avatar_preview::PreviewSlot::default();
        assert!(
            admit_native_choice(root.clone(), None, "workspace", &previews)
                .unwrap()
                .is_none()
        );
        assert!(!root.exists());
    }

    #[test]
    #[cfg(windows)]
    fn selected_reader_rejects_junction_to_outside() {
        let root = std::env::temp_dir().join(format!("nai-avatar-junction-{}", std::process::id()));
        let selected = root.join("selected");
        let outside = root.join("outside");
        fs::create_dir_all(&selected).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("asset.rgba"), b"secret").unwrap();
        let link = selected.join("link");
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(read_selected_file(&selected, "link/asset.rgba", 100).is_err());
        assert!(read_selected_file(&link, "asset.rgba", 100).is_err());
        fs::remove_dir(&link).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn selected_reader_never_follows_swapped_junction() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let root = std::env::temp_dir().join(format!("nai-avatar-race-{}", std::process::id()));
        let selected = root.join("selected");
        let outside = root.join("outside");
        fs::create_dir_all(selected.join("assetdir")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(selected.join("assetdir/asset.rgba"), b"safe").unwrap();
        fs::write(outside.join("asset.rgba"), b"secret").unwrap();
        let junction = selected.join("junction");
        assert!(
            std::process::Command::new("cmd.exe")
                .args(["/C", "mklink", "/J"])
                .arg(&junction)
                .arg(&outside)
                .status()
                .unwrap()
                .success()
        );
        let done = Arc::new(AtomicBool::new(false));
        let worker_done = done.clone();
        let worker_selected = selected.clone();
        let worker = std::thread::spawn(move || {
            let active = worker_selected.join("assetdir");
            let safe = worker_selected.join("safe");
            let junction = worker_selected.join("junction");
            let mut swaps = 0;
            while !worker_done.load(Ordering::Relaxed) {
                if fs::rename(&active, &safe).is_err() {
                    continue;
                }
                if fs::rename(&junction, &active).is_ok() {
                    swaps += 1;
                    while fs::rename(&active, &junction).is_err() {
                        std::thread::yield_now();
                    }
                }
                while fs::rename(&safe, &active).is_err() {
                    std::thread::yield_now();
                }
            }
            swaps
        });
        for _ in 0..1000 {
            if let Ok(bytes) = read_selected_file(&selected, "assetdir/asset.rgba", 100) {
                assert_eq!(bytes, b"safe");
            }
        }
        done.store(true, Ordering::Relaxed);
        assert!(
            worker.join().unwrap() > 0,
            "replacement must actually occur"
        );
        fs::remove_dir(junction).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn admitted_package_is_immutable_and_previewable() {
        let root = std::env::temp_dir().join(format!("nai-avatar-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (manifest, config, assets) = fixture();
        let a = admit(
            root.clone(),
            manifest.clone(),
            config.clone(),
            "test license".into(),
            assets,
            true,
        )
        .unwrap();
        assert_eq!((a.width, a.height), (2, 1));
        assert!(a.preview_png.starts_with(b"\x89PNG"));
        assert_eq!(preview(&root, &a.package_sha256).unwrap(), a.preview_png);
        let response = serde_json::to_value(&a).unwrap();
        assert_eq!(response["avatarId"], "desktop-test");
        assert_eq!(response["packageSha256"], a.package_sha256);
        assert!(response["previewPng"].is_array());
        let (_, _, assets) = fixture();
        let again = admit(
            root.clone(),
            manifest,
            config,
            "test license".into(),
            assets,
            true,
        )
        .unwrap();
        assert_eq!(again.package_sha256, a.package_sha256);
        fs::write(
            root.join("avatar-packages")
                .join(&a.package_sha256)
                .join("preview.png"),
            b"tampered",
        )
        .unwrap();
        assert_eq!(
            preview(&root, &a.package_sha256).unwrap_err(),
            "AVATAR_STORE_INTEGRITY"
        );
        let (manifest, config, assets) = fixture();
        assert_eq!(
            admit(
                root.clone(),
                manifest,
                config,
                "test license".into(),
                assets,
                true
            )
            .unwrap_err(),
            "AVATAR_STORE_INTEGRITY"
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn corrupt_asset_and_invalid_identity_fail_closed() {
        let root = std::env::temp_dir().join(format!("nai-avatar-bad-{}", std::process::id()));
        let (manifest, config, mut assets) = fixture();
        assets[0].bytes[0] ^= 1;
        assert_eq!(
            admit(
                root.clone(),
                manifest,
                config,
                "test license".into(),
                assets,
                true
            )
            .unwrap_err(),
            "AVATAR_ADMISSION_REJECTED"
        );
        assert_eq!(
            preview(&root, "../escape").unwrap_err(),
            "AVATAR_IDENTITY_REJECTED"
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn unconfirmed_rights_never_install_a_package() {
        let root = std::env::temp_dir().join(format!("nai-avatar-rights-{}", std::process::id()));
        let (manifest, config, assets) = fixture();
        assert_eq!(
            admit(
                root.clone(),
                manifest,
                config,
                "test license".into(),
                assets,
                false
            )
            .unwrap_err(),
            "AVATAR_RIGHTS_CONFIRMATION_REQUIRED"
        );
        assert!(!root.join("avatar-packages").exists());
    }
    #[test]
    fn empty_license_never_installs_a_package() {
        let root =
            std::env::temp_dir().join(format!("nai-avatar-empty-license-{}", std::process::id()));
        let (manifest, config, assets) = fixture();
        assert_eq!(
            admit(root.clone(), manifest, config, " \n\t".into(), assets, true).unwrap_err(),
            "AVATAR_SIZE_REJECTED"
        );
        assert!(!root.join("avatar-packages").exists());
    }
    #[test]
    fn oversized_asset_is_rejected_before_install() {
        let root = std::env::temp_dir().join(format!("nai-avatar-oversize-{}", std::process::id()));
        let (manifest, config, mut assets) = fixture();
        assets[0].bytes = vec![0; FILE_LIMIT + 1];
        assert_eq!(
            admit(
                root.clone(),
                manifest,
                config,
                "test license".into(),
                assets,
                true
            )
            .unwrap_err(),
            "AVATAR_SIZE_REJECTED"
        );
        assert!(!root.join("avatar-packages").exists());
    }
    #[test]
    fn selected_file_reader_rejects_traversal_and_oversize() {
        let root = std::env::temp_dir().join(format!("nai-avatar-read-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("asset.rgba"), [1, 2, 3]).unwrap();
        assert_eq!(
            read_selected_file(&root, "asset.rgba", 3).unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(
            read_selected_file(&root, "asset.rgba", 2).unwrap_err(),
            "AVATAR_SIZE_REJECTED"
        );
        assert_eq!(
            read_selected_file(&root.join("asset.rgba"), "asset.rgba", 3).unwrap_err(),
            "AVATAR_PATH_REJECTED"
        );
        for name in ["../asset.rgba", "/asset.rgba", "a\\b", "a/../asset.rgba"] {
            assert_eq!(
                read_selected_file(&root, name, 3).unwrap_err(),
                "AVATAR_PATH_REJECTED"
            );
        }
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn preview_rejects_altered_asset_or_license_without_creating_another_package() {
        for name in ["asset-0.rgba", "LICENSE.txt"] {
            let root = std::env::temp_dir().join(format!(
                "nai-avatar-altered-{}-{}",
                std::process::id(),
                name
            ));
            let (manifest, config, assets) = fixture();
            let admitted = admit(
                root.clone(),
                manifest,
                config,
                "test license".into(),
                assets,
                true,
            )
            .unwrap();
            let parent = root.join("avatar-packages");
            fs::write(parent.join(&admitted.package_sha256).join(name), b"altered").unwrap();
            assert_eq!(
                preview(&root, &admitted.package_sha256).unwrap_err(),
                "AVATAR_STORE_INTEGRITY"
            );
            assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn isolated_package_backup_rechecks_identity_and_never_overwrites_source() {
        let root = std::env::temp_dir().join(format!("nai-avatar-backup-{}", std::process::id()));
        let source = root.join("source");
        let backup = root.join("backup");
        let rejected = root.join("rejected");
        let (manifest, config, assets) = fixture();
        let admitted = admit(
            source.clone(),
            manifest,
            config,
            "test license".into(),
            assets,
            true,
        )
        .unwrap();
        nai_avatar_files::verify_installed(
            &source,
            &admitted.package_sha256,
            &admitted.license_sha256,
        )
        .unwrap();
        assert!(
            nai_avatar_files::verify_installed(&source, &admitted.package_sha256, &"a".repeat(64))
                .is_err()
        );
        let source_preview = preview(&source, &admitted.package_sha256).unwrap();
        let shared_backup = root.join("shared-backup");
        fs::create_dir(&shared_backup).unwrap();
        nai_avatar_files::copy_verified(
            &source,
            &shared_backup,
            &admitted.package_sha256,
            &admitted.license_sha256,
            || Ok(()),
        )
        .unwrap();
        assert_eq!(
            preview(&shared_backup, &admitted.package_sha256).unwrap(),
            source_preview
        );
        assert_eq!(
            copy_package_to_backup(
                &source,
                &rejected,
                &admitted.package_sha256,
                &"a".repeat(64)
            )
            .unwrap_err(),
            "AVATAR_STORE_INTEGRITY"
        );
        assert!(!rejected.exists());
        let copied = copy_package_to_backup(
            &source,
            &backup,
            &admitted.package_sha256,
            &admitted.license_sha256,
        )
        .unwrap();
        assert_eq!(copied.package_sha256, admitted.package_sha256);
        assert_eq!(copied.license_sha256, admitted.license_sha256);
        assert_eq!(
            preview(&backup, &copied.package_sha256).unwrap(),
            source_preview
        );
        fs::write(
            source
                .join("avatar-packages")
                .join(&admitted.package_sha256)
                .join("LICENSE.txt"),
            b"altered",
        )
        .unwrap();
        assert_eq!(
            copy_package_to_backup(
                &source,
                &rejected,
                &admitted.package_sha256,
                &admitted.license_sha256
            )
            .unwrap_err(),
            "AVATAR_STORE_INTEGRITY"
        );
        assert!(!rejected.exists());
        assert_eq!(
            preview(&backup, &copied.package_sha256).unwrap(),
            source_preview
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staged_existing_preview_rechecks_license_and_clears_old_slot_on_failure() {
        let root = std::env::temp_dir().join(format!("nai-avatar-stage-{}", std::process::id()));
        let (manifest, config, assets) = fixture();
        let admitted = admit(
            root.clone(),
            manifest,
            config,
            "test license".into(),
            assets,
            true,
        )
        .unwrap();
        let previews = crate::avatar_preview::PreviewSlot::default();
        let meta = stage_existing_preview(
            &root,
            "workspace",
            &admitted.package_sha256,
            &admitted.license_sha256,
            &previews,
        )
        .unwrap();
        assert!(
            previews
                .read("workspace", &meta.package_sha256, &meta.license_sha256)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
        assert!(
            stage_existing_preview(
                &root,
                "workspace",
                &admitted.package_sha256,
                &"a".repeat(64),
                &previews
            )
            .is_err()
        );
        assert!(
            previews
                .read("workspace", &meta.package_sha256, &meta.license_sha256)
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn existing_package_junction_is_not_accepted_for_preview_or_reinstall() {
        let root =
            std::env::temp_dir().join(format!("nai-avatar-store-link-{}", std::process::id()));
        let outside = root.join("outside");
        let store = root.join("store");
        let (manifest, config, assets) = fixture();
        let admission = admit(
            outside.clone(),
            manifest,
            config,
            "test license".into(),
            assets,
            true,
        )
        .unwrap();
        let real_package = outside
            .join("avatar-packages")
            .join(&admission.package_sha256);
        let parent = store.join("avatar-packages");
        fs::create_dir_all(&parent).unwrap();
        let junction = parent.join(&admission.package_sha256);
        assert!(
            std::process::Command::new("cmd.exe")
                .args(["/C", "mklink", "/J"])
                .arg(&junction)
                .arg(&real_package)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            preview(&store, &admission.package_sha256).unwrap_err(),
            "AVATAR_STORE_INTEGRITY"
        );
        assert_eq!(
            atomic_install(
                &store,
                &admission.package_sha256,
                &[("LICENSE.txt", b"test license")]
            )
            .unwrap_err(),
            "AVATAR_STORE_INTEGRITY"
        );
        fs::remove_dir(junction).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

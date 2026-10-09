#![allow(dead_code)]
//! Fixtures for the engine integration tests: a real provisioned workspace, an
//! owner-only token file and real 2D avatar assets. No mocks.
use nai_storage::{ConfigKind, DataPaths, Operation, Store};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub const WORKSPACE: &str = "ws-primary";
pub const ACTOR: &str = "root-owner";
pub const PROFILE: &str = "default";
pub const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Directory holding freshly built workspace binaries (`target/<profile>`).
pub fn target_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    exe.parent()
        .and_then(Path::parent)
        .expect("target profile directory")
        .to_path_buf()
}

/// Resolves a sibling binary. A missing binary fails loudly, never skips.
pub fn binary(name: &str) -> PathBuf {
    let path = target_dir().join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.is_file(),
        "build the workspace first; missing {}",
        path.display()
    );
    path
}

pub fn store_operation() -> Operation {
    Operation::new(Duration::from_secs(5)).unwrap()
}

pub fn open_store(root: &Path) -> Store {
    Store::open(DataPaths::at(root).unwrap(), &store_operation()).unwrap()
}

/// Creates the workspace, grants the runtime permissions and stores the profile,
/// then releases the single-owner lock for the engine process.
pub fn provision(root: &Path, workspace: &str, actor: &str, profile: &str, config: &Value) {
    let operation = store_operation();
    let mut store = open_store(root);
    store
        .create_workspace(&operation, workspace, workspace)
        .unwrap();
    for (permission, resource) in [
        ("config.read", profile),
        ("config.write", profile),
        ("approval.read", "*"),
        ("approval.write", "*"),
        ("session.manage", "*"),
    ] {
        store
            .set_permission(&operation, workspace, actor, permission, resource, true)
            .unwrap();
    }
    store
        .save_config(
            &operation,
            (workspace, actor),
            ConfigKind::Runtime,
            profile,
            config,
            0,
        )
        .unwrap();
}

/// Owner-only token file (0600 on Unix).
pub fn write_token(path: &Path) {
    fs::write(path, TOKEN).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// A schema-valid runtime profile with the short (600ms) media-lease window.
pub fn runtime_profile(workspace: &str) -> Value {
    json!({
        "schema_version": 1,
        "edition": "standard",
        "deployment": "local",
        "workspace_id": workspace,
        "runtime": {
            "max_active_sessions": 4,
            "plugin_catalog": "catalog-local",
            "control_listener": "named_pipe",
            "http_enabled": false
        },
        "bindings": {},
        "session_defaults": {
            "mode": "rehearsal",
            "allow_auto_without_platform_grant": false,
            "viewer_memory_enabled": false
        },
        "scheduler": {
            "max_pending_utterances": 8,
            "max_pending_speech_ms": 60000,
            "qa_ttl_ms": 60000,
            "welcome_ttl_ms": 60000,
            "thanks_ttl_ms": 60000
        },
        "media": {
            "lease_renew_ms": 100,
            "lease_expire_ms": 600,
            "output_sample_rate": 48000,
            "model_audio_format": "from_model_manifest",
            "startup_buffer_ms": 100,
            "max_buffer_ms": 5000
        },
        "knowledge": {
            "mode": "local",
            "fts_enabled": true,
            "semantic_enabled": false,
            "remote_source_refs": [],
            "index_during_live": "disabled"
        },
        "budgets": {
            "soft_threshold_ratio": 0.8,
            "hard_stop_new_paid_requests": true,
            "currency": "CNY",
            "per_session_limit_minor": "1000"
        },
        "cache": {"tts_max_bytes": 0, "invalidate_on_fact_change": true},
        "cloud": {"control_enabled": false, "relay_enabled": false, "render_enabled": false}
    })
}

pub struct AvatarFiles {
    pub manifest: PathBuf,
    pub config: PathBuf,
    pub assets: Vec<(String, PathBuf)>,
}

const BASE: [u8; 8] = [10, 20, 30, 255, 0, 0, 255, 255];
const CLOSED: [u8; 4] = [0, 0, 0, 255];
const OPEN: [u8; 4] = [255, 0, 0, 255];

/// Real RGBA8 assets with content hashes, a schema-valid manifest and a valid
/// layer config. The engine reads the manifest for its frame geometry.
pub fn write_avatar(dir: &Path) -> AvatarFiles {
    let (base_hash, closed_hash, open_hash) = (digest(&BASE), digest(&CLOSED), digest(&OPEN));
    let manifest = json!({
        "schema_version": 1,
        "avatar_id": "test-only-2d",
        "version": "1",
        "kind": "2d",
        "license_ref": "test-license",
        "assets": [
            {"id": "base", "path": "base.rgba", "sha256": base_hash, "license_ref": "test-license"},
            {"id": "closed", "path": "closed.rgba", "sha256": closed_hash, "license_ref": "test-license"},
            {"id": "open", "path": "open.rgba", "sha256": open_hash, "license_ref": "test-license"}
        ],
        "model_binding": "software-rgba8-v1",
        "resource_profile_ref": "test-software",
        "supported_actions": ["idle", "speak"],
        "supported_emotions": ["neutral"],
        "voice_binding": "test-voice",
        "native_width": 2,
        "native_height": 1,
        "output_width": 2,
        "output_height": 1,
        "fps_num": 25,
        "fps_den": 1,
        "base_duration_samples": "0",
        "switch_points": [],
        "personalization_refs": {}
    });
    let config = json!({
        "schema_version": 1,
        "pixel_format": "rgba8",
        "sample_rate": 8000,
        "max_buffer_samples": 8000,
        "noise_gate": 0.02,
        "full_open_rms": 0.5,
        "attack_ms": 10,
        "release_ms": 100,
        "layers": [
            {"asset_id": "base", "role": "base", "width": 2, "height": 1, "x": 0, "y": 0},
            {"asset_id": "closed", "role": "mouth_closed", "width": 1, "height": 1, "x": 1, "y": 0},
            {"asset_id": "open", "role": "mouth_open", "width": 1, "height": 1, "x": 1, "y": 0}
        ]
    });
    let manifest_path = dir.join("manifest.json");
    let config_path = dir.join("config.json");
    fs::write(&manifest_path, manifest.to_string()).unwrap();
    fs::write(&config_path, config.to_string()).unwrap();
    let mut assets = Vec::new();
    for (id, name, bytes) in [
        ("base", "base.rgba", BASE.as_slice()),
        ("closed", "closed.rgba", CLOSED.as_slice()),
        ("open", "open.rgba", OPEN.as_slice()),
    ] {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        assets.push((id.to_string(), path));
    }
    AvatarFiles {
        manifest: manifest_path,
        config: config_path,
        assets,
    }
}

/// Raw little-endian f32 PCM (no JSON, no base64).
pub fn write_pcm(dir: &Path, samples: usize) -> PathBuf {
    let path = dir.join("pcm.bin");
    let mut bytes = Vec::new();
    for _ in 0..samples {
        bytes.extend_from_slice(&0.5f32.to_le_bytes());
    }
    fs::write(&path, &bytes).unwrap();
    path
}

fn digest(bytes: &[u8]) -> String {
    nai_media_core::frame_slots::shared_ram::digest(bytes)
}

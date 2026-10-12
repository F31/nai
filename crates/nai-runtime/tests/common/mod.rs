#![allow(dead_code)]
//! Shared fixtures for the runtime integration tests. Provisioning uses the
//! real single-owner store; it is trusted owner administration, not a model path.
use nai_storage::{ConfigKind, DataPaths, Operation, Store};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const WORKSPACE: &str = "ws-primary";
pub const ACTOR: &str = "root-owner";
pub const PROFILE: &str = "default";

pub fn utc_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Directory holding freshly built workspace binaries (`target/<profile>`).
pub fn target_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    exe.parent()
        .and_then(Path::parent)
        .expect("target profile directory")
        .to_path_buf()
}

/// Resolves a sibling binary. Missing binaries fail loudly instead of skipping.
pub fn binary(name: &str) -> PathBuf {
    let path = target_dir().join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.is_file(),
        "build the workspace first; missing {}",
        path.display()
    );
    path
}

/// A schema-valid runtime profile with the short (600ms) media lease window.
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

/// The budget for the fixture's **bring-up**: opening the store on a fresh data
/// root runs the whole schema migration chain, and the workspace/grants/profile
/// writes that follow belong to the same one-shot setup.
///
/// `nai_runtime` re-exports one definition of this budget (`BRINGUP_OPERATION`)
/// and spells out why it is not the per-call budget. The fixture used to hold
/// its own 5s copy — a second, drifting definition — and that copy described the
/// wrong work: under the default parallel test run several data roots are
/// created at once, so a migration that loses the disk race surfaced as a
/// random `DeadlineExceeded` in the fixture rather than a statement about the
/// system under test. Reusing the constant is the "one budget, one definition"
/// rule (ADR 0027) applied to the fixture.
fn bringup() -> Operation {
    Operation::new(nai_runtime::BRINGUP_OPERATION).unwrap()
}

/// The budget for a **per-call** store access made from a test: reading rows the
/// fixture has already set up. Bring-up work MUST NOT use this one -- `open_store`
/// and `provision` take `bringup()`.
fn operation() -> Operation {
    Operation::new(Duration::from_secs(5)).unwrap()
}

pub fn open_store(root: &Path) -> Store {
    Store::open(DataPaths::at(root).unwrap(), &bringup()).unwrap()
}

/// Creates a workspace, grants the runtime permissions and stores the profile.
/// Drops the store so the single owner lock is released for the composition.
pub fn provision(root: &Path, workspace: &str, actor: &str, profile: &str, config: &Value) {
    let operation = bringup();
    let mut store = open_store(root);
    store
        .create_workspace(&operation, workspace, workspace)
        .unwrap();
    grant(&mut store, &operation, workspace, actor, profile);
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

pub fn grant(
    store: &mut Store,
    operation: &Operation,
    workspace: &str,
    actor: &str,
    profile: &str,
) {
    for (permission, resource) in [
        ("config.read", profile),
        ("config.write", profile),
        ("approval.read", "*"),
        ("approval.write", "*"),
        ("session.manage", "*"),
    ] {
        store
            .set_permission(operation, workspace, actor, permission, resource, true)
            .unwrap();
    }
}

pub fn store_operation() -> Operation {
    operation()
}

/// Recomputes the whole decision-log chain for the composition's workspace.
pub fn verify_chain(composition: &nai_runtime::Composition) {
    let operation = operation();
    let store = composition.store().lock().unwrap();
    store
        .verify_decision_chain(&operation, composition.workspace())
        .unwrap();
}

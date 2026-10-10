//! Offline install and deployment migration (R11).
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use nai_upgrade_core::{
    Announcement, Deployment, DeploymentState, DeterministicVerifier, ImportedState,
    MigrationFault, MigrationRequest, OfflineArtifact, OutputSwitch, PermissionGrant,
    PermissionMap, RehearsalMigration, RevocationEntry, RevocationList, UpgradePackage,
    VerifyLimits, install_offline, missing_artifacts, plan_migration, verify_migration,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

const CONTENT: &[(&str, &[u8])] = &[("bin/app", b"binary-payload"), ("data/model", b"weights")];

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn op() -> Operation {
    Operation::new(Duration::from_secs(30)).unwrap()
}

fn open_revocations(now_ms: i64) -> RevocationList {
    RevocationList::new(now_ms, now_ms + 60_000, []).unwrap()
}

/// The same schema-valid manifest builder the online package tests use, so the
/// offline path is proven against the *same* declaration shape.
fn build_manifest(files: &[(&str, &[u8])], mutate: impl FnOnce(&mut Value)) -> Vec<u8> {
    let declarations: Vec<Value> = files
        .iter()
        .map(|(path, content)| {
            json!({ "path": path, "sha256": sha256_hex(content), "bytes": content.len().to_string() })
        })
        .collect();
    let mut manifest = json!({
        "manifest_schema": 1,
        "package_id": "plugin.demo",
        "package_version": "1.2.3",
        "kind": "plugin",
        "kernel_range": ">=1.0.0, <2.0.0",
        "requires": [{ "ref": "cap.one", "range": ">=1.0.0, <2.0.0" }],
        "hosts": ["inproc"],
        "os": ["linux"],
        "architectures": ["x86_64"],
        "runtime": [{ "id": "rt-cpu", "accel": "cpu" }],
        "files": declarations,
        "install_bytes": "1024",
        "migrated_bytes": "2048",
        "signer_ref": "signer.demo",
        "license_ref": "license.demo",
        "migration_ref": "migration.forward",
        "rollback_range": ">=1.0.0, <1.2.0",
        "cost": { "currency": "CNY", "cloud": "10", "download": null, "storage": "5", "runtime_delta": null }
    });
    mutate(&mut manifest);
    serde_json::to_vec(&manifest).unwrap()
}

fn local<'a>(content: &'a [(&'a str, &'a [u8])]) -> Vec<OfflineArtifact<'a>> {
    content
        .iter()
        .map(|(path, bytes)| OfflineArtifact {
            path,
            content: bytes,
        })
        .collect()
}

fn install(
    manifest: &[u8],
    artifacts: &[OfflineArtifact<'_>],
    signature: &[u8],
    revocations: &RevocationList,
) -> nai_upgrade_core::Result<nai_upgrade_core::OfflineInstall> {
    install_offline(
        manifest,
        artifacts,
        signature,
        &DeterministicVerifier,
        revocations,
        1_000,
        VerifyLimits::default(),
        deadline(),
        &AtomicBool::new(false),
    )
}

#[test]
fn a_complete_local_set_installs_through_the_same_verification() {
    let manifest = build_manifest(CONTENT, |_| {});
    let signature = DeterministicVerifier::sign(&manifest);
    let installed = install(
        &manifest,
        &local(CONTENT),
        &signature,
        &open_revocations(1_000),
    )
    .unwrap();
    assert_eq!(installed.verified.package.package_id, "plugin.demo");
    assert_eq!(installed.artifacts, 2);
}

#[test]
fn a_missing_local_artifact_is_refused_and_named() {
    let manifest = build_manifest(CONTENT, |_| {});
    let signature = DeterministicVerifier::sign(&manifest);
    let partial: &[(&str, &[u8])] = &[("bin/app", b"binary-payload")];

    let refusal = install(
        &manifest,
        &local(partial),
        &signature,
        &open_revocations(1_000),
    )
    .unwrap_err();
    // distinct from the online path's "missing package content": offline the
    // bytes cannot arrive later, so the verdict is "not ready", not "incomplete"
    assert_eq!(refusal.code, ErrorCode::CapabilityNotReady);
    assert_eq!(refusal.reason, "offline install lacks a local artifact");

    let package = UpgradePackage::parse(&manifest).unwrap();
    assert_eq!(
        missing_artifacts(&package, &local(partial)),
        ["data/model".to_owned()]
    );
    assert_eq!(
        missing_artifacts(&package, &local(&[])),
        ["bin/app".to_owned(), "data/model".to_owned()]
    );
    assert!(missing_artifacts(&package, &local(CONTENT)).is_empty());
}

#[test]
fn offline_install_enforces_signature_and_revocation() {
    let manifest = build_manifest(CONTENT, |_| {});
    let signature = DeterministicVerifier::sign(&manifest);

    let tampered: &[(&str, &[u8])] = &[("bin/app", b"binary-payloaX"), ("data/model", b"weights")];
    assert_eq!(
        install(
            &manifest,
            &local(tampered),
            &signature,
            &open_revocations(1_000)
        )
        .unwrap_err()
        .code,
        ErrorCode::InvalidArgument
    );

    let wrong_signature = DeterministicVerifier::sign(b"other");
    assert_eq!(
        install(
            &manifest,
            &local(CONTENT),
            &wrong_signature,
            &open_revocations(1_000)
        )
        .unwrap_err()
        .code,
        ErrorCode::PermissionDenied
    );

    let revoked = RevocationList::new(
        1_000,
        60_000,
        [RevocationEntry {
            subject_ref: "plugin.demo".into(),
            revoked_at_ms: 2_000,
        }],
    )
    .unwrap();
    assert_eq!(
        install(&manifest, &local(CONTENT), &signature, &revoked)
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );

    // an extra artifact that the manifest never declared is not silently ignored
    let extra: &[(&str, &[u8])] = &[
        ("bin/app", b"binary-payload"),
        ("data/model", b"weights"),
        ("bin/extra", b"x"),
    ];
    assert!(
        install(
            &manifest,
            &local(extra),
            &signature,
            &open_revocations(1_000)
        )
        .is_err()
    );
}

fn grant(capability: &str, scope: &str) -> PermissionGrant {
    PermissionGrant {
        capability: capability.to_owned(),
        scope: scope.to_owned(),
    }
}

fn source_state() -> DeploymentState {
    DeploymentState {
        deployment: Deployment::Local,
        permissions: vec![
            grant("avatar.realistic", "render"),
            grant("admin.team", "manage"),
        ],
        evidence_version: "evidence-2026-10-10".to_owned(),
        data_hash: "a".repeat(64),
    }
}

fn full_map() -> PermissionMap {
    let mut map = PermissionMap::default();
    for target in [Deployment::Hybrid, Deployment::Distributed] {
        map.insert(
            target,
            grant("avatar.realistic", "render"),
            grant("avatar.realistic", "render.remote"),
        );
        map.insert(
            target,
            grant("admin.team", "manage"),
            grant("admin.team", "admin"),
        );
    }
    map
}

fn request(target: Deployment) -> MigrationRequest {
    MigrationRequest {
        target,
        live_sessions: 0,
        sqlite_writers: 1,
    }
}

#[test]
fn a_live_session_blocks_migration() {
    let mut busy = request(Deployment::Hybrid);
    busy.live_sessions = 1;
    let refusal = plan_migration(&source_state(), &busy, &full_map()).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::CapabilityNotReady);
    assert_eq!(refusal.reason, "migration requires a maintenance window");
}

#[test]
fn a_multi_writer_sqlite_target_is_refused() {
    let mut multi = request(Deployment::Distributed);
    multi.sqlite_writers = 2;
    let refusal = plan_migration(&source_state(), &multi, &full_map()).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::Conflict);
    assert_eq!(
        refusal.reason,
        "sqlite store cannot be written by several nodes"
    );

    // the same writer count is fine on a hybrid target
    let mut hybrid = request(Deployment::Hybrid);
    hybrid.sqlite_writers = 2;
    assert!(plan_migration(&source_state(), &hybrid, &full_map()).is_ok());
}

#[test]
fn migrating_to_the_current_deployment_is_refused() {
    let refusal =
        plan_migration(&source_state(), &request(Deployment::Local), &full_map()).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::InvalidArgument);
    assert_eq!(
        refusal.reason,
        "migration target equals the current deployment"
    );
}

#[test]
fn an_unmapped_permission_is_refused_rather_than_dropped() {
    let mut partial = PermissionMap::default();
    partial.insert(
        Deployment::Hybrid,
        grant("avatar.realistic", "render"),
        grant("avatar.realistic", "render.remote"),
    );
    let refusal =
        plan_migration(&source_state(), &request(Deployment::Hybrid), &partial).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::InvalidArgument);
    assert_eq!(
        refusal.reason,
        "permission has no mapping in the target deployment"
    );
}

#[test]
fn migration_preserves_permissions_evidence_version_and_hash() {
    let source = source_state();
    let plan = plan_migration(&source, &request(Deployment::Hybrid), &full_map()).unwrap();
    assert_eq!(plan.mappings.len(), 2);
    // the expected set is the *mapped* one, sorted — the target's own names
    assert_eq!(
        plan.expected,
        vec![
            grant("admin.team", "admin"),
            grant("avatar.realistic", "render.remote")
        ]
    );

    let mut host = RehearsalMigration::new(source.clone());
    let exported = host.export(&op()).unwrap();
    assert_eq!(
        exported, source,
        "a faithful export does not touch the state"
    );
    let imported = host.import(&op(), &plan).unwrap();
    verify_migration(&plan, &imported).unwrap();

    assert_eq!(host.state().deployment, Deployment::Hybrid);
    assert_eq!(host.state().permissions, plan.expected);
    assert_eq!(host.state().evidence_version, source.evidence_version);
    assert_eq!(host.state().data_hash, source.data_hash);
}

#[test]
fn verify_migration_classifies_each_mismatch() {
    let source = source_state();
    let plan = plan_migration(&source, &request(Deployment::Hybrid), &full_map()).unwrap();
    let good = ImportedState {
        permissions: plan.expected.clone(),
        evidence_version: source.evidence_version.clone(),
        data_hash: source.data_hash.clone(),
    };
    assert!(verify_migration(&plan, &good).is_ok());
    // permission order is not part of the contract
    let mut reordered = good.clone();
    reordered.permissions.reverse();
    assert!(verify_migration(&plan, &reordered).is_ok());

    let mut dropped = good.clone();
    dropped.permissions.pop();
    let refusal = verify_migration(&plan, &dropped).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::InvalidArgument);
    assert_eq!(
        refusal.reason,
        "migrated permissions differ from the mapped set"
    );

    // a version disagreement is not a hash disagreement, and vice versa
    let mut drifted = good.clone();
    drifted.evidence_version = "evidence-other".to_owned();
    let refusal = verify_migration(&plan, &drifted).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::VersionIncompatible);
    assert_eq!(
        refusal.reason,
        "migrated evidence version differs from the source"
    );

    let mut tampered = good.clone();
    tampered.data_hash = "b".repeat(64);
    let refusal = verify_migration(&plan, &tampered).unwrap_err();
    assert_eq!(refusal.code, ErrorCode::InvalidArgument);
    assert_eq!(refusal.reason, "migrated data hash differs from the source");
}

#[test]
fn plausible_import_mistakes_are_caught() {
    let source = source_state();
    let plan = plan_migration(&source, &request(Deployment::Distributed), &full_map()).unwrap();

    for (fault, expected) in [
        (
            MigrationFault::DroppedPermission,
            ErrorCode::InvalidArgument,
        ),
        (
            MigrationFault::EvidenceVersionDrift,
            ErrorCode::VersionIncompatible,
        ),
        (MigrationFault::DataHashDrift, ErrorCode::InvalidArgument),
    ] {
        let mut host = RehearsalMigration::new(source.clone()).with_fault(fault);
        let imported = host.import(&op(), &plan).unwrap();
        assert_eq!(
            verify_migration(&plan, &imported).unwrap_err().code,
            expected,
            "{fault:?} should have been caught"
        );
    }
}

#[test]
fn output_switch_announces_once() {
    let mut switch = OutputSwitch::default();
    assert_eq!(
        switch.switch(&op(), "session-1/epoch-7").unwrap(),
        Announcement::Announced
    );
    // a retry inside the maintenance window must not repeat the announcement
    assert_eq!(
        switch.switch(&op(), "session-1/epoch-7").unwrap(),
        Announcement::Duplicate
    );
    assert_eq!(switch.announced_count(), 1);
    // a genuinely new switch still announces
    assert_eq!(
        switch.switch(&op(), "session-1/epoch-8").unwrap(),
        Announcement::Announced
    );
    assert_eq!(switch.announced_count(), 2);

    assert_eq!(
        switch.switch(&op(), "").unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        switch.switch(&op(), &"k".repeat(129)).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

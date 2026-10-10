//! End-to-end checks for verify_package and preflight, across the module seams.
use nai_contracts::v1::ErrorCode;
use nai_upgrade_core::{
    DeterministicVerifier, HostFacts, NoSpaceProbe, PackageFile, Probe, ResourceState,
    RevocationEntry, RevocationList, SpaceProbe, UpgradePackage, VerifyLimits, verify_package,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn open_revocations(now_ms: i64) -> RevocationList {
    RevocationList::new(now_ms, now_ms + 60_000, []).unwrap()
}

/// Build a schema-valid manifest whose `files` array matches `files` exactly,
/// then apply `mutate` so a test can break one field at a time.
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

fn files<'a>(entries: &'a [(&'a str, &'a [u8])]) -> Vec<PackageFile<'a>> {
    entries
        .iter()
        .map(|(path, content)| PackageFile { path, content })
        .collect()
}

fn verify(
    manifest: &[u8],
    entries: &[PackageFile<'_>],
    signature: &[u8],
    revocations: &RevocationList,
) -> nai_upgrade_core::Result<nai_upgrade_core::VerifiedPackage> {
    verify_package(
        manifest,
        entries,
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
fn a_well_formed_package_verifies() {
    let content: &[(&str, &[u8])] = &[("bin/app", b"binary-payload"), ("data/model", b"weights")];
    let manifest = build_manifest(content, |_| {});
    let signature = DeterministicVerifier::sign(&manifest);
    let verified = verify(
        &manifest,
        &files(content),
        &signature,
        &open_revocations(1_000),
    )
    .unwrap();
    assert_eq!(verified.package.package_id, "plugin.demo");
    assert_eq!(verified.content_bytes, 21);
    assert_eq!(verified.package.cost.cloud, Some(10));
    assert_eq!(verified.package.cost.download, None);
}

#[test]
fn content_must_match_declaration_exactly() {
    let declared: &[(&str, &[u8])] = &[("bin/app", b"binary-payload")];
    let manifest = build_manifest(declared, |_| {});
    let signature = DeterministicVerifier::sign(&manifest);
    let revocations = open_revocations(1_000);

    // tampered bytes -> hash mismatch
    let tampered: &[(&str, &[u8])] = &[("bin/app", b"binary-payloaX")];
    assert_eq!(
        verify(&manifest, &files(tampered), &signature, &revocations)
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );

    // an extra undeclared file is refused
    let extra: &[(&str, &[u8])] = &[("bin/app", b"binary-payload"), ("bin/extra", b"x")];
    assert!(verify(&manifest, &files(extra), &signature, &revocations).is_err());

    // a missing file is refused
    let missing: &[PackageFile<'_>] = &[];
    assert!(verify(&manifest, missing, &signature, &revocations).is_err());

    // a duplicate provided path is refused
    let duplicate: &[(&str, &[u8])] = &[
        ("bin/app", b"binary-payload"),
        ("bin/app", b"binary-payload"),
    ];
    assert!(verify(&manifest, &files(duplicate), &signature, &revocations).is_err());
}

#[test]
fn duplicate_declared_path_is_refused_by_parsing() {
    let content: &[(&str, &[u8])] = &[("bin/app", b"payload")];
    let manifest = build_manifest(content, |value| {
        let files = value["files"].as_array_mut().unwrap();
        files.push(files[0].clone());
    });
    assert_eq!(
        UpgradePackage::parse(&manifest).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn signature_and_revocation_gate_staging() {
    let content: &[(&str, &[u8])] = &[("bin/app", b"payload")];
    let manifest = build_manifest(content, |_| {});
    let signature = DeterministicVerifier::sign(&manifest);

    // wrong signature
    assert_eq!(
        verify(
            &manifest,
            &files(content),
            &DeterministicVerifier::sign(b"other"),
            &open_revocations(1_000)
        )
        .unwrap_err()
        .code,
        ErrorCode::PermissionDenied
    );

    // revoked subject
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
        verify(&manifest, &files(content), &signature, &revoked)
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );

    // an expired cache refuses even when the subject is absent
    let stale = RevocationList::new(500, 1_000, []).unwrap();
    assert_eq!(
        verify(&manifest, &files(content), &signature, &stale)
            .unwrap_err()
            .code,
        ErrorCode::Unauthenticated
    );
}

#[test]
fn limits_and_cancellation_are_enforced() {
    let content: &[(&str, &[u8])] = &[("bin/app", b"payload")];
    let manifest = build_manifest(content, |_| {});
    let signature = DeterministicVerifier::sign(&manifest);
    let revocations = open_revocations(1_000);
    let provided = files(content);

    let too_many = VerifyLimits {
        max_files: 0,
        max_bytes: u64::MAX,
    };
    assert_eq!(
        verify_package(
            &manifest,
            &provided,
            &signature,
            &DeterministicVerifier,
            &revocations,
            1_000,
            too_many,
            deadline(),
            &AtomicBool::new(false)
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceExhausted
    );

    let tiny = VerifyLimits {
        max_files: 8,
        max_bytes: 1,
    };
    assert!(
        verify_package(
            &manifest,
            &provided,
            &signature,
            &DeterministicVerifier,
            &revocations,
            1_000,
            tiny,
            deadline(),
            &AtomicBool::new(false)
        )
        .is_err()
    );

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        verify_package(
            &manifest,
            &provided,
            &signature,
            &DeterministicVerifier,
            &revocations,
            1_000,
            VerifyLimits::default(),
            deadline(),
            &cancelled
        )
        .unwrap_err()
        .code,
        ErrorCode::Cancelled
    );

    let expired = AtomicBool::new(false);
    assert_eq!(
        verify_package(
            &manifest,
            &provided,
            &signature,
            &DeterministicVerifier,
            &revocations,
            1_000,
            VerifyLimits::default(),
            Instant::now(),
            &expired
        )
        .unwrap_err()
        .code,
        ErrorCode::DeadlineExceeded
    );
}

#[test]
fn schema_rejection_precedes_everything() {
    let manifest = br#"{"manifest_schema":1,"package_id":"x"}"#;
    assert_eq!(
        UpgradePackage::parse(manifest).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

struct FixedSpace(u64);
impl SpaceProbe for FixedSpace {
    fn free_bytes(&self) -> Probe {
        Probe::Measured(self.0)
    }
}
struct UnknownSpace;
impl SpaceProbe for UnknownSpace {
    fn free_bytes(&self) -> Probe {
        Probe::Unknown("no probe")
    }
}

fn facts() -> HostFacts {
    HostFacts {
        host: "inproc".into(),
        os: "linux".into(),
        architecture: "x86_64".into(),
        kernel_version: "1.96.0".into(),
        capabilities: vec!["cap.one".into()],
        runtime_ids: vec!["rt-cpu".into()],
        credentials_available: ResourceState::Sufficient,
    }
}

fn package() -> UpgradePackage {
    let content: &[(&str, &[u8])] = &[("bin/app", b"payload")];
    UpgradePackage::parse(&build_manifest(content, |_| {})).unwrap()
}

#[test]
fn preflight_admits_a_supported_target() {
    let report = nai_upgrade_core::preflight(&package(), &facts(), &FixedSpace(1 << 30)).unwrap();
    assert!(
        report.admitted(),
        "refusals: {:?}",
        report.refusals().collect::<Vec<_>>()
    );
    assert!(report.refusal_error().is_none());
}

#[test]
fn preflight_refuses_on_each_axis() {
    let support = FixedSpace(1 << 30);
    let base = nai_upgrade_core::preflight(&package(), &facts(), &support).unwrap();
    assert!(base.admitted());

    // unsupported OS
    let mut wrong_os = facts();
    wrong_os.os = "windows".into();
    let report = nai_upgrade_core::preflight(&package(), &wrong_os, &support).unwrap();
    assert_eq!(
        report.refusal_error().unwrap().code,
        ErrorCode::ResourceExhausted
    );

    // kernel outside range
    let mut wrong_kernel = facts();
    wrong_kernel.kernel_version = "2.0.0".into();
    assert!(
        !nai_upgrade_core::preflight(&package(), &wrong_kernel, &support)
            .unwrap()
            .admitted()
    );

    // dependency capability absent
    let mut missing_dependency = facts();
    missing_dependency.capabilities = vec![];
    assert!(
        !nai_upgrade_core::preflight(&package(), &missing_dependency, &support)
            .unwrap()
            .admitted()
    );

    // insufficient space (needs 1024 + 2048)
    assert!(
        !nai_upgrade_core::preflight(&package(), &facts(), &FixedSpace(100))
            .unwrap()
            .admitted()
    );

    // unknown space is refused and classified distinctly from insufficient
    let unknown = nai_upgrade_core::preflight(&package(), &facts(), &UnknownSpace).unwrap();
    assert!(!unknown.admitted());
    assert_eq!(
        unknown.refusal_error().unwrap().code,
        ErrorCode::CapabilityNotReady
    );

    // the default no-probe also refuses rather than assuming room
    assert!(
        !nai_upgrade_core::preflight(&package(), &facts(), &NoSpaceProbe)
            .unwrap()
            .admitted()
    );

    // unknown credentials are refused
    let mut unknown_credentials = facts();
    unknown_credentials.credentials_available = ResourceState::Unknown("vault unreachable");
    assert!(
        !nai_upgrade_core::preflight(&package(), &unknown_credentials, &support)
            .unwrap()
            .admitted()
    );
}

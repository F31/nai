//! Manifest conversion and plugin surface: framing, handshake, Plugin trait,
//! PluginContext, CapabilityClient and the platform connector seam. Not a native
//! loader or authorization authority; host isolation is enforced by
//! nai-plugin-hosts.
pub mod connector;
pub mod frame;
pub mod handshake;
pub mod model;
pub mod plugin;

use nai_contracts::{
    CAPABILITY_IDS,
    v1::{ContractError, ErrorCode, HostKind, PluginRegistration, ProtocolRange},
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

pub struct ManifestTarget<'a> {
    pub kernel_version: &'a str,
    pub os: &'a str,
    pub architecture: &'a str,
}
fn error(code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        code: code as i32,
        category: "manifest".into(),
        message: message.into(),
        ..Default::default()
    }
}
fn digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn parse_manifest(
    bytes: &[u8],
    target: ManifestTarget<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PluginRegistration, ContractError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(error(ErrorCode::Cancelled, "manifest cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(error(
            ErrorCode::DeadlineExceeded,
            "manifest deadline exceeded",
        ));
    }
    if bytes.len() > 65536 {
        return Err(error(ErrorCode::ResourceExhausted, "manifest too large"));
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid manifest JSON"))?;
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/plugin/manifest.schema.json"))
            .map_err(|_| error(ErrorCode::Internal, "embedded schema invalid"))?;
    if !jsonschema::validator_for(&schema)
        .map_err(|_| error(ErrorCode::Internal, "validator unavailable"))?
        .is_valid(&value)
    {
        return Err(error(
            ErrorCode::InvalidArgument,
            "manifest schema rejected",
        ));
    }
    let text = |name: &str| value[name].as_str().unwrap_or_default().to_owned();
    let strings = |name: &str| {
        value[name]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let version = semver::Version::parse(target.kernel_version)
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid target kernel version"))?;
    let range = semver::VersionReq::parse(&text("kernel_range"))
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid kernel range"))?;
    semver::Version::parse(&text("plugin_version"))
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid plugin version"))?;
    if !range.matches(&version)
        || !strings("os").iter().any(|os| os == target.os)
        || !strings("architectures")
            .iter()
            .any(|arch| arch == target.architecture)
    {
        return Err(error(
            ErrorCode::VersionIncompatible,
            "manifest target incompatible",
        ));
    }
    let contracts = strings("contracts");
    let requires = strings("requires");
    if contracts.len() > 64
        || requires.len() > 64
        || contracts
            .iter()
            .chain(&requires)
            .any(|cap| CAPABILITY_IDS.binary_search(&cap.as_str()).is_err())
    {
        return Err(error(
            ErrorCode::CapabilityAbsent,
            "unknown formal capability",
        ));
    }
    let supported_hosts = strings("supported_hosts")
        .iter()
        .map(|host| match host.as_str() {
            "inproc" => HostKind::Inproc as i32,
            "subprocess" => HostKind::Subprocess as i32,
            "remote" => HostKind::Remote as i32,
            "wasm" => HostKind::Wasm as i32,
            _ => 0,
        })
        .collect();
    let registration = PluginRegistration {
        plugin_id: text("plugin_id"),
        plugin_version: text("plugin_version"),
        artifact_hash: text("artifact_hash"),
        manifest_hash: digest(bytes),
        validated_kernel_version: target.kernel_version.into(),
        protocol_ranges: vec![ProtocolRange {
            major: 1,
            min_minor: 0,
            max_minor: 0,
        }],
        contracts,
        supported_hosts,
        requires,
        activation: text("activation"),
        idle_policy: text("idle_policy"),
        resource_profile_ref: value["resources"]["profile_id"]
            .as_str()
            .unwrap_or_default()
            .into(),
        certification_ref: value["resources"]["certification_ref"]
            .as_str()
            .unwrap_or_default()
            .into(),
        license_ref: text("license_ref"),
        declares_network: value["permissions"]["network"].as_bool().unwrap_or(false),
        declares_media_output: value["permissions"]["media_output"]
            .as_bool()
            .unwrap_or(false),
        credential_refs: value["permissions"]["secrets"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    };
    if Instant::now() >= deadline {
        return Err(error(
            ErrorCode::DeadlineExceeded,
            "manifest deadline exceeded",
        ));
    }
    if cancelled.load(Ordering::Acquire) {
        return Err(error(ErrorCode::Cancelled, "manifest cancelled"));
    }
    Ok(registration)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> serde_json::Value {
        serde_json::json!({"manifest_schema":1,"plugin_id":"test-guard","plugin_version":"0.1.0","kernel_range":">=0.1.0,<1.0.0","protocol_major":1,"contracts":["guard.check@1"],"supported_hosts":["inproc"],"activation":"on_demand","idle_policy":"stop_after_job","requires":[],"os":["windows","linux"],"architectures":["x86_64"],"artifact_hash":"a".repeat(64),"license_ref":"test-only","install_bytes":"100","config_schema_ref":"test","data_schema_ref":"test","health_operation":"health","migration_ref":"none","rollback_range":"0.1.x","price_policy_ref":"none","resources":{"profile_id":"test","certification_ref":"not-real-certification"},"permissions":{"network":false,"media_output":false,"secrets":[]}})
    }
    fn parse(value: &serde_json::Value) -> Result<PluginRegistration, ContractError> {
        parse_manifest(
            &serde_json::to_vec(value).unwrap(),
            ManifestTarget {
                kernel_version: "0.1.0",
                os: "linux",
                architecture: "x86_64",
            },
            Instant::now() + std::time::Duration::from_secs(5),
            &AtomicBool::new(false),
        )
    }
    #[test]
    fn valid_manifest_converts_without_granting_readiness_or_trust() {
        let result = parse(&fixture()).unwrap();
        assert_eq!(result.validated_kernel_version, "0.1.0");
        assert_eq!(result.contracts, ["guard.check@1"]);
        assert_eq!(result.manifest_hash.len(), 64);
    }
    #[test]
    fn unknown_fields_bad_ranges_and_target_mismatch_fail() {
        for (key, value) in [
            ("is_trusted", serde_json::json!(true)),
            ("kernel_range", serde_json::json!("not-a-range")),
            ("kernel_range", serde_json::json!(">=2.0.0")),
            ("os", serde_json::json!(["macos"])),
            ("architectures", serde_json::json!(["aarch64"])),
            ("contracts", serde_json::json!(["unknown.fake@1"])),
        ] {
            let mut manifest = fixture();
            manifest[key] = value;
            assert!(parse(&manifest).is_err(), "{key}");
        }
    }
    #[test]
    fn malformed_unbounded_expired_or_cancelled_input_fails() {
        let target = || ManifestTarget {
            kernel_version: "0.1.0",
            os: "linux",
            architecture: "x86_64",
        };
        assert!(
            parse_manifest(
                &vec![0; 65537],
                target(),
                Instant::now() + std::time::Duration::from_secs(1),
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            parse_manifest(
                b"{",
                target(),
                Instant::now() + std::time::Duration::from_secs(1),
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(parse_manifest(b"{}", target(), Instant::now(), &AtomicBool::new(false)).is_err());
        assert_eq!(
            parse_manifest(
                b"{}",
                target(),
                Instant::now() + std::time::Duration::from_secs(1),
                &AtomicBool::new(true)
            )
            .unwrap_err()
            .code,
            ErrorCode::Cancelled as i32
        );
    }
}

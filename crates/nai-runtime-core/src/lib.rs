//! RF01 bootstrap inspection. No sessions, storage, provider, device or output grants.
pub mod credential_rewrap;
pub mod migration_backup;
pub mod personal_policy_migration;
pub mod public_guard;
pub mod public_media;
pub mod public_output_binding;
pub mod public_standby;
pub mod v561_narration;
pub mod v561_probe;
pub mod workspace_budget;
use nai_contracts::workbench::*;
use nai_contracts::workbench_wire::{FRAME_LIMIT, TTL_MS, VERSION};
use std::{
    fs,
    path::{Component, Path},
    time::Duration,
};
pub fn config(presentation: Presentation) -> WorkbenchConfig {
    WorkbenchConfig {
        edition: Edition::Lite as i32,
        deployment: Deployment::Local as i32,
        presentation: presentation as i32,
        behavior: Behavior::Assisted as i32,
        delivery_scope: DeliveryScope::Private as i32,
        queue_limit: 2,
        task_deadline_ms: 3000,
        shadow: false,
    }
}
pub fn validate_config(c: &WorkbenchConfig) -> Result<(), WorkbenchError> {
    if !matches!(
        Edition::try_from(c.edition),
        Ok(Edition::Lite | Edition::Standard | Edition::Pro)
    ) || !matches!(
        Deployment::try_from(c.deployment),
        Ok(Deployment::Local | Deployment::Hybrid | Deployment::Distributed)
    ) || !matches!(
        Presentation::try_from(c.presentation),
        Ok(Presentation::ContentOnly
            | Presentation::Voice
            | Presentation::Avatar
            | Presentation::HumanAssist)
    ) || !matches!(
        Behavior::try_from(c.behavior),
        Ok(Behavior::Rehearsal | Behavior::Assisted | Behavior::AutoSupervised)
    ) || !matches!(
        DeliveryScope::try_from(c.delivery_scope),
        Ok(DeliveryScope::Private | DeliveryScope::Public)
    ) || !(1..=16).contains(&c.queue_limit)
        || !(100..=30000).contains(&c.task_deadline_ms)
        || c.shadow && c.delivery_scope == DeliveryScope::Public as i32
    {
        return Err(WorkbenchError::InvalidArgument);
    }
    Ok(())
}
/// Validate a data-root path: absolute, no parent traversal, no symlink/reparse
/// component. Unlike `validate_data_root`, an existing non-empty root is allowed.
pub fn validate_data_root_path(root: &Path) -> Result<(), WorkbenchError> {
    if !root.is_absolute() || root.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(WorkbenchError::DataRootRejected);
    }
    let mut prefix = std::path::PathBuf::new();
    for component in root.components() {
        prefix.push(component);
        match fs::symlink_metadata(&prefix) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    return Err(WorkbenchError::DataRootRejected);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err(WorkbenchError::DataRootRejected);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(WorkbenchError::DataRootRejected),
        }
    }
    Ok(())
}

/// Inspect only names/metadata. Never open or create a legacy database or credentials.
pub fn validate_data_root(root: &Path) -> Result<(), WorkbenchError> {
    validate_data_root_path(root)?;
    if root.exists() {
        let mut entries = fs::read_dir(root).map_err(|_| WorkbenchError::DataRootRejected)?;
        if entries.next().is_some() {
            return Err(WorkbenchError::DataRootRejected);
        }
    }
    Ok(())
}
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn problem(id: &str, seq: u64, code: WorkbenchError) -> WorkbenchResponse {
    let id = if safe_id(id) { id } else { "" };
    WorkbenchResponse {
        request_id: id.into(),
        seq,
        handshake: Some(handshake()),
        outcome: Some(workbench_response::Outcome::Problem(WorkbenchProblem {
            code: code as i32,
            safe_message: code.as_str_name().into(),
            request_id: id.into(),
            retryable: false,
            field_errors: vec![],
        })),
    }
}
fn handshake() -> WorkbenchHandshake {
    WorkbenchHandshake {
        protocol_major: VERSION,
        max_frame_bytes: FRAME_LIMIT as u32,
        build: env!("CARGO_PKG_VERSION").into(),
        clock: "monotonic-process".into(),
        lease_ms: 0,
    }
}
pub fn inspect(c: WorkbenchConfig, prepare: bool) -> WorkbenchCapabilities {
    let mut required = vec![
        "content.objects@1",
        "live.program@1",
        "guard.check@1",
        "delivery.dispatch@1",
        "storage.persist@1",
    ];
    if matches!(
        Presentation::try_from(c.presentation),
        Ok(Presentation::Voice | Presentation::Avatar)
    ) {
        required.extend(["tts.synthesize@1", "media.timeline@1", "output.deliver@1"]);
    }
    if c.presentation == Presentation::Avatar as i32 {
        required.extend(["avatar.profile@1", "avatar.render@1"]);
    }
    let mut capabilities = vec![WorkbenchCapabilityView {
        capability_id: "workbench.inspect@1".into(),
        implementation: WorkbenchImplementation::Implemented as i32,
        available: true,
        reason_code: String::new(),
    }];
    capabilities.extend(required.iter().map(|id| WorkbenchCapabilityView {
        capability_id: (*id).into(),
        implementation: WorkbenchImplementation::NotStarted as i32,
        available: false,
        reason_code: "NOT_IMPLEMENTED_RF02_ONWARD".into(),
    }));
    WorkbenchCapabilities {
        config: Some(c),
        capabilities,
        required_capabilities: required.into_iter().map(String::from).collect(),
        business_ready: false,
        status: if prepare {
            WorkbenchReadiness::NotReady
        } else {
            WorkbenchReadiness::ControlAvailable
        } as i32,
        started_media_processes: 0,
        provider_calls: 0,
    }
}
pub fn handle(r: WorkbenchRequest, token: &[u8], elapsed: Duration) -> WorkbenchResponse {
    let result = (|| {
        if r.protocol_major != VERSION {
            return Err(WorkbenchError::ProtocolIncompatible);
        }
        if token.len() != 64
            || r.authorization.len() != 64
            || token
                .iter()
                .zip(&r.authorization)
                .fold(0u8, |a, (b, c)| a | (b ^ c))
                != 0
        {
            return Err(WorkbenchError::AuthenticationFailed);
        }
        if r.seq != 1 || !safe_id(&r.request_id) {
            return Err(WorkbenchError::ProtocolError);
        }
        if r.ttl_ms == 0 || r.ttl_ms > TTL_MS {
            return Err(WorkbenchError::InvalidArgument);
        }
        if elapsed >= Duration::from_millis(r.ttl_ms as u64) {
            return Err(WorkbenchError::DeadlineExceeded);
        }
        let (c, prepare) = match r.operation {
            Some(workbench_request::Operation::Inspect(i)) => (i.config, false),
            Some(workbench_request::Operation::Prepare(p)) => (p.config, true),
            None => return Err(WorkbenchError::InvalidArgument),
        };
        let c = c.ok_or(WorkbenchError::InvalidArgument)?;
        validate_config(&c)?;
        Ok(inspect(c, prepare))
    })();
    match result {
        Ok(c) => WorkbenchResponse {
            request_id: r.request_id,
            seq: r.seq,
            handshake: Some(handshake()),
            outcome: Some(workbench_response::Outcome::Capabilities(c)),
        },
        Err(e) => problem(&r.request_id, r.seq, e),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn req() -> WorkbenchRequest {
        WorkbenchRequest {
            protocol_major: VERSION,
            authorization: vec![b'a'; 64],
            request_id: "test-1".into(),
            seq: 1,
            ttl_ms: 3000,
            operation: Some(workbench_request::Operation::Inspect(WorkbenchInspect {
                config: Some(config(Presentation::ContentOnly)),
            })),
        }
    }
    fn code(r: WorkbenchResponse) -> WorkbenchError {
        match r.outcome.unwrap() {
            workbench_response::Outcome::Problem(p) => WorkbenchError::try_from(p.code).unwrap(),
            _ => panic!("expected failure"),
        }
    }
    #[test]
    fn identity_version_sequence_deadline_and_unknowns_fail() {
        let good = req();
        for (r, want) in [
            (
                WorkbenchRequest {
                    protocol_major: 1,
                    ..good.clone()
                },
                WorkbenchError::ProtocolIncompatible,
            ),
            (
                WorkbenchRequest {
                    authorization: vec![b'b'; 64],
                    ..good.clone()
                },
                WorkbenchError::AuthenticationFailed,
            ),
            (
                WorkbenchRequest {
                    seq: 2,
                    ..good.clone()
                },
                WorkbenchError::ProtocolError,
            ),
            (
                WorkbenchRequest {
                    operation: None,
                    ..good.clone()
                },
                WorkbenchError::InvalidArgument,
            ),
            (
                WorkbenchRequest {
                    ttl_ms: 0,
                    ..good.clone()
                },
                WorkbenchError::InvalidArgument,
            ),
        ] {
            assert_eq!(code(handle(r, &[b'a'; 64], Duration::ZERO)), want);
        }
        assert_eq!(
            code(handle(good, &[b'a'; 64], Duration::from_secs(3))),
            WorkbenchError::DeadlineExceeded
        );
    }
    #[test]
    fn unknown_dimensions_bounds_and_shadow_public_rejected() {
        let c = config(Presentation::ContentOnly);
        for bad in [
            WorkbenchConfig { edition: 99, ..c },
            WorkbenchConfig { deployment: 0, ..c },
            WorkbenchConfig {
                presentation: 99,
                ..c
            },
            WorkbenchConfig { behavior: 99, ..c },
            WorkbenchConfig {
                delivery_scope: 0,
                ..c
            },
            WorkbenchConfig {
                queue_limit: 17,
                ..c
            },
            WorkbenchConfig {
                task_deadline_ms: 30001,
                ..c
            },
            WorkbenchConfig {
                shadow: true,
                delivery_scope: DeliveryScope::Public as i32,
                ..c
            },
        ] {
            assert_eq!(validate_config(&bad), Err(WorkbenchError::InvalidArgument));
        }
    }
    #[test]
    fn modes_are_independent_and_no_business_ready() {
        for e in [Edition::Lite, Edition::Standard, Edition::Pro] {
            for p in [
                Presentation::ContentOnly,
                Presentation::Voice,
                Presentation::Avatar,
                Presentation::HumanAssist,
            ] {
                let c = WorkbenchConfig {
                    edition: e as i32,
                    ..config(p)
                };
                validate_config(&c).unwrap();
                let v = inspect(c, true);
                assert!(!v.business_ready);
                assert_eq!(v.status, WorkbenchReadiness::NotReady as i32);
                assert_eq!(v.provider_calls, 0);
                assert_eq!(v.started_media_processes, 0);
                assert_eq!(
                    v.required_capabilities
                        .iter()
                        .any(|s| s == "tts.synthesize@1"),
                    matches!(p, Presentation::Voice | Presentation::Avatar)
                );
                assert_eq!(
                    v.required_capabilities
                        .iter()
                        .any(|s| s == "avatar.render@1"),
                    p == Presentation::Avatar
                );
            }
        }
    }
}

pub mod assistance;
/// RF02 policy ports; no database or concrete output dependencies.
pub mod foundation;
pub mod llm;
pub mod preparation;
pub mod publication;

pub mod tts;

pub mod audition;

pub mod public_execution;
pub mod task_context;

pub mod live;
pub mod narration;

pub mod human_mic;

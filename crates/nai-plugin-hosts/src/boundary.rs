//! Verified load paths, environment scrubbing and the honest native isolation
//! boundary. A native subprocess is only hosted when a certified OS sandbox
//! (AppContainer / restricted identity / Job Object) has been verified; without
//! that evidence the boundary returns an explicit Unavailable.
use nai_contracts::v1::{ContractError, ErrorCode};
use std::path::{Path, PathBuf};

fn error(request_id: &str, code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        request_id: request_id.into(),
        code: code as i32,
        category: "boundary".into(),
        message: message.into(),
        ..Default::default()
    }
}

/// Peer process identity as extracted from the actual transport (Unix socket
/// peer credentials or Windows named-pipe client PID). `None` means the
/// transport did not provide OS identity; an `IdentityValidator` must decide
/// whether that is acceptable rather than treating `None` as authenticated.
#[derive(Debug, Clone, Copy)]
pub struct OsPeer {
    pub pid: u32,
    pub uid: u32,
    pub gid: u32,
}

/// Resolves `candidate` and requires it to reside under `root`. Rejects lexical
/// `..` traversal, absolute escapes, and any symlink/reparse-point whose
/// canonical target leaves `root`. `candidate` must exist to be canonicalized.
pub fn verify_within_root(root: &Path, candidate: &Path) -> Result<PathBuf, ContractError> {
    if !root.is_absolute() {
        return Err(error(
            "",
            ErrorCode::InvalidArgument,
            "root must be absolute",
        ));
    }
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(error(
            "",
            ErrorCode::PermissionDenied,
            "path traversal rejected",
        ));
    }
    if candidate.is_absolute() && !candidate.starts_with(root) {
        return Err(error(
            "",
            ErrorCode::PermissionDenied,
            "absolute escape rejected",
        ));
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|_| error("", ErrorCode::PermissionDenied, "root cannot be resolved"))?;
    let canonical = candidate.canonicalize().map_err(|_| {
        error(
            "",
            ErrorCode::PermissionDenied,
            "candidate cannot be resolved",
        )
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(error(
            "",
            ErrorCode::PermissionDenied,
            "canonical path escapes root",
        ));
    }
    Ok(canonical)
}

/// Removes environment variables that look like secrets (token, secret, key,
/// password, credential) so a spawned subprocess does not inherit them unless
/// they are explicitly re-added by the caller.
pub fn scrub_env(env: &[(String, String)]) -> Vec<(String, String)> {
    env.iter()
        .filter(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            ![
                "token",
                "secret",
                "password",
                "credential",
                "api_key",
                "apikey",
            ]
            .iter()
            .any(|needle| lower.contains(needle))
        })
        .cloned()
        .collect()
}

/// Honest native isolation boundary. Without certified evidence, hosting an
/// untrusted native subprocess is Unavailable. A later task that integrates a
/// verified OS sandbox supplies that evidence and narrows this gate.
pub struct NativeBoundary;

impl NativeBoundary {
    pub fn verify_subprocess_sandbox() -> Result<(), ContractError> {
        Err(error(
            "",
            ErrorCode::CapabilityNotReady,
            "NATIVE_SUBPROCESS_SANDBOX_NOT_VERIFIED",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_absolute_and_symlink_escape_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("inside.txt"), b"test").unwrap();
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, b"test").unwrap();

        // Lexical traversal.
        assert_eq!(
            verify_within_root(&root, &root.join("../root/inside.txt"))
                .unwrap_err()
                .code,
            ErrorCode::PermissionDenied as i32
        );
        // Absolute escape.
        assert_eq!(
            verify_within_root(&root, &outside).unwrap_err().code,
            ErrorCode::PermissionDenied as i32
        );
        // Inside path resolves fine.
        assert!(verify_within_root(&root, &root.join("inside.txt")).is_ok());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("link.txt")).unwrap();
            assert_eq!(
                verify_within_root(&root, &root.join("link.txt"))
                    .unwrap_err()
                    .code,
                ErrorCode::PermissionDenied as i32
            );
        }
    }

    #[test]
    fn env_scrub_removes_secret_like_variables_only() {
        let env = vec![
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("API_TOKEN".to_string(), "sk-test".to_string()),
            ("DATABASE_PASSWORD".to_string(), "pw".to_string()),
            ("NORMAL".to_string(), "value".to_string()),
        ];
        let clean = scrub_env(&env);
        assert!(
            clean
                .iter()
                .all(|(name, _)| name != "API_TOKEN" && name != "DATABASE_PASSWORD")
        );
        assert!(
            clean
                .iter()
                .any(|(name, value)| name == "PATH" && value == "/usr/bin")
        );
        assert!(
            clean
                .iter()
                .any(|(name, value)| name == "NORMAL" && value == "value")
        );
    }

    #[test]
    fn native_subprocess_without_certified_sandbox_is_unavailable() {
        assert_eq!(
            NativeBoundary::verify_subprocess_sandbox()
                .unwrap_err()
                .code,
            ErrorCode::CapabilityNotReady as i32
        );
    }
}

//! Revocation: an *expiring* trusted cache, not a bare set.
//!
//! The specification requires the revocation list to be consulted both at
//! install time and on every start, and to be a trusted cache with an expiry
//! (V3.6 §4.4/§11.6). Modelled as a plain set, the dangerous case — the cache is
//! stale, so "not listed" means "we do not know" — collapses into "not
//! revoked", which disables revocation exactly when it matters. Hence the
//! three-state check below.
use crate::{Result, error};
use nai_contracts::v1::ErrorCode;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationEntry {
    pub subject_ref: String,
    pub revoked_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RevocationList {
    issued_at_ms: i64,
    expires_at_ms: i64,
    entries: BTreeMap<String, i64>,
}

impl RevocationList {
    pub fn new(
        issued_at_ms: i64,
        expires_at_ms: i64,
        entries: impl IntoIterator<Item = RevocationEntry>,
    ) -> Result<Self> {
        if expires_at_ms <= issued_at_ms {
            return Err(error(
                ErrorCode::InvalidArgument,
                "revocation window invalid",
            ));
        }
        let mut map = BTreeMap::new();
        for entry in entries {
            if entry.subject_ref.trim().is_empty() || entry.subject_ref.len() > 128 {
                return Err(error(ErrorCode::InvalidArgument, "invalid revoked subject"));
            }
            if entry.revoked_at_ms > expires_at_ms {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "revocation recorded after expiry",
                ));
            }
            map.insert(entry.subject_ref, entry.revoked_at_ms);
        }
        Ok(Self {
            issued_at_ms,
            expires_at_ms,
            entries: map,
        })
    }

    pub fn issued_at_ms(&self) -> i64 {
        self.issued_at_ms
    }
    pub fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn evaluate(&self, subject_ref: &str, now_ms: i64) -> Result<()> {
        if self.entries.contains_key(subject_ref) {
            return Err(error(ErrorCode::PermissionDenied, "subject revoked"));
        }
        if now_ms >= self.expires_at_ms {
            return Err(error(
                ErrorCode::Unauthenticated,
                "revocation cache expired",
            ));
        }
        Ok(())
    }

    /// Consulted on the install path. A subject inside the list is refused; a
    /// subject absent from a *valid* list is allowed; a subject absent from an
    /// *expired* list is refused, because absence is then not evidence.
    pub fn check_install(&self, subject_ref: &str, now_ms: i64) -> Result<()> {
        self.evaluate(subject_ref, now_ms)
    }

    /// Consulted on every start. The rule is the same as install time; the entry
    /// point is separate so an integration cannot quietly implement only one.
    pub fn check_startup(&self, subject_ref: &str, now_ms: i64) -> Result<()> {
        self.evaluate(subject_ref, now_ms)
    }
}

/// Where a revocation list comes from. Production reads a trusted cache or a
/// signed list; tests provide a fixed one. The core never fetches it itself.
pub trait RevocationSource {
    fn list(&self) -> Result<RevocationList>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(expires_at_ms: i64) -> RevocationList {
        RevocationList::new(
            1_000,
            expires_at_ms,
            [RevocationEntry {
                subject_ref: "plugin.bad".into(),
                revoked_at_ms: 1_500,
            }],
        )
        .unwrap()
    }

    #[test]
    fn three_state_check() {
        let list = list(5_000);
        // present -> refused
        assert_eq!(
            list.check_install("plugin.bad", 2_000).unwrap_err().code,
            ErrorCode::PermissionDenied
        );
        // absent and valid -> allowed
        list.check_install("plugin.good", 2_000).unwrap();
        // a startup check reaches the same verdict
        assert_eq!(
            list.check_startup("plugin.bad", 2_000).unwrap_err().code,
            ErrorCode::PermissionDenied
        );
        // absent but expired -> refused, never treated as "nothing revoked"
        assert_eq!(
            list.check_install("plugin.good", 5_000).unwrap_err().code,
            ErrorCode::Unauthenticated
        );
        // a revoked subject stays revoked even after the cache would expire
        assert_eq!(
            list.check_startup("plugin.bad", 9_999).unwrap_err().code,
            ErrorCode::PermissionDenied
        );
    }

    #[test]
    fn invalid_windows_are_rejected() {
        assert!(RevocationList::new(10, 10, []).is_err());
        assert!(RevocationList::new(10, 5, []).is_err());
        assert!(
            RevocationList::new(
                1,
                10,
                [RevocationEntry {
                    subject_ref: String::new(),
                    revoked_at_ms: 2
                }]
            )
            .is_err()
        );
        assert!(
            RevocationList::new(
                1,
                10,
                [RevocationEntry {
                    subject_ref: "x".into(),
                    revoked_at_ms: 11
                }]
            )
            .is_err()
        );
    }
}

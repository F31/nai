//! Transport ACL: authorization is checked before capability dispatch and is
//! supplied by the trusted composition root, never read from plugin claims.
use std::collections::{BTreeMap, BTreeSet};

/// A granted capability operation for a principal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AccessRule {
    pub capability_id: String,
    pub operation: String,
}

#[derive(Debug, Clone, Default)]
pub struct AccessPolicy {
    // session -> principal -> allowed operations
    grants: BTreeMap<String, BTreeMap<String, BTreeSet<AccessRule>>>,
    // session -> principals bound to it
    sessions: BTreeMap<String, BTreeSet<String>>,
}

impl AccessPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `principal` to `session` and grants the given capability operations.
    pub fn grant(
        &mut self,
        session: &str,
        principal: &str,
        rules: impl IntoIterator<Item = AccessRule>,
    ) {
        self.sessions
            .entry(session.to_string())
            .or_default()
            .insert(principal.to_string());
        let grants = self
            .grants
            .entry(session.to_string())
            .or_default()
            .entry(principal.to_string())
            .or_default();
        grants.extend(rules);
    }

    pub fn revoke_session(&mut self, session: &str) {
        self.sessions.remove(session);
        self.grants.remove(session);
    }

    /// Returns false unless `principal` is bound to `session` and the exact
    /// capability/operation pair was granted. No unknown capability is assumed
    /// authorized.
    pub fn allow(
        &self,
        session: &str,
        principal: &str,
        capability_id: &str,
        operation: &str,
    ) -> bool {
        self.sessions
            .get(session)
            .is_some_and(|principals| principals.contains(principal))
            && self
                .grants
                .get(session)
                .and_then(|by_principal| by_principal.get(principal))
                .is_some_and(|rules| {
                    rules.contains(&AccessRule {
                        capability_id: capability_id.into(),
                        operation: operation.into(),
                    })
                })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(capability: &str, operation: &str) -> AccessRule {
        AccessRule {
            capability_id: capability.into(),
            operation: operation.into(),
        }
    }

    #[test]
    fn grants_are_exact_and_scoped() {
        let mut policy = AccessPolicy::new();
        policy.grant("session-a", "principal-1", [rule("guard.check@1", "check")]);
        assert!(policy.allow("session-a", "principal-1", "guard.check@1", "check"));
        assert!(!policy.allow("session-a", "principal-1", "guard.check@1", "recheck"));
        assert!(!policy.allow("session-a", "principal-2", "guard.check@1", "check"));
        assert!(!policy.allow("session-b", "principal-1", "guard.check@1", "check"));
        assert!(!policy.allow("session-a", "principal-1", "not.registered@1", "check"));
    }

    #[test]
    fn revoke_session_removes_all_grants() {
        let mut policy = AccessPolicy::new();
        policy.grant("session-a", "principal-1", [rule("guard.check@1", "check")]);
        policy.revoke_session("session-a");
        assert!(!policy.allow("session-a", "principal-1", "guard.check@1", "check"));
    }

    #[test]
    fn empty_policy_denies_everything() {
        let policy = AccessPolicy::new();
        assert!(!policy.allow("s", "p", "guard.check@1", "check"));
    }
}

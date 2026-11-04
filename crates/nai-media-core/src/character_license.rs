//! In-memory license ledger for character identity bind, revoke, and restore-fault.
//!
//! Binds `package_sha256` and `rights_sha256` on `CharacterIdentity` to an admitted
//! license record, then opens `CharacterPreviewHost` only while that bind is live.
//! Revoke cancels the host, drops action/media refs, and records the hashes.
//! Restoring revoked rights or an old epoch is a fault. This is not a production
//! database, IPC, public output, or playback permit.

use crate::character_host::{CharacterClockReading, CharacterHostError, CharacterPreviewHost};
use nai_contracts::character_identity_bound;
use nai_contracts::v1::{CharacterIdentity, CharacterRuntimeView};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterLicenseRecord {
    pub workspace_id: String,
    pub package_sha256: String,
    pub rights_sha256: String,
}

impl CharacterLicenseRecord {
    fn matches(&self, identity: &CharacterIdentity) -> bool {
        self.workspace_id == identity.workspace_id
            && self.package_sha256 == identity.package_sha256
            && self.rights_sha256 == identity.rights_sha256
    }

    fn key(&self) -> (String, String, String) {
        (
            self.workspace_id.clone(),
            self.package_sha256.clone(),
            self.rights_sha256.clone(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterLicenseError {
    Identity,
    Session,
    Epoch,
    Rights,
    AlreadyBound,
    Unbound,
    Restore,
    Closed,
    Host(CharacterHostError),
}

#[derive(Clone)]
struct BindSnapshot {
    license: CharacterLicenseRecord,
    identity: CharacterIdentity,
    session_id: String,
    epoch: u64,
    supported: Vec<String>,
}

struct LiveBind {
    snapshot: BindSnapshot,
    host: CharacterPreviewHost,
}

/// Media-layer license gate. One live bind at a time. Preview is not a playback permit.
pub struct CharacterLicenseGate {
    live: Option<LiveBind>,
    last_view: Option<CharacterRuntimeView>,
    last_bind: Option<BindSnapshot>,
    revoked: HashSet<(String, String, String)>,
    epoch_floor: HashMap<(String, String), u64>,
}

impl Default for CharacterLicenseGate {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterLicenseGate {
    pub fn new() -> Self {
        Self {
            live: None,
            last_view: None,
            last_bind: None,
            revoked: HashSet::new(),
            epoch_floor: HashMap::new(),
        }
    }

    pub fn bind(
        &mut self,
        license: CharacterLicenseRecord,
        identity: CharacterIdentity,
        session_id: String,
        epoch: u64,
        supported: Vec<String>,
    ) -> Result<CharacterRuntimeView, CharacterLicenseError> {
        if !character_identity_bound(&identity) {
            return Err(CharacterLicenseError::Identity);
        }
        if !license.matches(&identity) {
            return Err(CharacterLicenseError::Rights);
        }
        if self.revoked.contains(&license.key()) {
            return Err(CharacterLicenseError::Restore);
        }
        if epoch == 0 {
            return Err(CharacterLicenseError::Epoch);
        }
        let floor_key = (identity.workspace_id.clone(), session_id.clone());
        if self
            .epoch_floor
            .get(&floor_key)
            .is_some_and(|floor| epoch <= *floor)
        {
            return Err(CharacterLicenseError::Restore);
        }
        if self.live.is_some() {
            return Err(CharacterLicenseError::AlreadyBound);
        }
        let host = CharacterPreviewHost::open(
            identity.clone(),
            session_id.clone(),
            epoch,
            supported.clone(),
        )
        .map_err(map_host)?;
        let view = host.view();
        self.live = Some(LiveBind {
            snapshot: BindSnapshot {
                license,
                identity,
                session_id,
                epoch,
                supported,
            },
            host,
        });
        self.last_view = None;
        Ok(view)
    }

    pub fn begin_action(
        &mut self,
        action_id: &str,
        max_samples: u64,
    ) -> Result<CharacterRuntimeView, CharacterLicenseError> {
        self.live_host_mut()?
            .begin_action(action_id, max_samples)
            .map_err(map_host)
    }

    pub fn tick(
        &mut self,
        played_samples: u64,
    ) -> Result<CharacterClockReading, CharacterLicenseError> {
        self.live_host_mut()?.tick(played_samples).map_err(map_host)
    }

    pub fn revoke(&mut self) -> Result<CharacterRuntimeView, CharacterLicenseError> {
        let mut live = self.live.take().ok_or(CharacterLicenseError::Unbound)?;
        let view = live.host.cancel();
        self.revoked.insert(live.snapshot.license.key());
        self.record_floor(&live.snapshot);
        self.last_bind = Some(live.snapshot);
        self.last_view = Some(view.clone());
        debug_assert!(view.action_id.is_empty());
        debug_assert!(view.media.is_none());
        Ok(view)
    }

    /// Drops the live bind without recording revoked hashes or raising the epoch floor.
    /// Used when the workbench switches the previewed package.
    pub fn release(&mut self) -> Result<CharacterRuntimeView, CharacterLicenseError> {
        let mut live = self.live.take().ok_or(CharacterLicenseError::Unbound)?;
        let view = live.host.cancel();
        self.last_view = Some(view.clone());
        debug_assert!(view.action_id.is_empty());
        debug_assert!(view.media.is_none());
        Ok(view)
    }

    /// Reopens the last revoked bind. After revoke this is always a restore fault.
    pub fn restore_last(&mut self) -> Result<CharacterRuntimeView, CharacterLicenseError> {
        let snapshot = self
            .last_bind
            .clone()
            .ok_or(CharacterLicenseError::Unbound)?;
        self.bind(
            snapshot.license,
            snapshot.identity,
            snapshot.session_id,
            snapshot.epoch,
            snapshot.supported,
        )
    }

    pub fn view(&self) -> Option<CharacterRuntimeView> {
        if let Some(live) = self.live.as_ref() {
            Some(live.host.view())
        } else {
            self.last_view.clone()
        }
    }

    pub fn host(&self) -> Option<&CharacterPreviewHost> {
        self.live.as_ref().map(|live| &live.host)
    }

    fn live_host_mut(&mut self) -> Result<&mut CharacterPreviewHost, CharacterLicenseError> {
        match self.live.as_mut() {
            Some(live) => Ok(&mut live.host),
            None if self.last_view.is_some() => Err(CharacterLicenseError::Closed),
            None => Err(CharacterLicenseError::Unbound),
        }
    }

    fn record_floor(&mut self, snapshot: &BindSnapshot) {
        let key = (
            snapshot.identity.workspace_id.clone(),
            snapshot.session_id.clone(),
        );
        let floor = self.epoch_floor.entry(key).or_insert(0);
        *floor = (*floor).max(snapshot.epoch);
    }
}

fn map_host(err: CharacterHostError) -> CharacterLicenseError {
    match err {
        CharacterHostError::Identity => CharacterLicenseError::Identity,
        CharacterHostError::Session => CharacterLicenseError::Session,
        CharacterHostError::Epoch => CharacterLicenseError::Epoch,
        CharacterHostError::Closed => CharacterLicenseError::Closed,
        other => CharacterLicenseError::Host(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character_host::CANCELLED;
    use nai_contracts::character_runtime_view_bound;
    use nai_contracts::v1::CharacterRuntimeState;

    const WS: &str = "00000000-0000-4000-8000-000000000001";
    const SESSION: &str = "00000000-0000-4000-8000-000000000003";

    fn identity() -> CharacterIdentity {
        CharacterIdentity {
            workspace_id: WS.into(),
            character_id: "builtin-2d".into(),
            version: "v1".into(),
            package_sha256: "a".repeat(64),
            rights_sha256: "b".repeat(64),
        }
    }

    fn license() -> CharacterLicenseRecord {
        CharacterLicenseRecord {
            workspace_id: WS.into(),
            package_sha256: "a".repeat(64),
            rights_sha256: "b".repeat(64),
        }
    }

    fn supported() -> Vec<String> {
        vec!["idle".into(), "speak".into()]
    }

    fn bound() -> CharacterLicenseGate {
        let mut gate = CharacterLicenseGate::new();
        gate.bind(license(), identity(), SESSION.into(), 1, supported())
            .unwrap();
        gate
    }

    fn assert_preview(view: &CharacterRuntimeView) {
        assert_eq!(view.state, CharacterRuntimeState::Preview as i32);
        assert_ne!(view.state, CharacterRuntimeState::PublicActive as i32);
        assert!(character_runtime_view_bound(view, &supported()));
    }

    #[test]
    fn bind_matching_package_and_rights_opens_preview() {
        let gate = bound();
        let view = gate.view().expect("bound view");
        assert_preview(&view);
        assert!(view.action_id.is_empty());
        assert!(view.media.is_none());
        assert!(gate.host().is_some());
        let production = include_str!("character_license.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("CharacterRuntimeState::PublicActive"));
        assert!(!production.contains("PublicStandby"));
    }

    #[test]
    fn mismatched_package_or_rights_is_rejected() {
        let mut gate = CharacterLicenseGate::new();
        let mut other = license();
        other.package_sha256 = "c".repeat(64);
        assert_eq!(
            gate.bind(other, identity(), SESSION.into(), 1, supported()),
            Err(CharacterLicenseError::Rights)
        );
        let mut other_rights = license();
        other_rights.rights_sha256 = "c".repeat(64);
        assert_eq!(
            gate.bind(other_rights, identity(), SESSION.into(), 1, supported()),
            Err(CharacterLicenseError::Rights)
        );
        let mut bad = identity();
        bad.version.clear();
        assert_eq!(
            gate.bind(license(), bad, SESSION.into(), 1, supported()),
            Err(CharacterLicenseError::Identity)
        );
        assert_eq!(
            gate.bind(license(), identity(), SESSION.into(), 0, supported()),
            Err(CharacterLicenseError::Epoch)
        );
        assert!(gate.host().is_none());
        assert!(gate.view().is_none());
    }

    #[test]
    fn revoke_cancels_host_and_clears_refs() {
        let mut gate = bound();
        gate.begin_action("speak", 1_600).unwrap();
        gate.tick(800).unwrap();
        assert!(gate.view().unwrap().media.is_some());
        let view = gate.revoke().unwrap();
        assert_eq!(view.state, CharacterRuntimeState::Revoked as i32);
        assert!(view.action_id.is_empty());
        assert!(view.media.is_none());
        assert_eq!(view.reason_code, CANCELLED);
        assert!(character_runtime_view_bound(&view, &supported()));
        assert!(gate.host().is_none());
        assert_eq!(gate.tick(1_600), Err(CharacterLicenseError::Closed));
        assert_eq!(
            gate.begin_action("idle", 800),
            Err(CharacterLicenseError::Closed)
        );
        let stored = gate.view().expect("revoked view");
        assert!(stored.action_id.is_empty());
        assert!(stored.media.is_none());
        assert_eq!(gate.revoke(), Err(CharacterLicenseError::Unbound));
    }

    #[test]
    fn restore_of_revoked_rights_is_refused() {
        let mut gate = bound();
        gate.begin_action("idle", 800).unwrap();
        gate.revoke().unwrap();
        assert_eq!(gate.restore_last(), Err(CharacterLicenseError::Restore));
        assert_eq!(
            gate.bind(license(), identity(), SESSION.into(), 2, supported()),
            Err(CharacterLicenseError::Restore)
        );
        assert!(gate.host().is_none());
        assert_eq!(
            gate.view().unwrap().state,
            CharacterRuntimeState::Revoked as i32
        );
    }

    #[test]
    fn restore_of_old_epoch_is_refused() {
        let mut gate = bound();
        gate.revoke().unwrap();
        let mut fresh = identity();
        fresh.package_sha256 = "d".repeat(64);
        fresh.rights_sha256 = "e".repeat(64);
        let mut fresh_license = license();
        fresh_license.package_sha256 = fresh.package_sha256.clone();
        fresh_license.rights_sha256 = fresh.rights_sha256.clone();
        assert_eq!(
            gate.bind(
                fresh_license.clone(),
                fresh.clone(),
                SESSION.into(),
                1,
                supported()
            ),
            Err(CharacterLicenseError::Restore)
        );
        let view = gate
            .bind(
                fresh_license.clone(),
                fresh.clone(),
                SESSION.into(),
                2,
                supported(),
            )
            .unwrap();
        assert_preview(&view);
        assert!(gate.host().is_some());
        assert_eq!(
            gate.bind(fresh_license, fresh, SESSION.into(), 3, supported()),
            Err(CharacterLicenseError::AlreadyBound)
        );
        assert_eq!(
            gate.bind(license(), identity(), SESSION.into(), 3, supported()),
            Err(CharacterLicenseError::Restore)
        );
    }

    #[test]
    fn release_drops_live_bind_without_revoking_hashes() {
        let mut gate = bound();
        gate.begin_action("speak", 1_600).unwrap();
        let view = gate.release().unwrap();
        assert_eq!(view.state, CharacterRuntimeState::Revoked as i32);
        assert!(view.action_id.is_empty());
        assert!(view.media.is_none());
        assert!(gate.host().is_none());
        let rebound = gate
            .bind(license(), identity(), SESSION.into(), 1, supported())
            .unwrap();
        assert_preview(&rebound);
        assert!(gate.host().is_some());
        assert_eq!(
            gate.release().unwrap().state,
            CharacterRuntimeState::Revoked as i32
        );
        assert_eq!(gate.release(), Err(CharacterLicenseError::Unbound));
    }

    #[test]
    fn bind_is_not_a_playback_permit() {
        let mut gate = bound();
        let view = gate.begin_action("speak", 800).unwrap();
        assert_preview(&view);
        assert_ne!(view.state, CharacterRuntimeState::PublicActive as i32);
        assert_ne!(view.state, CharacterRuntimeState::PublicStandby as i32);
        let source = include_str!("character_license.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        assert!(production.contains("playback permit"));
        assert!(!production.contains("PlaybackLease"));
    }
}

//! Workbench one-click generate, private preview, selection, and public-output status.
//!
//! Generate binds an admitted license and opens `CharacterPreviewHost`. Preview
//! actions are idle and speak; any other action is `UNSUPPORTED_ACTION`. Select
//! records the workspace choice and stays Preview. Public output backend is the
//! builtin 2D renderer and the public destination is the window. Generate,
//! preview, and select are not a playback permit.

use crate::character_host::CharacterHostError;
use crate::character_license::{
    CharacterLicenseError, CharacterLicenseGate, CharacterLicenseRecord,
};
use nai_contracts::v1::{CharacterIdentity, CharacterRuntimeView};

pub const PUBLIC_BACKEND: &str = "builtin-2d";
pub const PUBLIC_DESTINATION: &str = "window";
pub const UNSUPPORTED_ACTION: &str = "UNSUPPORTED_ACTION";
pub const IDLE: &str = "idle";
pub const SPEAK: &str = "speak";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterWorkbenchError {
    Identity,
    Session,
    Epoch,
    Rights,
    AlreadyBound,
    Unbound,
    Restore,
    Closed,
    UnsupportedAction,
    Host(CharacterHostError),
}

pub struct CharacterPublicOutput {
    pub backend: &'static str,
    pub destination: &'static str,
    pub selected: Option<CharacterIdentity>,
    pub preview: Option<CharacterRuntimeView>,
}

impl CharacterPublicOutput {
    pub fn playback_permit(&self) -> bool {
        false
    }
}

/// One live preview bind, one workspace selection, builtin public backend.
pub struct CharacterWorkbench {
    gate: CharacterLicenseGate,
    selected: Option<CharacterIdentity>,
    epoch: u64,
}

impl Default for CharacterWorkbench {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterWorkbench {
    pub fn new() -> Self {
        Self {
            gate: CharacterLicenseGate::new(),
            selected: None,
            epoch: 0,
        }
    }

    pub fn generate(
        &mut self,
        license: CharacterLicenseRecord,
        identity: CharacterIdentity,
        session_id: String,
    ) -> Result<CharacterRuntimeView, CharacterWorkbenchError> {
        if let Some(current) = self.live_identity() {
            if current == identity {
                return self.gate.view().ok_or(CharacterWorkbenchError::Unbound);
            }
            self.gate.release().map_err(map_license)?;
        }
        self.epoch = self.epoch.saturating_add(1);
        self.gate
            .bind(license, identity, session_id, self.epoch, supported())
            .map_err(map_license)
    }

    pub fn preview(
        &mut self,
        action_id: &str,
        max_samples: u64,
    ) -> Result<CharacterRuntimeView, CharacterWorkbenchError> {
        self.gate
            .begin_action(action_id, max_samples)
            .map_err(map_license)
    }

    pub fn select(
        &mut self,
        license: CharacterLicenseRecord,
        identity: CharacterIdentity,
        session_id: String,
    ) -> Result<CharacterRuntimeView, CharacterWorkbenchError> {
        let view = self.generate(license, identity.clone(), session_id)?;
        self.selected = Some(identity);
        Ok(view)
    }

    pub fn revoke(&mut self) -> Result<CharacterRuntimeView, CharacterWorkbenchError> {
        let view = self.gate.revoke().map_err(map_license)?;
        self.selected = None;
        Ok(view)
    }

    pub fn public_output(&self) -> CharacterPublicOutput {
        CharacterPublicOutput {
            backend: PUBLIC_BACKEND,
            destination: PUBLIC_DESTINATION,
            selected: self.selected.clone(),
            preview: self.gate.view(),
        }
    }

    pub fn view(&self) -> Option<CharacterRuntimeView> {
        self.gate.view()
    }

    pub fn selected(&self) -> Option<&CharacterIdentity> {
        self.selected.as_ref()
    }

    fn live_identity(&self) -> Option<CharacterIdentity> {
        self.gate.host()?;
        self.gate.view().and_then(|view| view.identity)
    }
}

fn supported() -> Vec<String> {
    vec![IDLE.into(), SPEAK.into()]
}

fn map_license(err: CharacterLicenseError) -> CharacterWorkbenchError {
    match err {
        CharacterLicenseError::Host(CharacterHostError::Action) => {
            CharacterWorkbenchError::UnsupportedAction
        }
        CharacterLicenseError::Identity => CharacterWorkbenchError::Identity,
        CharacterLicenseError::Session => CharacterWorkbenchError::Session,
        CharacterLicenseError::Epoch => CharacterWorkbenchError::Epoch,
        CharacterLicenseError::Rights => CharacterWorkbenchError::Rights,
        CharacterLicenseError::AlreadyBound => CharacterWorkbenchError::AlreadyBound,
        CharacterLicenseError::Unbound => CharacterWorkbenchError::Unbound,
        CharacterLicenseError::Restore => CharacterWorkbenchError::Restore,
        CharacterLicenseError::Closed => CharacterWorkbenchError::Closed,
        CharacterLicenseError::Host(other) => CharacterWorkbenchError::Host(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn other_identity() -> CharacterIdentity {
        CharacterIdentity {
            workspace_id: WS.into(),
            character_id: "custom-2d".into(),
            version: "v1".into(),
            package_sha256: "c".repeat(64),
            rights_sha256: "d".repeat(64),
        }
    }

    fn license_of(identity: &CharacterIdentity) -> CharacterLicenseRecord {
        CharacterLicenseRecord {
            workspace_id: identity.workspace_id.clone(),
            package_sha256: identity.package_sha256.clone(),
            rights_sha256: identity.rights_sha256.clone(),
        }
    }

    fn opened() -> CharacterWorkbench {
        let mut workbench = CharacterWorkbench::new();
        workbench
            .generate(license_of(&identity()), identity(), SESSION.into())
            .unwrap();
        workbench
    }

    fn assert_preview(view: &CharacterRuntimeView) {
        assert_eq!(view.state, CharacterRuntimeState::Preview as i32);
        assert_ne!(view.state, CharacterRuntimeState::PublicActive as i32);
        assert_ne!(view.state, CharacterRuntimeState::PublicStandby as i32);
        assert!(character_runtime_view_bound(view, &supported()));
    }

    #[test]
    fn generate_opens_preview_and_public_stays_builtin() {
        let workbench = opened();
        let view = workbench.view().expect("generated view");
        assert_preview(&view);
        assert!(view.action_id.is_empty());
        let output = workbench.public_output();
        assert_eq!(output.backend, PUBLIC_BACKEND);
        assert_eq!(output.destination, PUBLIC_DESTINATION);
        assert!(!output.playback_permit());
        assert!(output.selected.is_none());
        assert_eq!(
            output.preview.as_ref().map(|item| item.state),
            Some(CharacterRuntimeState::Preview as i32)
        );
    }

    #[test]
    fn preview_idle_and_speak() {
        let mut workbench = opened();
        let idle = workbench.preview(IDLE, 800).unwrap();
        assert_preview(&idle);
        assert_eq!(idle.action_id, IDLE);
        assert!(idle.media.is_some());
        let speak = workbench.preview(SPEAK, 1_600).unwrap();
        assert_preview(&speak);
        assert_eq!(speak.action_id, SPEAK);
    }

    #[test]
    fn unsupported_action_is_named() {
        let mut workbench = opened();
        assert_eq!(
            workbench.preview("dance", 800),
            Err(CharacterWorkbenchError::UnsupportedAction)
        );
        assert_eq!(UNSUPPORTED_ACTION, "UNSUPPORTED_ACTION");
        let view = workbench.view().expect("still preview");
        assert_preview(&view);
        assert!(view.action_id.is_empty());
    }

    #[test]
    fn select_is_not_public_active_or_playback() {
        let mut workbench = CharacterWorkbench::new();
        let view = workbench
            .select(license_of(&identity()), identity(), SESSION.into())
            .unwrap();
        assert_preview(&view);
        assert_eq!(workbench.selected(), Some(&identity()));
        let output = workbench.public_output();
        assert_eq!(output.backend, PUBLIC_BACKEND);
        assert_eq!(output.destination, PUBLIC_DESTINATION);
        assert!(!output.playback_permit());
        assert_eq!(output.selected.as_ref(), Some(&identity()));
        assert_ne!(view.state, CharacterRuntimeState::PublicActive as i32);
    }

    #[test]
    fn switch_selection_releases_without_revoke() {
        let mut workbench = CharacterWorkbench::new();
        workbench
            .select(license_of(&identity()), identity(), SESSION.into())
            .unwrap();
        let switched = workbench
            .select(
                license_of(&other_identity()),
                other_identity(),
                SESSION.into(),
            )
            .unwrap();
        assert_preview(&switched);
        assert_eq!(workbench.selected(), Some(&other_identity()));
        let restored = workbench
            .generate(license_of(&identity()), identity(), SESSION.into())
            .unwrap();
        assert_preview(&restored);
        assert_eq!(workbench.selected(), Some(&other_identity()));
    }

    #[test]
    fn revoke_stops_preview_and_blocks_restore() {
        let mut workbench = opened();
        workbench
            .select(license_of(&identity()), identity(), SESSION.into())
            .unwrap();
        workbench.preview(SPEAK, 1_600).unwrap();
        let view = workbench.revoke().unwrap();
        assert_eq!(view.state, CharacterRuntimeState::Revoked as i32);
        assert!(view.action_id.is_empty());
        assert!(workbench.selected().is_none());
        assert_eq!(
            workbench.preview(IDLE, 800),
            Err(CharacterWorkbenchError::Closed)
        );
        assert_eq!(
            workbench.generate(license_of(&identity()), identity(), SESSION.into()),
            Err(CharacterWorkbenchError::Restore)
        );
        let output = workbench.public_output();
        assert_eq!(output.backend, PUBLIC_BACKEND);
        assert_eq!(output.destination, PUBLIC_DESTINATION);
        assert!(!output.playback_permit());
        assert!(output.selected.is_none());
    }

    #[test]
    fn production_source_has_no_public_active_assignment() {
        let production = include_str!("character_workbench.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("CharacterRuntimeState::PublicActive"));
        assert!(!production.contains("PublicStandby"));
        assert!(production.contains("playback permit"));
        assert_eq!(PUBLIC_BACKEND, "builtin-2d");
        assert_eq!(PUBLIC_DESTINATION, "window");
    }
}

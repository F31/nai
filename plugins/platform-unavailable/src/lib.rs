//! Explicitly unavailable platform connectors (V3.7 §9.4, §17.2, §1344).
//!
//! §9.4 puts 快手 and 视频号 in the same platform plugin directory and says:
//! "在核实官方接口前保留**显式 Unavailable 实现**，不宣称可上线". §17.2 says a
//! connector without an official interface keeps **contracts, admission notes and
//! an explicit Unavailable implementation — and no fake SDK**. This crate is that
//! implementation, for those two platforms.
//!
//! What that means concretely:
//!
//! - Each connector declares [`ConnectorAvailability::Unavailable`] with reason
//!   [`UnavailableReason::NoOfficialInterface`]. Nothing here claims the platform
//!   can go live.
//! - Each connector carries the full §13.1 compliance set and the §9.3 access-pace
//!   set as [`AdmissionNotes`] — the checks that are still **outstanding**.
//! - Each connector implements **none** of the four §27.1 duty hooks, and needs
//!   none: the seam gates every duty with [`nai_plugin_sdk::connector::require_available`]
//!   before the hook runs, so the only reachable outcome is a named failure.
//!   A fabricated success is not merely absent, it is unrepresentable.
//! - No SDK is linked. The manifest depends on the contracts crate and the plugin
//!   seam only, and `tests/unavailable.rs` reads this crate's own manifest to keep
//!   it that way.
//!
//! Nothing here loads a library, opens a socket, starts a companion process or
//! pushes a stream. The real official interfaces, the debug-build companion PoC
//! and the platform's own admission decision are external truths (§9.3) and are
//! **absent** — that absence is exactly what this crate records.
use nai_plugin_sdk::connector::{
    AdmissionNotes, ConnectorAvailability, ConnectorIdentity, PlatformConnector, UnavailableReason,
};

/// §9.4: one platform whose official interface is not yet verified, held in the
/// platform plugin directory as an explicit Unavailable implementation.
/// `connector_id` is updated when the connector itself changes; `platform_id` is
/// the platform it speaks for. Two platforms in one directory MUST have distinct
/// `platform_id`s, so platforms and connector revisions are not confusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailablePlatform {
    connector_id: &'static str,
    platform_id: &'static str,
    platform_label: &'static str,
    notes: AdmissionNotes,
}

impl UnavailablePlatform {
    /// §9.4: 快手. Official interface not verified; the platform lists the event
    /// types and no entry event, and no phone-side third-party video path was
    /// found — none of which is an authorisation to go live.
    pub fn kuaishou() -> Self {
        Self::declared("platform-kuaishou", "kuaishou", "快手")
    }

    /// §9.4: 视频号. Same directory, same standing: unverified interface,
    /// explicitly unavailable.
    pub fn wechat_channels() -> Self {
        Self::declared("platform-wechat-channels", "wechat-channels", "视频号")
    }

    /// §9.4: the platforms this directory holds, in a stable order.
    pub fn all() -> [Self; 2] {
        [Self::kuaishou(), Self::wechat_channels()]
    }

    /// The platform ids registered here, in the same order as [`Self::all`].
    pub const PLATFORM_IDS: [&'static str; 2] = ["kuaishou", "wechat-channels"];

    fn declared(
        connector_id: &'static str,
        platform_id: &'static str,
        platform_label: &'static str,
    ) -> Self {
        Self {
            connector_id,
            platform_id,
            platform_label,
            // Every §13.1 compliance check and every §9.3 access-pace item is
            // still outstanding for these platforms; that is the honest set, and
            // `AdmissionNotes::declared` rejects anything less.
            notes: AdmissionNotes::complete(),
        }
    }

    pub fn connector_id(&self) -> &'static str {
        self.connector_id
    }

    pub fn platform_id(&self) -> &'static str {
        self.platform_id
    }

    pub fn platform_label(&self) -> &'static str {
        self.platform_label
    }
}

impl PlatformConnector for UnavailablePlatform {
    fn identity(&self) -> ConnectorIdentity {
        ConnectorIdentity {
            connector_id: self.connector_id.to_owned(),
            platform_id: self.platform_id.to_owned(),
            platform_label: self.platform_label.to_owned(),
        }
    }

    fn availability(&self) -> ConnectorAvailability {
        // §9.4: before the official interface is verified there is no revision to
        // name, so `Verified` is not available to this connector at all.
        ConnectorAvailability::unavailable(UnavailableReason::NoOfficialInterface)
    }

    fn admission_notes(&self) -> AdmissionNotes {
        self.notes.clone()
    }

    // No `*_impl` overrides. That is the point: with `Unavailable` declared, the
    // seam's gate fails every duty before any hook could run, and there is no
    // code here that could answer as if the platform had responded.
}

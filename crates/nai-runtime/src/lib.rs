//! Composition root: assembles the storage owner, session, media gate, approval
//! backend, decision log and supervised media subprocesses into one governed
//! whole (V3.6 §29.4). Kernel MUST NOT depend on this crate.
//!
//! Explicit boundary: this assembly starts no loopback HTTP service, touches no
//! real device/window/audio output, produces no PlaybackAck and certifies no
//! Windows/hardware path. The `--watchdog-drill`/`--render-drill` subprocesses
//! remain engineering drills, not production playback.
mod assembly;
mod composition;
mod degradation;
mod memory;
mod observation;
mod pitch;
mod ramp;
pub mod supervisor;
mod upgrade;

pub use assembly::{
    AssemblyOutcome, AssemblyRebuild, AssemblyRefusal, LoggedAssembly, LoggedAssemblyFragment,
    LoggedCrop, LoggedSegmentBudget, LoggedSegmentUsage,
};
pub use composition::{BRINGUP_OPERATION, Composition, ControlAction, DutyEvent, ObservedFault};
pub use degradation::{
    DegradationOutcome, DegradationView, FreeFormRefusal, LoggedDegradation, Retained,
};
pub use memory::{
    COPY_SITES, CopySite, MemoryAdapter, MemoryForgetError, MemoryReadError, MemoryWriteOutcome,
    MemoryWriteRefusal, PurgeReport,
};
pub use observation::{ObservationOutcome, ObservationRefusal};
pub use pitch::{
    PitchInput, PitchRefusal, PitchRunner, RECHECK_FACES, RecheckFace, RecheckOutcome,
    ResumeRecheck,
};
pub use ramp::{RampOutcome, RampRefusal};
pub use supervisor::{
    Binaries, FramePoolArgs, RenderWorkerSpec, StandbyReason, Supervisor, SupervisorFault,
};
pub use upgrade::UpgradeActivation;

use nai_contracts::v1::{ContractError, ErrorCode};

/// Runtime assembly error. Carries a contract error code so callers can map it
/// without losing the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub code: ErrorCode,
    pub message: String,
}
impl RuntimeError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime: {} ({})", self.code.as_str_name(), self.message)
    }
}
impl std::error::Error for RuntimeError {}
pub type Result<T> = std::result::Result<T, RuntimeError>;
pub(crate) fn err(code: ErrorCode, message: impl Into<String>) -> RuntimeError {
    RuntimeError::new(code, message)
}

impl From<nai_storage::StorageError> for RuntimeError {
    fn from(error: nai_storage::StorageError) -> Self {
        Self::new(error.code, error.reason)
    }
}
/// The upgrade core reports failures as a code plus a stable token. That is
/// exactly the shape this error already carries, so the mapping is total and
/// loses nothing — in particular it does not degrade a `Conflict` (an
/// unobservable session) into an internal failure.
impl From<nai_upgrade_core::UpgradeError> for RuntimeError {
    fn from(error: nai_upgrade_core::UpgradeError) -> Self {
        Self::new(error.code, error.reason)
    }
}
impl From<ContractError> for RuntimeError {
    fn from(error: ContractError) -> Self {
        Self::new(
            ErrorCode::try_from(error.code).unwrap_or(ErrorCode::Internal),
            error.message,
        )
    }
}
impl From<nai_kernel::session::SessionError> for RuntimeError {
    fn from(error: nai_kernel::session::SessionError) -> Self {
        use nai_kernel::session::SessionError as S;
        let code = match error {
            S::Unauthorized => ErrorCode::PermissionDenied,
            S::InvalidTransition => ErrorCode::Conflict,
            S::Stale => ErrorCode::StaleEpoch,
            S::Conflict => ErrorCode::Conflict,
            S::Capacity => ErrorCode::ResourceExhausted,
            S::Expired => ErrorCode::DeadlineExceeded,
            S::Exhausted => ErrorCode::ResourceExhausted,
        };
        Self::new(code, "session operation rejected")
    }
}
impl From<nai_media_core::MediaError> for RuntimeError {
    fn from(error: nai_media_core::MediaError) -> Self {
        use nai_media_core::MediaError as M;
        let code = match error {
            M::Muted => ErrorCode::PermissionDenied,
            M::Expired => ErrorCode::DeadlineExceeded,
            M::Stale => ErrorCode::StaleEpoch,
            M::Unauthorized => ErrorCode::PermissionDenied,
            M::Capacity => ErrorCode::ResourceExhausted,
            M::InvalidAck => ErrorCode::InvalidArgument,
        };
        Self::new(code, "media gate rejected the operation")
    }
}

//! Official-event gate. Admission arguments are trusted composition-root
//! decisions, not assertions accepted from a platform event or remote client.
use crate::CallContext;
use nai_contracts::v1::{
    ContractError, ErrorCode, EventKind, EventSource, PermissionSnapshot, PermissionState,
    PlatformCapabilities, PlatformEvent,
};
use prost::Message;
use std::time::{Duration, Instant};

fn fail(code: ErrorCode, reason: &str) -> ContractError {
    ContractError {
        code: code as i32,
        category: "platform".into(),
        message: reason.into(),
        ..Default::default()
    }
}
fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

struct Connection {
    room: String,
    room_source: String,
    permissions: PermissionSnapshot,
    deadline: Instant,
}

/// One session/platform/app scope. No queues, SDK calls or output authority.
pub struct PlatformLink {
    session: String,
    epoch: u64,
    platform: String,
    app_scope: String,
    generation: u64,
    last_permission_revision: String,
    connection: Option<Connection>,
}

impl PlatformLink {
    pub fn new(
        session: &str,
        epoch: u64,
        platform: &str,
        app_scope: &str,
    ) -> Result<Self, ContractError> {
        if ![session, platform, app_scope].into_iter().all(identifier) {
            return Err(fail(ErrorCode::InvalidArgument, "invalid platform scope"));
        }
        Ok(Self {
            session: session.into(),
            epoch,
            platform: platform.into(),
            app_scope: app_scope.into(),
            generation: 0,
            last_permission_revision: String::new(),
            connection: None,
        })
    }

    /// Root must independently verify identity/session token and official room
    /// provenance, and obtain fresh permissions. Success grants no auto mode.
    pub fn admit(
        &mut self,
        capabilities: &PlatformCapabilities,
        verified_room_source: &str,
        authenticated: bool,
        context: &CallContext,
    ) -> Result<u64, ContractError> {
        context.check()?;
        if !authenticated {
            return Err(fail(
                ErrorCode::Unauthenticated,
                "platform host not authenticated",
            ));
        }
        if self.connection.is_some() {
            return Err(fail(ErrorCode::Conflict, "platform already connected"));
        }
        if capabilities.encoded_len() > 16384
            || !capabilities.connected
            || capabilities.platform != self.platform
            || capabilities.app_scope != self.app_scope
            || !identifier(&capabilities.room_id)
            || !identifier(verified_room_source)
        {
            return Err(fail(
                ErrorCode::InvalidArgument,
                "invalid platform admission",
            ));
        }
        let permissions = capabilities
            .permissions
            .as_ref()
            .ok_or_else(|| fail(ErrorCode::PermissionDenied, "missing platform permissions"))?;
        if !identifier(&permissions.revision)
            || !identifier(&permissions.authority_ref)
            || permissions.capabilities.len() > 32
            || permissions.capabilities.keys().any(|key| !identifier(key))
            || permissions.revision == self.last_permission_revision
        {
            return Err(fail(
                ErrorCode::PermissionDenied,
                "invalid or reused platform permissions",
            ));
        }
        let remaining = permissions
            .valid_until_utc_ms
            .and_then(|until| until.checked_sub(context.now_utc_ms()))
            .filter(|remaining| *remaining > 0 && *remaining <= 120000)
            .ok_or_else(|| {
                fail(
                    ErrorCode::PermissionDenied,
                    "platform permission lease invalid",
                )
            })?;
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(remaining))
            .ok_or_else(|| fail(ErrorCode::InvalidArgument, "platform lease overflow"))?;
        let generation = self.generation.checked_add(1).ok_or_else(|| {
            fail(
                ErrorCode::ResourceExhausted,
                "platform generation exhausted",
            )
        })?;
        let connection = Connection {
            room: capabilities.room_id.clone(),
            room_source: verified_room_source.into(),
            permissions: permissions.clone(),
            deadline,
        };
        context.check()?;
        self.last_permission_revision = permissions.revision.clone();
        self.generation = generation;
        self.connection = Some(connection);
        Ok(generation)
    }

    /// Trusted host disconnect/revocation path; never waits for event queues.
    pub fn disconnect(&mut self) {
        self.connection = None;
    }

    /// Call before enqueue AND after dequeue, before event-driven work. A
    /// successful check is not a durable grant across disconnect or revocation.
    pub fn validate(
        &self,
        event: &PlatformEvent,
        context: &CallContext,
    ) -> Result<(), ContractError> {
        context.check()?;
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| fail(ErrorCode::CapabilityNotReady, "platform disconnected"))?;
        let now = context.now_utc_ms();
        if Instant::now() >= connection.deadline
            || connection
                .permissions
                .valid_until_utc_ms
                .is_none_or(|until| now >= until)
        {
            return Err(fail(
                ErrorCode::PermissionDenied,
                "platform permissions expired",
            ));
        }
        if event.encoded_len() > 65536 || !identifier(&event.event_id) || event.payload_version != 1
        {
            return Err(fail(ErrorCode::InvalidArgument, "invalid platform event"));
        }
        if event.source != EventSource::SourceOfficial as i32
            || event.session_id != self.session
            || event.generation_epoch != Some(self.epoch)
            || event.connection_generation != self.generation
            || event.platform != self.platform
            || event.app_scope != self.app_scope
            || event.room_id != connection.room
            || event.room_source_ref != connection.room_source
            || event.permission_authority_ref != connection.permissions.authority_ref
        {
            return Err(fail(
                ErrorCode::PermissionDenied,
                "platform event scope mismatch",
            ));
        }
        let kind = EventKind::try_from(event.kind)
            .ok()
            .filter(|kind| *kind != EventKind::EventUnknown)
            .ok_or_else(|| fail(ErrorCode::InvalidArgument, "unknown platform event kind"))?;
        let permission = format!("platform.events@1/{}", kind.as_str_name());
        if connection.permissions.capabilities.get(&permission)
            != Some(&(PermissionState::Granted as i32))
        {
            return Err(fail(
                ErrorCode::PermissionDenied,
                "platform event permission absent",
            ));
        }
        if event.expires_at_utc_ms <= now
            || event.expires_at_utc_ms.saturating_sub(now) > 60000
            || event.expires_at_utc_ms <= event.received_at_utc_ms
            || event.received_at_utc_ms.abs_diff(now) > 5000
            || event
                .event_at_utc_ms
                .is_some_and(|time| time.abs_diff(event.received_at_utc_ms) > 5000)
        {
            return Err(fail(
                ErrorCode::DeadlineExceeded,
                "platform event time invalid",
            ));
        }
        context.check()
    }
}

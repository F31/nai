//! Plugin surface: [`Plugin`], [`PluginContext`] and [`CapabilityClient`].
//!
//! The Plugin trait is object-safe and synchronous. Async streaming upgrades
//! belong to later execution work; cancellation here is a cooperative check.
use crate::{frame, handshake};
use nai_contracts::v1::{
    CancelAck, CancelRequest, CapabilityInvocation, CapabilityResult, ContractError, ErrorCode,
    HealthResponse, PluginRegistration, RequestContext, StreamEnd,
};
use prost::Message;
use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

fn error(request_id: &str, code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        request_id: request_id.into(),
        code: code as i32,
        category: "plugin".into(),
        message: message.into(),
        ..Default::default()
    }
}

/// Trusted, host-provided context. Fields are injected by the composition root;
/// none are taken from plugin claims.
pub struct PluginContext {
    plugin_id: String,
    session_id: String,
    generation_epoch: u64,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    budget_ms: u32,
    config_json: serde_json::Value,
    credential_refs: Vec<String>,
    now_utc_ms: u64,
}

impl PluginContext {
    /// Trusted, host-provided context. Fields are injected by the composition
    /// root; none are taken from plugin claims.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        plugin_id: impl Into<String>,
        session_id: impl Into<String>,
        generation_epoch: u64,
        budget_ms: u32,
        config_json: serde_json::Value,
        credential_refs: impl IntoIterator<Item = String>,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
        now_utc_ms: u64,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            session_id: session_id.into(),
            generation_epoch,
            deadline,
            cancelled,
            budget_ms,
            config_json,
            credential_refs: credential_refs.into_iter().collect(),
            now_utc_ms,
        }
    }
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn generation_epoch(&self) -> u64 {
        self.generation_epoch
    }
    pub fn budget_ms(&self) -> u32 {
        self.budget_ms
    }
    pub fn now_utc_ms(&self) -> u64 {
        self.now_utc_ms
    }
    pub fn config(&self) -> &serde_json::Value {
        &self.config_json
    }
    pub fn credential_refs(&self) -> &[String] {
        &self.credential_refs
    }
    pub fn check(&self) -> Result<(), ContractError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(error("", ErrorCode::Cancelled, "plugin call cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(error(
                "",
                ErrorCode::DeadlineExceeded,
                "plugin call expired",
            ));
        }
        Ok(())
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

/// Object-safe plugin boundary. Implementations run in the host they declare;
/// Ready/health are host observations, not authorization. `Send + Sync` lets a
/// host share a plugin across threads; the trait stays object-safe (sync methods).
pub trait Plugin: Send + Sync {
    fn describe(&self) -> PluginRegistration;
    fn validate_config(&self, config: &serde_json::Value) -> Result<(), ContractError>;
    fn start(&mut self, ctx: &PluginContext) -> Result<(), ContractError>;
    fn health(&self, ctx: &PluginContext) -> Result<HealthResponse, ContractError>;
    fn suspend(&mut self, ctx: &PluginContext) -> Result<(), ContractError>;
    fn resume(&mut self, ctx: &PluginContext) -> Result<(), ContractError>;
    fn stop(&mut self, ctx: &PluginContext) -> Result<(), ContractError>;
    fn invoke(
        &self,
        ctx: &PluginContext,
        capability_id: &str,
        operation: &str,
        arguments: &[u8],
        sink: &mut dyn FnMut(CapabilityResult),
    ) -> Result<(), ContractError>;
}

/// A capability over a framed transport. The peer must have completed the
/// handshake first; this client only sends Envelopes after that.
pub struct CapabilityClient<R: Read, W: Write> {
    reader: R,
    writer: W,
    negotiated: (u32, u32),
}

impl<R: Read, W: Write> CapabilityClient<R, W> {
    pub fn new(reader: R, writer: W, negotiated: (u32, u32)) -> Self {
        Self {
            reader,
            writer,
            negotiated,
        }
    }
    pub fn handshake(
        reader: R,
        writer: W,
        request: &nai_contracts::v1::Handshake,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ContractError> {
        let mut reader = reader;
        let mut writer = writer;
        let ack = handshake::connect(&mut reader, &mut writer, request, deadline, cancelled)?;
        Ok(Self::new(
            reader,
            writer,
            (ack.protocol_major, ack.protocol_minor),
        ))
    }
    /// Sends a bounded CapabilityInvocation and collects results until an
    /// explicit StreamEnd, bounded by `max_chunks`.
    #[allow(clippy::too_many_arguments)]
    pub fn invoke(
        &mut self,
        context: RequestContext,
        capability_id: &str,
        operation: &str,
        arguments: Vec<u8>,
        deadline: Instant,
        cancelled: &AtomicBool,
        max_chunks: usize,
    ) -> Result<Vec<CapabilityResult>, ContractError> {
        let invocation = CapabilityInvocation {
            context: Some(context.clone()),
            capability_id: capability_id.into(),
            operation: operation.into(),
            arguments,
        };
        let envelope = nai_contracts::v1::Envelope {
            protocol_major: self.negotiated.0,
            protocol_minor: self.negotiated.1,
            message_type: "nai.v1.CapabilityInvocation".into(),
            message_id: format!("{}:cap", context.request_id),
            request_id: context.request_id,
            trace_id: context.trace_id,
            session_id: context.session_id,
            generation_epoch: context.generation_epoch,
            sent_at_utc_ms: Some(context.remaining_budget_ms as u64),
            payload: invocation.encode_to_vec(),
            remaining_budget_ms: context.remaining_budget_ms,
        };
        frame::write(&mut self.writer, &envelope.encode_to_vec())?;
        let mut results = Vec::new();
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(error(
                    "",
                    ErrorCode::Cancelled,
                    "client cancelled while awaiting results",
                ));
            }
            if Instant::now() >= deadline {
                return Err(error(
                    "",
                    ErrorCode::DeadlineExceeded,
                    "client expired while awaiting results",
                ));
            }
            let body = frame::read(&mut self.reader, deadline, cancelled)?
                .ok_or_else(|| error("", ErrorCode::Internal, "peer closed before stream end"))?;
            let response = nai_contracts::v1::Envelope::decode(body.as_slice()).map_err(|_| {
                error(
                    "",
                    ErrorCode::InvalidArgument,
                    "malformed response envelope",
                )
            })?;
            let result = CapabilityResult::decode(response.payload.as_slice()).map_err(|_| {
                error(
                    "",
                    ErrorCode::InvalidArgument,
                    "malformed capability result",
                )
            })?;
            let ended = result.end != StreamEnd::StreamData as i32;
            results.push(result);
            if ended {
                break;
            }
            if results.len() > max_chunks {
                return Err(error(
                    "",
                    ErrorCode::ResourceExhausted,
                    "capability stream limit exceeded",
                ));
            }
        }
        Ok(results)
    }
    pub fn cancel(
        &mut self,
        context: RequestContext,
        target_request_id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CancelAck, ContractError> {
        let request = CancelRequest {
            context: Some(context.clone()),
            target_request_id: target_request_id.into(),
            reason_code: "caller".into(),
        };
        let envelope = nai_contracts::v1::Envelope {
            protocol_major: self.negotiated.0,
            protocol_minor: self.negotiated.1,
            message_type: "nai.v1.CancelRequest".into(),
            message_id: format!("{}:cancel", context.request_id),
            request_id: context.request_id,
            trace_id: context.trace_id,
            session_id: context.session_id,
            generation_epoch: context.generation_epoch,
            sent_at_utc_ms: Some(context.remaining_budget_ms as u64),
            payload: request.encode_to_vec(),
            remaining_budget_ms: context.remaining_budget_ms,
        };
        frame::write(&mut self.writer, &envelope.encode_to_vec())?;
        let body = frame::read(&mut self.reader, deadline, cancelled)?
            .ok_or_else(|| error("", ErrorCode::Internal, "peer closed before cancel ack"))?;
        let response = nai_contracts::v1::Envelope::decode(body.as_slice())
            .map_err(|_| error("", ErrorCode::InvalidArgument, "malformed cancel ack"))?;
        let ack = CancelAck::decode(response.payload.as_slice()).map_err(|_| {
            error(
                "",
                ErrorCode::InvalidArgument,
                "malformed cancel ack payload",
            )
        })?;
        Ok(ack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::HostKind;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn capability_client_trait_is_object_safe() {
        // Compile-time: `dyn Plugin` must be constructible.
        let _: Option<Box<dyn Plugin>> = None;
    }

    #[test]
    fn context_rejects_cancelled_and_expired_calls() {
        let cancelled = AtomicBool::new(true);
        let ctx = PluginContext::new(
            "test",
            "session",
            7,
            100,
            serde_json::json!({}),
            [],
            Instant::now() + std::time::Duration::from_secs(5),
            Arc::new(cancelled),
            1,
        );
        assert_eq!(ctx.check().unwrap_err().code, ErrorCode::Cancelled as i32);
        let ctx = PluginContext::new(
            "test",
            "session",
            7,
            100,
            serde_json::json!({}),
            [],
            Instant::now(),
            Arc::new(AtomicBool::new(false)),
            1,
        );
        assert_eq!(
            ctx.check().unwrap_err().code,
            ErrorCode::DeadlineExceeded as i32
        );
    }

    #[test]
    fn context_exposes_trusted_host_values_only() {
        let ctx = PluginContext::new(
            "plugin-a",
            "session-1",
            9,
            42,
            serde_json::json!({"policy":"p1"}),
            ["credential:test".to_string()],
            Instant::now() + std::time::Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            123,
        );
        assert_eq!(ctx.plugin_id(), "plugin-a");
        assert_eq!(ctx.session_id(), "session-1");
        assert_eq!(ctx.generation_epoch(), 9);
        assert_eq!(ctx.budget_ms(), 42);
        assert_eq!(ctx.now_utc_ms(), 123);
        assert_eq!(ctx.credential_refs(), ["credential:test"]);
        assert_eq!(ctx.config()["policy"], "p1");
        let _ = HostKind::Inproc;
    }
}

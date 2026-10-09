//! CapabilityHost: runs a handshaken, ACL-gated capability transport against a
//! set of registered Plugin implementations. Cancellation is recorded before
//! dispatch; a matching invocation is rejected CANCELLED. Sync dispatch does
//! not preempt in-flight work (that belongs to async execution work).
use crate::{AccessPolicy, boundary::OsPeer};
use nai_contracts::v1::{
    CancelAck, CancelRequest, CapabilityInvocation, CapabilityResult, ContractError, Envelope,
    ErrorCode, ProtocolRange, StreamEnd,
};
use nai_plugin_sdk::{frame, handshake, plugin::Plugin, plugin::PluginContext};
use prost::Message;
use std::{
    collections::BTreeMap,
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
        category: "host".into(),
        message: message.into(),
        ..Default::default()
    }
}

/// Validates handshake identity material (and optionally OS peer credentials)
/// and returns the authenticated principal. Supplied by the trusted composition
/// root, not derived from plugin claims.
pub trait IdentityValidator: Send + Sync {
    fn validate(
        &self,
        handshake: &nai_contracts::v1::Handshake,
        os_peer: Option<&OsPeer>,
    ) -> Result<String, ContractError>;
}

#[derive(Debug, Clone, Copy)]
pub struct HostLimits {
    pub max_results: usize,
    pub max_plugins: usize,
}

impl Default for HostLimits {
    fn default() -> Self {
        Self {
            max_results: 4096,
            max_plugins: 256,
        }
    }
}

pub struct CapabilityHost {
    identity: Box<dyn IdentityValidator>,
    policy: AccessPolicy,
    local_ranges: Vec<ProtocolRange>,
    plugins: BTreeMap<String, Arc<dyn Plugin>>,
    cancelled_requests: BTreeMap<String, bool>,
    limits: HostLimits,
    cancelled: Arc<AtomicBool>,
}

impl CapabilityHost {
    pub fn new(
        identity: Box<dyn IdentityValidator>,
        policy: AccessPolicy,
        local_ranges: Vec<ProtocolRange>,
        limits: HostLimits,
    ) -> Self {
        Self {
            identity,
            policy,
            local_ranges,
            plugins: BTreeMap::new(),
            cancelled_requests: BTreeMap::new(),
            limits,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Registers a plugin by every capability it declares. Duplicate capability
    /// registration is rejected; a plugin declaring no capability is rejected.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<(), ContractError> {
        let registration = plugin.describe();
        if registration.plugin_id.trim().is_empty() || registration.contracts.is_empty() {
            return Err(error(
                "",
                ErrorCode::InvalidArgument,
                "plugin must declare a non-empty capability set",
            ));
        }
        if self.plugins.len() >= self.limits.max_plugins {
            return Err(error("", ErrorCode::ResourceExhausted, "host plugin limit"));
        }
        for capability in &registration.contracts {
            if self.plugins.contains_key(capability) {
                return Err(error(
                    "",
                    ErrorCode::Conflict,
                    "capability already registered",
                ));
            }
        }
        let shared: Arc<dyn Plugin> = Arc::from(plugin);
        for capability in &registration.contracts {
            self.plugins.insert(capability.clone(), shared.clone());
        }
        Ok(())
    }

    /// Records `target_request_id` as cancelled. Later invocations whose
    /// request_id matches are rejected CANCELLED before dispatch.
    pub fn record_cancellation(&mut self, target_request_id: &str) {
        self.cancelled_requests
            .insert(target_request_id.to_string(), true);
    }

    pub fn cancel_all(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Serves one connection: handshake first, then a bounded envelope loop.
    /// `os_peer` is the OS identity extracted by the caller from the transport
    /// (None when the transport does not provide it).
    pub fn serve<R: Read, W: Write>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        os_peer: Option<&OsPeer>,
        deadline: Instant,
    ) -> Result<(), ContractError> {
        let cancelled = self.cancelled.clone();
        let (principal, major, minor) = handshake::accept(
            reader,
            writer,
            &self.local_ranges,
            |handshake| self.identity.validate(handshake, os_peer),
            deadline,
            &cancelled,
        )?;
        loop {
            if self.cancelled.load(Ordering::Acquire) || Instant::now() >= deadline {
                return Err(error("", ErrorCode::Cancelled, "host stopped or expired"));
            }
            let body = match frame::read(reader, deadline, &cancelled)? {
                None => return Ok(()),
                Some(body) => body,
            };
            let envelope = Envelope::decode(body.as_slice())
                .map_err(|_| error("", ErrorCode::InvalidArgument, "malformed envelope"))?;
            if envelope.protocol_major != major || envelope.protocol_minor > minor {
                return Err(error(
                    "",
                    ErrorCode::VersionIncompatible,
                    "envelope protocol mismatch",
                ));
            }
            if self.cancelled_requests.contains_key(&envelope.request_id) {
                self.write_result(
                    writer,
                    &envelope.request_id,
                    |_| {
                        error(
                            "",
                            ErrorCode::Cancelled,
                            "request cancelled before dispatch",
                        )
                    },
                    deadline,
                    &cancelled,
                )?;
                continue;
            }
            match envelope.message_type.as_str() {
                "nai.v1.CapabilityInvocation" => {
                    let invocation = CapabilityInvocation::decode(envelope.payload.as_slice())
                        .map_err(|_| {
                            error(
                                "",
                                ErrorCode::InvalidArgument,
                                "malformed capability invocation",
                            )
                        })?;
                    self.dispatch(
                        writer, &envelope, invocation, &principal, deadline, &cancelled,
                    )?;
                }
                "nai.v1.CancelRequest" => {
                    let cancel =
                        CancelRequest::decode(envelope.payload.as_slice()).map_err(|_| {
                            error("", ErrorCode::InvalidArgument, "malformed cancel request")
                        })?;
                    self.record_cancellation(&cancel.target_request_id);
                    self.write_envelope(
                        writer,
                        &envelope.request_id,
                        "nai.v1.CancelAck",
                        CancelAck {
                            target_request_id: cancel.target_request_id,
                            cancellation_recorded: true,
                        },
                        deadline,
                        &cancelled,
                    )?;
                }
                other => {
                    return Err(error(
                        "",
                        ErrorCode::InvalidArgument,
                        &format!("unknown message type {other}"),
                    ));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch<W: Write>(
        &mut self,
        writer: &mut W,
        envelope: &Envelope,
        invocation: CapabilityInvocation,
        principal: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ContractError> {
        let context = invocation
            .context
            .as_ref()
            .ok_or_else(|| error("", ErrorCode::InvalidArgument, "invocation missing context"))?;
        if context.session_id.trim().is_empty()
            || context.request_id.trim().is_empty()
            || context.trace_id.trim().is_empty()
            || context.generation_epoch.is_none()
        {
            return Err(error(
                "",
                ErrorCode::InvalidArgument,
                "invocation requires session/request/trace/epoch",
            ));
        }
        if !self.policy.allow(
            &context.session_id,
            principal,
            &invocation.capability_id,
            &invocation.operation,
        ) {
            let denied = error(
                "",
                ErrorCode::PermissionDenied,
                "ACL denied before dispatch",
            );
            return self.write_result(
                writer,
                &envelope.request_id,
                |_| denied,
                deadline,
                cancelled,
            );
        }
        let plugin = match self.plugins.get(&invocation.capability_id) {
            Some(plugin) => plugin,
            None => {
                let absent = error(
                    "",
                    ErrorCode::CapabilityAbsent,
                    "no registered implementation",
                );
                return self.write_result(
                    writer,
                    &envelope.request_id,
                    |_| absent,
                    deadline,
                    cancelled,
                );
            }
        };
        let plugin_context = PluginContext::new(
            plugin.describe().plugin_id,
            context.session_id.clone(),
            context.generation_epoch.unwrap_or(0),
            context.remaining_budget_ms,
            serde_json::Value::Null,
            [],
            deadline,
            self.cancelled.clone(),
            envelope.sent_at_utc_ms.unwrap_or(0),
        );
        let mut results = Vec::new();
        let mut first = true;
        let invoke_result = plugin.invoke(
            &plugin_context,
            &invocation.capability_id,
            &invocation.operation,
            &invocation.arguments,
            &mut |result| {
                first = false;
                results.push(result);
            },
        );
        match invoke_result {
            Ok(()) if first => {
                let result = CapabilityResult {
                    request_id: envelope.request_id.clone(),
                    end: StreamEnd::StreamComplete as i32,
                    ..Default::default()
                };
                self.write_envelope(
                    writer,
                    &envelope.request_id,
                    "nai.v1.CapabilityResult",
                    result,
                    deadline,
                    cancelled,
                )?;
            }
            Ok(()) => {
                for result in results {
                    self.write_envelope(
                        writer,
                        &envelope.request_id,
                        "nai.v1.CapabilityResult",
                        result,
                        deadline,
                        cancelled,
                    )?;
                }
            }
            Err(err) => {
                self.write_result(writer, &envelope.request_id, |_| err, deadline, cancelled)?;
            }
        }
        Ok(())
    }

    fn write_result<W: Write>(
        &self,
        writer: &mut W,
        request_id: &str,
        error: impl FnOnce(&str) -> ContractError,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ContractError> {
        let result = CapabilityResult {
            request_id: request_id.into(),
            end: StreamEnd::StreamError as i32,
            error: Some(error(request_id)),
            ..Default::default()
        };
        self.write_envelope(
            writer,
            request_id,
            "nai.v1.CapabilityResult",
            result,
            deadline,
            cancelled,
        )
    }

    fn write_envelope<W: Write, M: Message>(
        &self,
        writer: &mut W,
        request_id: &str,
        message_type: &str,
        message: M,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ContractError> {
        if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
            return Err(error(
                "",
                ErrorCode::Cancelled,
                "host stopped before writing result",
            ));
        }
        let envelope = Envelope {
            protocol_major: 1,
            protocol_minor: 0,
            message_type: message_type.into(),
            message_id: format!("{request_id}:res"),
            request_id: request_id.into(),
            trace_id: "host".into(),
            session_id: String::new(),
            generation_epoch: None,
            sent_at_utc_ms: Some(0),
            payload: message.encode_to_vec(),
            remaining_budget_ms: 1,
        };
        frame::write(writer, &envelope.encode_to_vec())
    }
}

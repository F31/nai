//! End-to-end host/client tests over a real TCP loopback duplex: handshake,
//! ACL gating, capability dispatch, cancellation and unknown message type.
use nai_contracts::v1::{
    CapabilityResult, ContractError, ErrorCode, Handshake, HealthResponse, HostKind,
    PluginRegistration, ProtocolRange, RequestContext, StreamEnd,
};
use nai_plugin_hosts::{
    AccessPolicy, AccessRule, CapabilityHost, HostLimits, IdentityValidator, boundary::OsPeer,
};
use nai_plugin_sdk::plugin::{CapabilityClient, Plugin, PluginContext};
use nai_plugin_sdk::{frame, handshake};
use prost::Message;
use std::{
    io::Write,
    net::{Shutdown, TcpListener, TcpStream},
    sync::atomic::AtomicBool,
    time::Instant,
};

fn budget() -> Instant {
    Instant::now() + std::time::Duration::from_secs(10)
}

fn ranges() -> Vec<ProtocolRange> {
    vec![ProtocolRange {
        major: 1,
        min_minor: 0,
        max_minor: 3,
    }]
}

fn request_context() -> RequestContext {
    RequestContext {
        request_id: "req-1".into(),
        trace_id: "trace-1".into(),
        workspace_id: "workspace-1".into(),
        session_id: "session-a".into(),
        generation_epoch: Some(7),
        remaining_budget_ms: 1000,
        cancellation_id: "req-1".into(),
    }
}

struct EchoPlugin;
impl Plugin for EchoPlugin {
    fn describe(&self) -> PluginRegistration {
        PluginRegistration {
            plugin_id: "echo".into(),
            plugin_version: "0.1.0".into(),
            contracts: vec!["knowledge.retrieve@1".into()],
            supported_hosts: vec![HostKind::Inproc as i32],
            ..Default::default()
        }
    }
    fn validate_config(&self, _config: &serde_json::Value) -> Result<(), ContractError> {
        Ok(())
    }
    fn start(&mut self, _ctx: &PluginContext) -> Result<(), ContractError> {
        Ok(())
    }
    fn health(&self, _ctx: &PluginContext) -> Result<HealthResponse, ContractError> {
        Ok(HealthResponse {
            ready: true,
            revision: "health-v1".into(),
            ..Default::default()
        })
    }
    fn suspend(&mut self, _ctx: &PluginContext) -> Result<(), ContractError> {
        Ok(())
    }
    fn resume(&mut self, _ctx: &PluginContext) -> Result<(), ContractError> {
        Ok(())
    }
    fn stop(&mut self, _ctx: &PluginContext) -> Result<(), ContractError> {
        Ok(())
    }
    fn invoke(
        &self,
        _ctx: &PluginContext,
        _capability_id: &str,
        _operation: &str,
        arguments: &[u8],
        sink: &mut dyn FnMut(CapabilityResult),
    ) -> Result<(), ContractError> {
        let result = CapabilityResult {
            request_id: "req-1".into(),
            seq: 1,
            end: StreamEnd::StreamComplete as i32,
            data: arguments.to_vec(),
            ..Default::default()
        };
        sink(result);
        Ok(())
    }
}

struct FixedIdentity(String);
impl IdentityValidator for FixedIdentity {
    fn validate(
        &self,
        handshake: &Handshake,
        _os_peer: Option<&OsPeer>,
    ) -> Result<String, ContractError> {
        if handshake.plugin_id == self.0 {
            Ok("principal-test".into())
        } else {
            Err(ContractError {
                code: ErrorCode::PermissionDenied as i32,
                message: "unknown plugin".into(),
                ..Default::default()
            })
        }
    }
}

fn duplex() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let client = TcpStream::connect(address).unwrap();
    let (server, _) = listener.accept().unwrap();
    (client, server)
}

#[test]
fn end_to_end_handshake_acl_dispatch_and_cancel() {
    let (mut client, mut server) = duplex();
    let mut policy = AccessPolicy::new();
    policy.grant(
        "session-a",
        "principal-test",
        [AccessRule {
            capability_id: "knowledge.retrieve@1".into(),
            operation: "search".into(),
        }],
    );
    let mut host = CapabilityHost::new(
        Box::new(FixedIdentity("echo".into())),
        policy,
        ranges(),
        HostLimits::default(),
    );
    host.register(Box::new(EchoPlugin)).unwrap();

    let server_thread = std::thread::spawn(move || {
        let mut read_half = server.try_clone().unwrap();
        host.serve(&mut read_half, &mut server, None, budget())
            .unwrap();
    });

    // Handshake from the client side.
    let mut client_read = client.try_clone().unwrap();
    let ack = handshake::connect(
        &mut client_read,
        &mut client,
        &Handshake {
            plugin_id: "echo".into(),
            plugin_version: "0.1.0".into(),
            nonce: b"nonce-test".to_vec(),
            host: HostKind::Inproc as i32,
            protocol_ranges: ranges(),
            ..Default::default()
        },
        budget(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(ack.protocol_major, 1);

    // Build a CapabilityClient over the same duplex.
    let writer_keepalive = client.try_clone().unwrap();
    let client_read = client.try_clone().unwrap();
    let mut capability = CapabilityClient::new(
        client_read,
        client,
        (ack.protocol_major, ack.protocol_minor),
    );
    let results = capability
        .invoke(
            request_context(),
            "knowledge.retrieve@1",
            "search",
            b"query".to_vec(),
            budget(),
            &AtomicBool::new(false),
            100,
        )
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].data, b"query");
    assert_eq!(results[0].end, StreamEnd::StreamComplete as i32);

    // Cancel a request before dispatch; a later matching request is rejected.
    let mut ghost = request_context();
    ghost.request_id = "req-cancelled".into();
    let cancel = capability
        .cancel(
            ghost.clone(),
            "req-cancelled",
            budget(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert!(cancel.cancellation_recorded);
    let results = capability
        .invoke(
            ghost.clone(),
            "knowledge.retrieve@1",
            "search",
            b"late".to_vec(),
            budget(),
            &AtomicBool::new(false),
            100,
        )
        .unwrap();
    assert_eq!(results[0].end, StreamEnd::StreamError as i32);
    assert_eq!(
        results[0].error.as_ref().unwrap().code,
        ErrorCode::Cancelled as i32
    );

    // Signal EOF to the server loop, then drop the client handles.
    writer_keepalive.shutdown(Shutdown::Write).unwrap();
    drop(capability);
    server_thread.join().unwrap();
}

#[test]
fn acl_denied_before_dispatch() {
    let (mut client, mut server) = duplex();
    let policy = AccessPolicy::new(); // no grants
    let mut host = CapabilityHost::new(
        Box::new(FixedIdentity("echo".into())),
        policy,
        ranges(),
        HostLimits::default(),
    );
    host.register(Box::new(EchoPlugin)).unwrap();
    let server_thread = std::thread::spawn(move || {
        let mut read_half = server.try_clone().unwrap();
        host.serve(&mut read_half, &mut server, None, budget())
            .unwrap()
    });

    let mut client_read = client.try_clone().unwrap();
    let ack = handshake::connect(
        &mut client_read,
        &mut client,
        &Handshake {
            plugin_id: "echo".into(),
            plugin_version: "0.1.0".into(),
            nonce: b"n".to_vec(),
            host: HostKind::Inproc as i32,
            protocol_ranges: ranges(),
            ..Default::default()
        },
        budget(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let writer_keepalive = client.try_clone().unwrap();
    let client_read = client.try_clone().unwrap();
    let mut capability = CapabilityClient::new(
        client_read,
        client,
        (ack.protocol_major, ack.protocol_minor),
    );
    let results = capability
        .invoke(
            request_context(),
            "knowledge.retrieve@1",
            "search",
            b"x".to_vec(),
            budget(),
            &AtomicBool::new(false),
            100,
        )
        .unwrap();
    assert_eq!(
        results[0].error.as_ref().unwrap().code,
        ErrorCode::PermissionDenied as i32
    );
    writer_keepalive.shutdown(Shutdown::Write).unwrap();
    drop(capability);
    server_thread.join().unwrap();
}

#[test]
fn unknown_message_type_closes_connection() {
    let (mut client, mut server) = duplex();
    let mut host = CapabilityHost::new(
        Box::new(FixedIdentity("echo".into())),
        AccessPolicy::new(),
        ranges(),
        HostLimits::default(),
    );
    host.register(Box::new(EchoPlugin)).unwrap();
    let server_thread = std::thread::spawn(move || {
        let mut read_half = server.try_clone().unwrap();
        let result = host.serve(&mut read_half, &mut server, None, budget());
        assert!(result.is_err());
    });

    let mut client_read = client.try_clone().unwrap();
    let _ack = handshake::connect(
        &mut client_read,
        &mut client,
        &Handshake {
            plugin_id: "echo".into(),
            plugin_version: "0.1.0".into(),
            nonce: b"n".to_vec(),
            host: HostKind::Inproc as i32,
            protocol_ranges: ranges(),
            ..Default::default()
        },
        budget(),
        &AtomicBool::new(false),
    )
    .unwrap();
    // Send an envelope with an unknown message type.
    let envelope = nai_contracts::v1::Envelope {
        protocol_major: 1,
        protocol_minor: 3,
        message_type: "nai.v1.FutureAdminCommand".into(),
        message_id: "m".into(),
        request_id: "r".into(),
        trace_id: "t".into(),
        session_id: "session-a".into(),
        generation_epoch: Some(7),
        sent_at_utc_ms: Some(1),
        payload: vec![8, 1],
        remaining_budget_ms: 100,
    };
    client
        .write_all(&frame::encode(&envelope.encode_to_vec()).unwrap())
        .unwrap();
    server_thread.join().unwrap();
}

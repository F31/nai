//! Deterministic randomized-input testing of the IPC surface (S11).
//!
//! Three decoder entry points consume bytes an untrusted peer fully controls:
//! the 4-byte frame length prefix (`nai_plugin_sdk::frame::read`), the handshake
//! body (`handshake::accept`) and the envelope/payload decoders reached from
//! `CapabilityHost::serve`. Plus one string entry point: the plugin-declared
//! path checked by `verify_within_root`.
//!
//! Directed tests prove that *imagined* shapes are handled. They do not imply
//! "arbitrary bytes never panic, never allocate unboundedly, and always fail
//! with a named code". That is what these tests assert — totality and
//! boundedness, **not** "no vulnerabilities exist". A randomized negative
//! result is not a security proof, and this file does not claim to be one.
//!
//! This is deliberately **not** coverage-guided fuzzing: no `cargo-fuzz`, no
//! libFuzzer, no nightly, no new dependency. The PRNG is a few lines of
//! splitmix64 with a fixed seed, so any failure reproduces exactly.
//!
//! Anti-vacuity: a generator that only emits well-formed input would make every
//! assertion here pass while proving nothing. Each test therefore counts the
//! shapes and codes its corpus actually hit and fails when a count is zero.

use nai_contracts::v1::{
    CancelAck, CancelRequest, CapabilityInvocation, CapabilityResult, ContractError, Envelope,
    ErrorCode, Handshake, HandshakeAck, HealthResponse, HostKind, PluginRegistration,
    ProtocolRange, RequestContext, StreamEnd,
};
use nai_plugin_hosts::{
    AccessPolicy, AccessRule, CapabilityHost, HostLimits, IdentityValidator, boundary::OsPeer,
    verify_within_root,
};
use nai_plugin_sdk::plugin::{Plugin, PluginContext};
use nai_plugin_sdk::{frame, handshake};
use prost::Message;
use std::{
    io::Cursor,
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Deterministic PRNG (splitmix64). Fixed seeds: a failure must reproduce.
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish index in `0..bound`; never panics on `bound == 0`.
    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound.max(1)
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| (self.next_u64() >> 24) as u8).collect()
    }

    /// `bytes` with a random length below `bound`. Kept as one method so the
    /// generator never needs two simultaneous mutable borrows of the state.
    fn bytes_upto(&mut self, bound: u64) -> Vec<u8> {
        let len = self.below(bound) as usize;
        self.bytes(len)
    }

    fn text_upto(&mut self, bound: u64) -> String {
        let len = self.below(bound) as usize;
        self.text(len)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    fn text(&mut self, len: usize) -> String {
        const ALPHABET: &[char] = &[
            'a', 'b', 'c', 'X', 'Y', 'Z', '0', '1', '9', '_', '-', '.', '/', ':', '\\', ' ', '\t',
            '\u{4e2d}', '\u{6587}', '\0', '"', '\'', '{', '}', '[', ']', '#', '$', '%', '&', '*',
            '+', '<', '=', '>', '?', '@', '^', '`', '|', '~',
        ];
        (0..len)
            .map(|_| ALPHABET[self.below(ALPHABET.len() as u64) as usize])
            .collect()
    }
}

fn budget() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

fn ranges() -> Vec<ProtocolRange> {
    vec![ProtocolRange {
        major: 1,
        min_minor: 0,
        max_minor: 0,
    }]
}

/// Every error code any entry point under test may legitimately return. An
/// outcome outside this set means the failure was not a *named* one.
const NAMED_CODES: &[ErrorCode] = &[
    ErrorCode::InvalidArgument,
    ErrorCode::ResourceExhausted,
    ErrorCode::VersionIncompatible,
    ErrorCode::PermissionDenied,
    ErrorCode::CapabilityAbsent,
    ErrorCode::Cancelled,
    ErrorCode::DeadlineExceeded,
    ErrorCode::Internal,
    ErrorCode::Conflict,
];

fn assert_named(code: i32, context: &str) {
    assert!(
        NAMED_CODES.iter().any(|known| *known as i32 == code),
        "{context}: unexpected error code {code} (not one of the named outcomes)"
    );
}

// ---------------------------------------------------------------------------
// 1. Frame boundary: arbitrary bytes must be total and bounded.
// ---------------------------------------------------------------------------

/// How one `frame::read` call ended. There is no fourth possibility, and that
/// absence is the property under test.
#[derive(Debug, PartialEq, Eq)]
enum FrameOutcome {
    Body(usize),
    CleanEof,
    Named(i32),
}

fn read_frames(stream: &[u8], limit: usize) -> Vec<FrameOutcome> {
    let mut reader = Cursor::new(stream.to_vec());
    let mut out = Vec::new();
    for _ in 0..limit {
        match frame::read(&mut reader, budget(), &AtomicBool::new(false)) {
            Ok(Some(body)) => out.push(FrameOutcome::Body(body.len())),
            Ok(None) => {
                out.push(FrameOutcome::CleanEof);
                break;
            }
            Err(err) => {
                out.push(FrameOutcome::Named(err.code));
                break;
            }
        }
    }
    out
}

#[test]
fn arbitrary_bytes_at_the_frame_boundary_are_total_and_bounded() {
    let mut rng = Rng::new(0x5EED_0001);
    let mut bodies = 0usize;
    let mut clean_eof = 0usize;
    let mut oversized = 0usize;
    let mut truncated = 0usize;

    for _ in 0..4096 {
        let stream: Vec<u8> = match rng.below(6) {
            // Purely arbitrary bytes: the length prefix is whatever falls out.
            0 => rng.bytes_upto(257),
            // Largest possible declared length: rejected before allocation.
            1 => {
                let mut v = u32::MAX.to_le_bytes().to_vec();
                v.extend_from_slice(&rng.bytes_upto(64));
                v
            }
            // Legal declared length, short body (truncated frame).
            2 => {
                let declared = rng.below(frame::MAX_FRAME_BYTES as u64) as u32;
                let mut v = declared.to_le_bytes().to_vec();
                let short = rng.below(declared as u64) as usize;
                v.extend_from_slice(&rng.bytes(short));
                v
            }
            // A well-formed frame followed by trailing garbage.
            3 => {
                let mut v = frame::encode(&rng.bytes_upto(64)).unwrap();
                v.extend_from_slice(&rng.bytes_upto(64));
                v
            }
            // Truncated header (0..3 bytes), and the empty stream.
            4 => rng.bytes_upto(4),
            // Declared length exactly at the ceiling, body withheld.
            _ => {
                let mut v = (frame::MAX_FRAME_BYTES as u32).to_le_bytes().to_vec();
                v.extend_from_slice(&rng.bytes_upto(32));
                v
            }
        };

        let outcomes = read_frames(&stream, 4);
        assert!(
            !outcomes.is_empty(),
            "every stream must yield at least one outcome"
        );
        for outcome in &outcomes {
            match outcome {
                FrameOutcome::Body(len) => {
                    bodies += 1;
                    assert!(
                        *len <= frame::MAX_FRAME_BYTES,
                        "a body larger than the ceiling was handed out"
                    );
                }
                FrameOutcome::CleanEof => clean_eof += 1,
                FrameOutcome::Named(code) => {
                    assert_named(*code, "frame::read");
                    if *code == ErrorCode::ResourceExhausted as i32 {
                        oversized += 1;
                    } else if *code == ErrorCode::InvalidArgument as i32 {
                        truncated += 1;
                    }
                }
            }
        }
    }

    assert!(bodies > 0, "corpus never produced a decodable frame");
    assert!(clean_eof > 0, "corpus never produced a clean EOF");
    assert!(
        oversized > 0,
        "corpus never exercised the oversized-length rejection"
    );
    assert!(
        truncated > 0,
        "corpus never exercised a truncated frame (InvalidArgument)"
    );
}

#[test]
fn random_bodies_round_trip_byte_for_byte() {
    let mut rng = Rng::new(0x5EED_0002);
    for _ in 0..1024 {
        let len = match rng.below(4) {
            0 => 0usize,
            1 => rng.below(16) as usize,
            2 => rng.below(4096) as usize,
            _ => rng.below(frame::MAX_FRAME_BYTES as u64 + 1) as usize,
        };
        let body = rng.bytes(len);
        let encoded = frame::encode(&body).unwrap();
        assert_eq!(
            encoded.len(),
            frame::HEADER_BYTES + body.len(),
            "encoding is header + body, nothing else"
        );
        match read_frames(&encoded, 2).as_slice() {
            [FrameOutcome::Body(decoded_len), FrameOutcome::CleanEof] => {
                assert_eq!(*decoded_len, body.len());
            }
            other => panic!("well-formed frame did not round trip: {other:?}"),
        }
        // And the body must match, not merely have the right length.
        let mut reader = Cursor::new(encoded);
        assert_eq!(
            frame::read(&mut reader, budget(), &AtomicBool::new(false))
                .unwrap()
                .unwrap(),
            body
        );
    }
}

#[test]
fn oversized_body_is_refused_by_the_encoder_too() {
    let mut rng = Rng::new(0x5EED_0003);
    for _ in 0..64 {
        let len = frame::MAX_FRAME_BYTES + 1 + rng.below(4096) as usize;
        let err = frame::encode(&vec![0u8; len]).unwrap_err();
        assert_eq!(err.code, ErrorCode::ResourceExhausted as i32);
    }
}

// ---------------------------------------------------------------------------
// 2. Envelope / invocation / cancel decoding through a real `serve` loop.
// ---------------------------------------------------------------------------

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
        sink(CapabilityResult {
            request_id: "req-1".into(),
            seq: 1,
            end: StreamEnd::StreamComplete as i32,
            data: arguments.to_vec(),
            ..Default::default()
        });
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

fn host() -> CapabilityHost {
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
    host
}

fn handshake_frame(rng: &mut Rng) -> Vec<u8> {
    let request = Handshake {
        plugin_id: "echo".into(),
        plugin_version: "0.1.0".into(),
        nonce: rng.bytes(16),
        host: HostKind::Inproc as i32,
        protocol_ranges: ranges(),
        ..Default::default()
    };
    frame::encode(&request.encode_to_vec()).unwrap()
}

/// Feeds `stream` to a fresh host over an in-memory duplex. `serve` is generic
/// over `Read`/`Write`, so no socket, thread or mock transport is involved.
/// Returns the connection-level outcome plus everything the host wrote back.
fn serve_capture(stream: &[u8]) -> (Result<(), i32>, Vec<u8>) {
    let mut reader = Cursor::new(stream.to_vec());
    let mut writer: Vec<u8> = Vec::new();
    let mut host = host();
    let outcome = match host.serve(&mut reader, &mut writer, None, budget()) {
        Ok(()) => Ok(()),
        Err(err) => Err(err.code),
    };
    (outcome, writer)
}

/// One reply the host wrote back. Every variant must be well formed; the host
/// MUST NOT emit a frame that does not decode.
#[derive(Debug)]
enum Reply {
    Result(CapabilityResult),
    CancelAck,
}

/// Decodes every frame the host wrote.
///
/// Note the asymmetry that makes this necessary: an ACL denial or a missing
/// capability is reported **to the peer** as a `CapabilityResult` carrying a
/// stream error, not as a connection-level failure, so `serve` returns `Ok(())`
/// for them. Judging only `serve`'s return value would silently count those as
/// successes.
fn replies(writer: &[u8]) -> Vec<Reply> {
    let mut reader = Cursor::new(writer.to_vec());
    let mut out = Vec::new();

    // The first frame is the handshake acknowledgement, written unwrapped.
    let ack = frame::read(&mut reader, budget(), &AtomicBool::new(false))
        .expect("host wrote a malformed frame")
        .expect("host wrote no handshake acknowledgement");
    HandshakeAck::decode(ack.as_slice()).expect("host wrote a non-decodable handshake ack");

    loop {
        match frame::read(&mut reader, budget(), &AtomicBool::new(false)) {
            Ok(Some(body)) => {
                let envelope =
                    Envelope::decode(body.as_slice()).expect("host wrote a non-envelope frame");
                match envelope.message_type.as_str() {
                    "nai.v1.CapabilityResult" => out.push(Reply::Result(
                        CapabilityResult::decode(envelope.payload.as_slice())
                            .expect("host wrote a non-decodable capability result"),
                    )),
                    // An empty payload decodes as a default `CancelRequest`, so a
                    // cancellation acknowledgement is a legitimate reply here.
                    "nai.v1.CancelAck" => {
                        CancelAck::decode(envelope.payload.as_slice())
                            .expect("host wrote a non-decodable cancel ack");
                        out.push(Reply::CancelAck);
                    }
                    other => panic!("host wrote an unexpected message type: {other}"),
                }
            }
            Ok(None) => break,
            Err(err) => panic!("host wrote a malformed frame: {}", err.message),
        }
    }
    out
}

fn invocation_envelope(rng: &mut Rng) -> Envelope {
    let invocation = CapabilityInvocation {
        context: Some(RequestContext {
            request_id: format!("req-{}", rng.below(1000)),
            trace_id: format!("trace-{}", rng.below(100)),
            workspace_id: "workspace-1".into(),
            session_id: "session-a".into(),
            generation_epoch: Some(rng.next_u64()),
            remaining_budget_ms: rng.below(10_000) as u32,
            cancellation_id: "c".into(),
        }),
        capability_id: "knowledge.retrieve@1".into(),
        operation: "search".into(),
        arguments: rng.bytes_upto(64),
    };
    Envelope {
        protocol_major: 1,
        protocol_minor: 0,
        message_type: "nai.v1.CapabilityInvocation".into(),
        message_id: "m".into(),
        request_id: format!("req-{}", rng.below(1000)),
        trace_id: "t".into(),
        session_id: "session-a".into(),
        generation_epoch: Some(1),
        sent_at_utc_ms: Some(0),
        payload: invocation.encode_to_vec(),
        remaining_budget_ms: rng.below(10_000) as u32,
    }
}

#[test]
fn structured_envelopes_with_arbitrary_payloads_end_in_named_errors() {
    let mut rng = Rng::new(0x5EED_0004);
    let mut version_incompatible = 0usize;
    let mut invalid_argument = 0usize;
    let mut denied_or_absent = 0usize;
    let mut completed = 0usize;
    let mut invocation_decoded = 0usize;
    let mut cancel_acks = 0usize;

    for _ in 0..2048 {
        let envelope: Envelope = match rng.below(9) {
            // Valid protocol + invocation type, arbitrary payload bytes: this is
            // what actually reaches `CapabilityInvocation::decode`.
            0 => Envelope {
                payload: rng.bytes_upto(128),
                ..invocation_envelope(&mut rng)
            },
            // A fully valid invocation: reaches ACL, dispatch and the plugin.
            1 => invocation_envelope(&mut rng),
            2 => Envelope {
                message_type: "nai.v1.CancelRequest".into(),
                payload: CancelRequest {
                    target_request_id: rng.text_upto(16),
                    ..Default::default()
                }
                .encode_to_vec(),
                ..invocation_envelope(&mut rng)
            },
            // A non-empty unknown message type.
            3 => Envelope {
                message_type: rng.text_upto(25),
                ..invocation_envelope(&mut rng)
            },
            // Wrong major version.
            4 => Envelope {
                protocol_major: (rng.below(4) + 2) as u32,
                ..invocation_envelope(&mut rng)
            },
            // Minor above the negotiated maximum.
            5 => Envelope {
                protocol_minor: (rng.below(8) + 1) as u32,
                ..invocation_envelope(&mut rng)
            },
            // A struct with a valid header but a payload that is not an invocation.
            6 => Envelope {
                payload: rng.bytes_upto(160),
                ..invocation_envelope(&mut rng)
            },
            // A decodable invocation whose operation is not granted: this is the
            // only way to reach the ACL gate with well-formed input.
            7 => {
                let mut envelope = invocation_envelope(&mut rng);
                let mut invocation =
                    CapabilityInvocation::decode(envelope.payload.as_slice()).unwrap();
                invocation.operation = rng.text_upto(12);
                envelope.payload = invocation.encode_to_vec();
                envelope
            }
            // The all-defaults envelope (wrong protocol version, no type).
            _ => Envelope::default(),
        };

        // Occasionally send arbitrary bytes where an Envelope is expected.
        let body = if rng.below(8) == 7 {
            rng.bytes_upto(160)
        } else {
            envelope.encode_to_vec()
        };

        // Independent check of the payload decoder: a decoded envelope's payload
        // must never panic the invocation decoder, and the corpus must actually
        // reach it (anti-vacuity below).
        if let Ok(decoded) = Envelope::decode(envelope.encode_to_vec().as_slice())
            && CapabilityInvocation::decode(decoded.payload.as_slice()).is_ok()
        {
            invocation_decoded += 1;
        }

        let mut stream = handshake_frame(&mut rng);
        stream.extend_from_slice(&frame::encode(&body).unwrap());
        let (outcome, writer) = serve_capture(&stream);

        // Connection-level outcome (malformed envelope body, bad version, ...).
        if let Err(code) = outcome {
            assert_named(code, "serve");
            if code == ErrorCode::VersionIncompatible as i32 {
                version_incompatible += 1;
            } else if code == ErrorCode::InvalidArgument as i32 {
                invalid_argument += 1;
            }
        }

        // Reply-level outcome (ACL denial, missing capability, dispatched result).
        for reply in replies(&writer) {
            match reply {
                Reply::Result(result) => match result.error.as_ref() {
                    Some(err) => {
                        assert_named(err.code, "capability result");
                        if err.code == ErrorCode::PermissionDenied as i32
                            || err.code == ErrorCode::CapabilityAbsent as i32
                        {
                            denied_or_absent += 1;
                        }
                    }
                    None => {
                        assert_eq!(
                            result.end,
                            StreamEnd::StreamComplete as i32,
                            "a result without an error must end the stream cleanly"
                        );
                        completed += 1;
                    }
                },
                Reply::CancelAck => cancel_acks += 1,
            }
        }
    }

    assert!(
        invocation_decoded > 0,
        "the corpus never produced a decodable capability invocation"
    );
    assert!(
        version_incompatible > 0,
        "the corpus never produced a version mismatch"
    );
    assert!(
        invalid_argument > 0,
        "the corpus never produced an unknown or malformed message type"
    );
    assert!(
        denied_or_absent > 0,
        "the corpus never reached the ACL gate with a decodable invocation"
    );
    assert!(
        completed > 0,
        "the corpus never completed a granted invocation through the plugin"
    );
    assert!(
        cancel_acks > 0,
        "the corpus never reached the cancellation path"
    );
}

#[test]
fn a_rejected_frame_stops_the_connection_instead_of_continuing() {
    // A malformed frame followed by a perfectly valid invocation: the valid
    // frame must never be served.
    let mut rng = Rng::new(0x5EED_0005);
    let mut stream = handshake_frame(&mut rng);
    stream.extend_from_slice(&frame::encode(&[0xff; 32]).unwrap());
    stream
        .extend_from_slice(&frame::encode(&invocation_envelope(&mut rng).encode_to_vec()).unwrap());

    let total = stream.len();
    let mut reader = Cursor::new(stream);
    let mut writer: Vec<u8> = Vec::new();
    let mut host = host();
    let err = host
        .serve(&mut reader, &mut writer, None, budget())
        .unwrap_err();
    assert_named(err.code, "serve after malformed frame");
    assert!(
        (reader.position() as usize) < total,
        "the host consumed the trailing valid frame after a rejection"
    );
}

#[test]
fn arbitrary_handshakes_are_total_too() {
    let mut rng = Rng::new(0x5EED_0008);
    let mut accepted = 0usize;
    let mut refused = 0usize;

    for _ in 0..2048 {
        let body = match rng.below(4) {
            0 => rng.bytes_upto(96),
            1 => Handshake {
                plugin_id: rng.text_upto(16),
                plugin_version: rng.text_upto(8),
                nonce: rng.bytes_upto(16),
                host: rng.below(6) as i32,
                protocol_ranges: vec![ProtocolRange {
                    major: rng.below(4) as u32,
                    min_minor: rng.below(4) as u32,
                    max_minor: rng.below(4) as u32,
                }],
                ..Default::default()
            }
            .encode_to_vec(),
            2 => Handshake {
                plugin_id: "echo".into(),
                plugin_version: "0.1.0".into(),
                nonce: b"n".to_vec(),
                host: HostKind::Inproc as i32,
                protocol_ranges: ranges(),
                ..Default::default()
            }
            .encode_to_vec(),
            // A well-formed frame carrying an empty body.
            _ => Vec::new(),
        };

        let mut reader = Cursor::new(frame::encode(&body).unwrap());
        let mut writer: Vec<u8> = Vec::new();
        let outcome = handshake::accept(
            &mut reader,
            &mut writer,
            &ranges(),
            |hs| {
                if hs.plugin_id == "echo" {
                    Ok("principal-test".to_string())
                } else {
                    Err(ContractError {
                        code: ErrorCode::PermissionDenied as i32,
                        ..Default::default()
                    })
                }
            },
            budget(),
            &AtomicBool::new(false),
        );
        match outcome {
            Ok((principal, major, minor)) => {
                assert_eq!(principal, "principal-test");
                assert_eq!(major, 1);
                // The host advertises exactly major 1 / minor 0 here, so the
                // negotiated minor must land on that single point.
                assert_eq!(
                    minor, 0,
                    "negotiated minor must be inside the host's advertised range"
                );
                accepted += 1;
            }
            Err(err) => {
                assert_named(err.code, "handshake::accept");
                refused += 1;
            }
        }
    }

    assert!(
        accepted > 0,
        "the corpus never produced a successful handshake"
    );
    assert!(refused > 0, "the corpus never produced a refused handshake");
}

// ---------------------------------------------------------------------------
// 3. Cancellation and deadline take precedence over content — for any input.
// ---------------------------------------------------------------------------

#[test]
fn revocation_and_expiry_win_over_any_content() {
    let mut rng = Rng::new(0x5EED_0006);
    let mut with_bytes = 0usize;

    for _ in 0..2048 {
        let stream = match rng.below(3) {
            0 => Vec::new(),
            1 => rng.bytes_upto(64),
            _ => frame::encode(&rng.bytes_upto(256)).unwrap(),
        };
        if !stream.is_empty() {
            with_bytes += 1;
        }

        let mut reader = Cursor::new(stream.clone());
        let err = frame::read(&mut reader, budget(), &AtomicBool::new(true)).unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::Cancelled as i32,
            "cancellation must precede content interpretation"
        );

        let mut reader = Cursor::new(stream);
        let err = frame::read(&mut reader, Instant::now(), &AtomicBool::new(false)).unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::DeadlineExceeded as i32,
            "expiry must precede content interpretation"
        );
    }

    assert!(
        with_bytes > 0,
        "the precedence corpus never contained any bytes to read"
    );
}

// ---------------------------------------------------------------------------
// 4. Path containment under randomized inputs.
// ---------------------------------------------------------------------------

fn path_corpus(rng: &mut Rng, root: &Path) -> PathBuf {
    const SAFE: &[&str] = &[
        "inside.txt",
        "sub",
        "sub/inside.txt",
        ".",
        "a-b_c.d",
        "depth0/depth1/depth2",
    ];
    const HOSTILE: &[&str] = &[
        "..",
        "../outside.txt",
        "sub/../../outside.txt",
        "....//....//outside.txt",
    ];
    const JUNK: &[&str] = &[
        "",
        " ",
        "\u{4e2d}\u{6587}",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ];

    // Roughly a third of the corpus is hostile by construction, so the guard
    // below has something to count.
    let hostile = rng.below(3) == 0;
    let pieces = 1 + rng.below(4) as usize;
    let mut parts: Vec<String> = Vec::new();
    for _ in 0..pieces {
        let pool = if hostile { HOSTILE } else { SAFE };
        parts.push((*rng.pick(pool)).to_string());
        if rng.below(4) == 0 {
            parts.push((*rng.pick(JUNK)).to_string());
        }
    }
    let joined = parts.join(if rng.below(8) == 0 { "\\" } else { "/" });

    if rng.below(2) == 0 {
        // Relative to the process: no traversal component, so resolution decides.
        PathBuf::from(joined)
    } else {
        // Anchored at the root, so a `..` component is the only escape route.
        root.join(joined)
    }
}

#[test]
fn randomized_paths_never_escape_the_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("inside.txt"), b"x").unwrap();
    std::fs::write(root.join("sub").join("inside.txt"), b"x").unwrap();
    let canonical_root = root.canonicalize().unwrap();

    let mut rng = Rng::new(0x5EED_0007);
    let mut rejected = 0usize;
    let mut accepted = 0usize;
    let mut traversal_rejected = 0usize;

    for _ in 0..2048 {
        let candidate = path_corpus(&mut rng, &root);
        let has_parent = candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir));

        match verify_within_root(&root, &candidate) {
            Ok(resolved) => {
                assert!(
                    !has_parent,
                    "a path with a `..` component was accepted: {candidate:?}"
                );
                assert!(
                    resolved.starts_with(&canonical_root),
                    "an accepted path escaped the root: {resolved:?} not under {canonical_root:?} (input {candidate:?})"
                );
                accepted += 1;
            }
            Err(err) => {
                assert_named(err.code, "verify_within_root");
                if has_parent {
                    assert_eq!(
                        err.code,
                        ErrorCode::PermissionDenied as i32,
                        "traversal must be refused with PermissionDenied: {candidate:?}"
                    );
                    traversal_rejected += 1;
                }
                rejected += 1;
            }
        }
    }

    assert!(
        traversal_rejected > 0,
        "the corpus never produced a rejected traversal"
    );
    assert!(
        accepted > 0,
        "the corpus never produced an accepted in-root path"
    );
    assert!(rejected > 0, "the corpus never produced a rejection");
}

use nai_contracts::{MAX_CONTROL_BYTES, decode_envelope, negotiate, v1::*, validate_envelope};
use prost::Message;

#[test]
fn platform_epoch_presence_and_large_connection_generation_roundtrip() {
    let legacy = PlatformEvent::decode(&[][..]).unwrap();
    assert_eq!(legacy.generation_epoch, None);
    assert_eq!(legacy.connection_generation, 0);
    for epoch in [0, 9_007_199_254_740_993] {
        let event = PlatformEvent {
            generation_epoch: Some(epoch),
            connection_generation: u64::MAX,
            ..Default::default()
        };
        let mut encoded = event.encode_to_vec();
        encoded.extend([0x98, 0x06, 0x07]); // Compatible future field.
        assert_eq!(PlatformEvent::decode(encoded.as_slice()).unwrap(), event);
    }
}

fn envelope() -> Envelope {
    Envelope {
        protocol_major: 1,
        message_type: "nai.v1.ControlCommand".into(),
        message_id: "m".into(),
        request_id: "r".into(),
        trace_id: "t".into(),
        session_id: "s".into(),
        generation_epoch: Some(9_007_199_254_740_993),
        sent_at_utc_ms: Some(u64::MAX),
        payload: vec![8, 1],
        remaining_budget_ms: 200,
        ..Default::default()
    }
}

#[test]
fn large_u64_and_unknown_optional_field_survive_known_field_decode() {
    let original = envelope();
    let mut bytes = original.encode_to_vec();
    // Unknown field 99, varint 7, as sent by a newer compatible producer.
    bytes.extend([0x98, 0x06, 0x07]);
    assert_eq!(decode_envelope(&bytes).unwrap(), original);
}

#[test]
fn mandatory_fields_and_session_epoch_are_not_defaults() {
    let mut missing = envelope();
    missing.generation_epoch = None;
    assert_eq!(
        validate_envelope(&missing).unwrap_err().code,
        ErrorCode::InvalidArgument as i32
    );
    missing = envelope();
    missing.session_id.clear();
    assert!(validate_envelope(&missing).is_err());
    missing = envelope();
    missing.sent_at_utc_ms = None;
    assert!(validate_envelope(&missing).is_err());
    missing = envelope();
    missing.generation_epoch = Some(0);
    assert!(
        validate_envelope(&missing).is_ok(),
        "explicit epoch zero is representable, not an absent field"
    );
    for field in [0, 1, 2] {
        let mut missing = envelope();
        match field {
            0 => missing.message_id.clear(),
            1 => missing.request_id.clear(),
            _ => missing.trace_id.clear(),
        }
        assert!(validate_envelope(&missing).is_err());
    }
}

#[test]
fn rejects_malformed_oversized_expired_and_incompatible_messages() {
    assert!(decode_envelope(&[0x80]).is_err());
    assert_eq!(
        decode_envelope(&vec![0; MAX_CONTROL_BYTES + 1])
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted as i32
    );
    let mut bad = envelope();
    bad.payload = vec![0; MAX_CONTROL_BYTES];
    assert_eq!(
        validate_envelope(&bad).unwrap_err().code,
        ErrorCode::ResourceExhausted as i32
    );
    bad = envelope();
    bad.remaining_budget_ms = 0;
    assert_eq!(
        validate_envelope(&bad).unwrap_err().code,
        ErrorCode::DeadlineExceeded as i32
    );
    bad = envelope();
    bad.protocol_major = 2;
    assert_eq!(
        validate_envelope(&bad).unwrap_err().code,
        ErrorCode::VersionIncompatible as i32
    );
    bad = envelope();
    bad.message_type = "nai.v1.FutureAdminCommand".into();
    assert!(validate_envelope(&bad).is_err());
    bad = envelope();
    bad.protocol_minor = 10;
    assert!(
        validate_envelope(&bad).is_ok(),
        "handshake is responsible for the negotiated minor range"
    );
}

#[test]
fn version_ranges_negotiate_intersection_not_unilateral_version() {
    let range = |major, min_minor, max_minor| ProtocolRange {
        major,
        min_minor,
        max_minor,
    };
    assert_eq!(
        negotiate(&[range(1, 0, 3)], &[range(1, 2, 5)]).unwrap(),
        (1, 3)
    );
    assert!(negotiate(&[range(1, 0, 1)], &[range(2, 0, 1)]).is_err());
    assert!(negotiate(&[range(1, 0, 1)], &[range(1, 2, 3)]).is_err());
    assert!(negotiate(&[range(1, 4, 1)], &[range(1, 0, 3)]).is_err());
    assert!(negotiate(&[], &[]).is_err());
}

#[test]
fn unknown_enum_is_not_granted_or_guard_pass_and_stream_end_is_explicit() {
    assert!(PermissionState::try_from(77).is_err());
    assert!(GuardVerdict::try_from(77).is_err());
    assert_eq!(PermissionSnapshot::default().capabilities.len(), 0);
    assert_eq!(GuardResult::default().verdict, 0);
    let result = CapabilityResult {
        end: StreamEnd::StreamError as i32,
        error: Some(ContractError {
            code: ErrorCode::Cancelled as i32,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        CapabilityResult::decode(result.encode_to_vec().as_slice()).unwrap(),
        result
    );
}

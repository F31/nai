use crate::{
    MESSAGE_TYPES, SESSION_MESSAGE_TYPES,
    v1::{ContractError, Envelope, ErrorCode, ProtocolRange},
};
use prost::Message;

pub const MAX_CONTROL_BYTES: usize = 64 * 1024;

fn error(request_id: &str, code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        request_id: request_id.into(),
        code: code as i32,
        category: "contract".into(),
        message: message.into(),
        ..Default::default()
    }
}

/// Does not attest peer identity, approval, facts, lease or current epoch.
pub fn validate_envelope(envelope: &Envelope) -> Result<(), ContractError> {
    if envelope.encoded_len() > MAX_CONTROL_BYTES {
        return Err(error(
            &envelope.request_id,
            ErrorCode::ResourceExhausted,
            "control message exceeds 64KiB",
        ));
    }
    if envelope.protocol_major != 1 {
        return Err(error(
            &envelope.request_id,
            ErrorCode::VersionIncompatible,
            "unsupported protocol major",
        ));
    }
    if MESSAGE_TYPES
        .binary_search(&envelope.message_type.as_str())
        .is_err()
    {
        return Err(error(
            &envelope.request_id,
            ErrorCode::InvalidArgument,
            "unknown message type",
        ));
    }
    if [
        &envelope.message_id,
        &envelope.request_id,
        &envelope.trace_id,
    ]
    .iter()
    .any(|value| value.trim().is_empty())
        || envelope.sent_at_utc_ms.is_none()
        || envelope.payload.is_empty()
    {
        return Err(error(
            &envelope.request_id,
            ErrorCode::InvalidArgument,
            "missing required envelope field",
        ));
    }
    if envelope.remaining_budget_ms == 0 {
        return Err(error(
            &envelope.request_id,
            ErrorCode::DeadlineExceeded,
            "no remaining deadline budget",
        ));
    }
    let scoped = SESSION_MESSAGE_TYPES
        .binary_search(&envelope.message_type.as_str())
        .is_ok();
    if (scoped || !envelope.session_id.is_empty())
        && (envelope.session_id.trim().is_empty() || envelope.generation_epoch.is_none())
    {
        return Err(error(
            &envelope.request_id,
            ErrorCode::InvalidArgument,
            "session message requires session_id and epoch",
        ));
    }
    Ok(())
}

/// Bounds the encoded input before protobuf performs any payload allocations.
pub fn decode_envelope(bytes: &[u8]) -> Result<Envelope, ContractError> {
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err(error(
            "",
            ErrorCode::ResourceExhausted,
            "control message exceeds 64KiB",
        ));
    }
    let envelope = Envelope::decode(bytes).map_err(|_| {
        error(
            "",
            ErrorCode::InvalidArgument,
            "malformed protobuf envelope",
        )
    })?;
    validate_envelope(&envelope)?;
    Ok(envelope)
}

pub fn negotiate(
    local: &[ProtocolRange],
    peer: &[ProtocolRange],
) -> Result<(u32, u32), ContractError> {
    if local
        .iter()
        .chain(peer)
        .any(|range| range.major == 0 || range.min_minor > range.max_minor)
    {
        return Err(error(
            "",
            ErrorCode::InvalidArgument,
            "malformed protocol range",
        ));
    }
    local
        .iter()
        .flat_map(|a| {
            peer.iter().filter_map(move |b| {
                let minimum = a.min_minor.max(b.min_minor);
                let maximum = a.max_minor.min(b.max_minor);
                (a.major == b.major && minimum <= maximum).then_some((a.major, maximum))
            })
        })
        .max()
        .ok_or_else(|| {
            error(
                "",
                ErrorCode::VersionIncompatible,
                "no common protocol version",
            )
        })
}

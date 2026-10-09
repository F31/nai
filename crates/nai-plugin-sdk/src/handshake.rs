//! Handshake negotiation and validation for nai plugin IPC.
use crate::frame;
use nai_contracts::{
    negotiate,
    v1::{ContractError, ErrorCode, Handshake, HandshakeAck, ProtocolRange},
};
use prost::Message;
use std::{
    io::{Read, Write},
    sync::atomic::AtomicBool,
    time::Instant,
};

fn error(request_id: &str, code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        request_id: request_id.into(),
        code: code as i32,
        category: "handshake".into(),
        message: message.into(),
        ..Default::default()
    }
}

/// Server side of the handshake. Reads exactly one Handshake frame, validates
/// identity material via `verify`, negotiates the protocol version and writes a
/// HandshakeAck that echoes the peer nonce. Returns the negotiated (major, minor).
pub fn accept<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    local: &[ProtocolRange],
    verify: impl FnOnce(&Handshake) -> Result<String, ContractError>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(String, u32, u32), ContractError> {
    let body = frame::read(reader, deadline, cancelled)?.ok_or_else(|| {
        error(
            "",
            ErrorCode::InvalidArgument,
            "connection closed before handshake",
        )
    })?;
    let handshake = Handshake::decode(body.as_slice())
        .map_err(|_| error("", ErrorCode::InvalidArgument, "malformed handshake"))?;
    if handshake.plugin_id.trim().is_empty()
        || handshake.plugin_version.trim().is_empty()
        || handshake.nonce.is_empty()
        || handshake.host as u32 == 0
    {
        return Err(error(
            "",
            ErrorCode::InvalidArgument,
            "handshake requires id/version/nonce/host",
        ));
    }
    if handshake.protocol_ranges.is_empty() {
        return Err(error(
            "",
            ErrorCode::InvalidArgument,
            "handshake has no protocol ranges",
        ));
    }
    let principal = verify(&handshake)?;
    if principal.trim().is_empty() {
        return Err(error(
            "",
            ErrorCode::PermissionDenied,
            "handshake identity rejected",
        ));
    }
    let (major, minor) = negotiate(local, &handshake.protocol_ranges)?;
    let ack = HandshakeAck {
        protocol_major: major,
        protocol_minor: minor,
        peer_nonce: handshake.nonce,
        ..Default::default()
    };
    frame::write(writer, &ack.encode_to_vec())?;
    Ok((principal, major, minor))
}

/// Client side of the handshake. Sends a Handshake, reads a HandshakeAck and
/// verifies the echoed nonce plus the negotiated version intersection.
pub fn connect<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    request: &Handshake,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<HandshakeAck, ContractError> {
    frame::write(writer, &request.encode_to_vec())?;
    let body = frame::read(reader, deadline, cancelled)?.ok_or_else(|| {
        error(
            "",
            ErrorCode::InvalidArgument,
            "connection closed during handshake",
        )
    })?;
    let ack = HandshakeAck::decode(body.as_slice())
        .map_err(|_| error("", ErrorCode::InvalidArgument, "malformed handshake ack"))?;
    if let Some(peer_error) = ack.error.as_ref() {
        return Err(peer_error.clone());
    }
    if ack.peer_nonce != request.nonce {
        return Err(error(
            "",
            ErrorCode::PermissionDenied,
            "handshake nonce mismatch",
        ));
    }
    let negotiated = [ProtocolRange {
        major: ack.protocol_major,
        min_minor: ack.protocol_minor,
        max_minor: ack.protocol_minor,
    }];
    negotiate(&request.protocol_ranges, &negotiated).map_err(|_| {
        error(
            "",
            ErrorCode::VersionIncompatible,
            "no common protocol version",
        )
    })?;
    Ok(ack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::HostKind;
    use std::sync::atomic::AtomicBool;

    fn ranges() -> Vec<ProtocolRange> {
        vec![ProtocolRange {
            major: 1,
            min_minor: 0,
            max_minor: 3,
        }]
    }
    fn request() -> Handshake {
        Handshake {
            plugin_id: "test-guard".into(),
            plugin_version: "0.1.0".into(),
            nonce: b"test-nonce".to_vec(),
            host: HostKind::Inproc as i32,
            protocol_ranges: ranges(),
            ..Default::default()
        }
    }
    fn budget() -> Instant {
        Instant::now() + std::time::Duration::from_secs(5)
    }

    #[test]
    fn successful_handshake_echoes_nonce_and_negotiates() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut client_stream = std::net::TcpStream::connect(address).unwrap();
        let (mut server_stream, _) = listener.accept().unwrap();
        let client = std::thread::spawn(move || {
            let mut read_half = client_stream.try_clone().unwrap();
            handshake_client(&mut read_half, &mut client_stream)
        });
        let (server_major, server_minor) = {
            let mut read_half = server_stream.try_clone().unwrap();
            let (_, maj, min) = accept(
                &mut read_half,
                &mut server_stream,
                &ranges(),
                |hs| {
                    assert_eq!(hs.plugin_id, "test-guard");
                    Ok("principal-test".to_string())
                },
                budget(),
                &AtomicBool::new(false),
            )
            .unwrap();
            (maj, min)
        };
        let client_ack = client.join().unwrap();
        assert_eq!((server_major, server_minor), (1, 3));
        assert_eq!(client_ack.protocol_major, 1);
        assert_eq!(client_ack.peer_nonce, b"test-nonce");
    }

    fn handshake_client<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> HandshakeAck {
        connect(
            reader,
            writer,
            &request(),
            budget(),
            &AtomicBool::new(false),
        )
        .unwrap()
    }

    #[test]
    fn identity_rejection_and_version_mismatch_fail() {
        let mut sink = Vec::new();
        // server rejects via verify
        let mut frame_bytes = frame::encode(&request().encode_to_vec()).unwrap();
        let mut reader = std::io::Cursor::new(std::mem::take(&mut frame_bytes));
        assert_eq!(
            accept(
                &mut reader,
                &mut std::io::Cursor::new(&mut sink),
                &ranges(),
                |_| {
                    Err(ContractError {
                        code: ErrorCode::PermissionDenied as i32,
                        ..Default::default()
                    })
                },
                budget(),
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .code,
            ErrorCode::PermissionDenied as i32
        );
        // no common version
        let mut bad = request();
        bad.protocol_ranges = vec![ProtocolRange {
            major: 2,
            min_minor: 0,
            max_minor: 1,
        }];
        let mut reader = std::io::Cursor::new(frame::encode(&bad.encode_to_vec()).unwrap());
        assert_eq!(
            accept(
                &mut reader,
                &mut std::io::Cursor::new(&mut sink),
                &ranges(),
                |_| Ok("p".into()),
                budget(),
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .code,
            ErrorCode::VersionIncompatible as i32
        );
    }
}

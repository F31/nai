//! Bounded binary framing for nai IPC.
//!
//! A frame is a 4-byte little-endian length prefix followed by the body. The
//! length is validated before any body allocation and never exceeds
//! [`MAX_FRAME_BYTES`]. Read loops honor a deadline and a shared cancellation
//! flag; EOF is only a clean end between frames.
use nai_contracts::v1::{ContractError, ErrorCode};
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const HEADER_BYTES: usize = 4;

fn error(request_id: &str, code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        request_id: request_id.into(),
        code: code as i32,
        category: "frame".into(),
        message: message.into(),
        ..Default::default()
    }
}

/// Encodes `body` with a 4-byte little-endian length prefix.
pub fn encode(body: &[u8]) -> Result<Vec<u8>, ContractError> {
    if body.len() > MAX_FRAME_BYTES {
        return Err(error(
            "",
            ErrorCode::ResourceExhausted,
            "frame body exceeds 64KiB",
        ));
    }
    let mut out = Vec::with_capacity(HEADER_BYTES + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

/// Reads the next frame from `reader`.
///
/// Returns `Ok(None)` on a clean EOF at a frame boundary. Returns an error on
/// partial header, truncated body, oversized length, cancellation, deadline
/// expiry or I/O failure. Handles coalesced frames (one frame per call) and
/// partial reads by looping over the underlying reader.
pub fn read<R: Read>(
    reader: &mut R,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<Vec<u8>>, ContractError> {
    let mut header = [0u8; HEADER_BYTES];
    if read_exact(reader, &mut header, deadline, cancelled)?.is_none() {
        return Ok(None);
    }
    let length = u32::from_le_bytes(header) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(error(
            "",
            ErrorCode::ResourceExhausted,
            "oversized frame length rejected before allocation",
        ));
    }
    let mut body = vec![0u8; length];
    read_exact(reader, &mut body, deadline, cancelled)?
        .ok_or_else(|| error("", ErrorCode::InvalidArgument, "truncated frame body"))?;
    Ok(Some(body))
}

/// Writes a complete frame (header + body) to `writer`.
pub fn write<W: Write>(writer: &mut W, body: &[u8]) -> Result<(), ContractError> {
    let frame = encode(body)?;
    writer
        .write_all(&frame)
        .map_err(|_| error("", ErrorCode::Internal, "frame write failed"))
}

fn read_exact<R: Read>(
    reader: &mut R,
    buffer: &mut [u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<()>, ContractError> {
    let mut filled = 0;
    while filled < buffer.len() {
        if cancelled.load(Ordering::Acquire) {
            return Err(error("", ErrorCode::Cancelled, "frame read cancelled"));
        }
        if Instant::now() >= deadline {
            return Err(error("", ErrorCode::DeadlineExceeded, "frame read expired"));
        }
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => {
                if filled == 0 {
                    return Ok(None);
                }
                return Err(error("", ErrorCode::InvalidArgument, "partial frame EOF"));
            }
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(error("", ErrorCode::Internal, "frame read failed")),
        }
    }
    Ok(Some(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn budget() -> Instant {
        Instant::now() + std::time::Duration::from_secs(5)
    }

    #[test]
    fn roundtrip_and_oversized_body_fail() {
        let body = b"hello";
        let frame = encode(body).unwrap();
        assert_eq!(&frame[..4], &5u32.to_le_bytes());
        let mut cursor = Cursor::new(frame);
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false))
                .unwrap()
                .unwrap(),
            body
        );
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false)).unwrap(),
            None
        );
        assert!(encode(&vec![0u8; MAX_FRAME_BYTES + 1]).is_err());
    }

    #[test]
    fn malicious_and_oversized_length_rejected_before_body() {
        let mut bytes = (MAX_FRAME_BYTES as u32 + 1).to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0u8; 8]);
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false))
                .unwrap_err()
                .code,
            ErrorCode::ResourceExhausted as i32
        );
        let mut truncated_header = vec![0u8; 2];
        let mut cursor = Cursor::new(truncated_header.clone());
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false))
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument as i32
        );
        truncated_header.clear();
        let mut cursor = Cursor::new(truncated_header);
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false)).unwrap(),
            None
        );
    }

    #[test]
    fn truncated_body_and_partial_header_eof_fail() {
        let body = b"partial-body";
        let mut frame = encode(body).unwrap();
        frame.truncate(6);
        let mut cursor = Cursor::new(frame);
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false))
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument as i32
        );
    }

    #[test]
    fn coalesced_frames_are_consumed_one_at_a_time() {
        let first = encode(b"first").unwrap();
        let second = encode(b"second").unwrap();
        let mut combined = first.clone();
        combined.extend_from_slice(&second);
        let mut cursor = Cursor::new(combined);
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false))
                .unwrap()
                .unwrap(),
            b"first"
        );
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false))
                .unwrap()
                .unwrap(),
            b"second"
        );
        assert_eq!(
            read(&mut cursor, budget(), &AtomicBool::new(false)).unwrap(),
            None
        );
    }

    #[test]
    fn partial_reads_reassemble_a_frame() {
        struct Chunked(Cursor<Vec<u8>>);
        impl Read for Chunked {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if buffer.is_empty() || self.0.position() >= self.0.get_ref().len() as u64 {
                    return Ok(0);
                }
                buffer[0] = self.0.get_ref()[self.0.position() as usize];
                self.0.set_position(self.0.position() + 1);
                Ok(1)
            }
        }
        let frame = encode(&vec![0xabu8; 300]).unwrap();
        let mut reader = Chunked(Cursor::new(frame));
        assert_eq!(
            read(&mut reader, budget(), &AtomicBool::new(false))
                .unwrap()
                .unwrap()
                .len(),
            300
        );
    }

    #[test]
    fn deadline_and_cancellation_abort_read() {
        let mut bytes = (MAX_FRAME_BYTES as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&vec![0u8; MAX_FRAME_BYTES]);
        let mut cursor = Cursor::new(bytes);
        let cancelled = AtomicBool::new(true);
        assert_eq!(
            read(&mut cursor, budget(), &cancelled).unwrap_err().code,
            ErrorCode::Cancelled as i32
        );
        let mut cursor = Cursor::new(vec![0u8; 8]);
        assert_eq!(
            read(&mut cursor, Instant::now(), &AtomicBool::new(false))
                .unwrap_err()
                .code,
            ErrorCode::DeadlineExceeded as i32
        );
    }
}

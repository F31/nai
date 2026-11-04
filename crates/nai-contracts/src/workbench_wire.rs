//! Bounded little-endian framing only. Authentication belongs to the runtime.
use crate::workbench::{WorkbenchError as Error, WorkbenchLimit, WorkbenchProtocolMajor};
use prost::Message;
use std::io::{Read, Write};
pub const VERSION: u32 = WorkbenchProtocolMajor::Current as u32;
pub const FRAME_LIMIT: usize = WorkbenchLimit::FrameBytes as usize;
pub const TTL_MS: u32 = WorkbenchLimit::TtlMs as u32;
pub fn read<M: Message + Default>(input: &mut impl Read) -> Result<M, Error> {
    let mut prefix = [0; 4];
    input
        .read_exact(&mut prefix)
        .map_err(|_| Error::Truncated)?;
    let n = u32::from_le_bytes(prefix) as usize;
    if n == 0 || n > FRAME_LIMIT {
        return Err(Error::FrameLimit);
    }
    let mut bytes = vec![0; n];
    input.read_exact(&mut bytes).map_err(|_| Error::Truncated)?;
    M::decode(bytes.as_slice()).map_err(|_| Error::ProtocolError)
}
pub fn write<M: Message>(output: &mut impl Write, message: &M) -> Result<(), Error> {
    let bytes = message.encode_to_vec();
    if bytes.is_empty() || bytes.len() > FRAME_LIMIT {
        return Err(Error::FrameLimit);
    }
    output
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|_| output.write_all(&bytes))
        .and_then(|_| output.flush())
        .map_err(|_| Error::IoFailed)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::WorkbenchRequest;
    #[test]
    fn rejects_empty_huge_and_truncated_before_allocating() {
        for n in [0u32, 65537, u32::MAX] {
            assert_eq!(
                read::<WorkbenchRequest>(&mut n.to_le_bytes().as_slice()),
                Err(Error::FrameLimit)
            );
        }
        assert_eq!(
            read::<WorkbenchRequest>(&mut [3u8, 0, 0].as_slice()),
            Err(Error::Truncated)
        );
        assert_eq!(
            read::<WorkbenchRequest>(&mut [3u8, 0, 0, 0, 1].as_slice()),
            Err(Error::Truncated)
        );
    }
    #[test]
    fn little_endian_roundtrip_and_invalid_proto() {
        let r = WorkbenchRequest {
            protocol_major: VERSION,
            seq: 1,
            ..Default::default()
        };
        let mut b = vec![];
        write(&mut b, &r).unwrap();
        assert_eq!(
            u32::from_le_bytes(b[..4].try_into().unwrap()) as usize,
            b.len() - 4
        );
        assert_eq!(read::<WorkbenchRequest>(&mut b.as_slice()).unwrap(), r);
        assert_eq!(
            read::<WorkbenchRequest>(&mut [1u8, 0, 0, 0, 255].as_slice()),
            Err(Error::ProtocolError)
        );
    }
}

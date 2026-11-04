//! Inherited private control pipe only. PCM belongs to a separate media handle/file.
use prost::Message;
use std::io::{Read, Write};
pub fn read<T: Message + Default>(input: &mut impl Read) -> std::io::Result<T> {
    let mut h = [0; 4];
    input.read_exact(&mut h)?;
    let n = u32::from_le_bytes(h) as usize;
    if n == 0 || n > 4096 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut b = vec![0; n];
    input.read_exact(&mut b)?;
    T::decode(b.as_slice()).map_err(|_| std::io::ErrorKind::InvalidData.into())
}
pub fn write<T: Message>(out: &mut impl Write, value: &T) -> std::io::Result<()> {
    let b = value.encode_to_vec();
    if b.is_empty() || b.len() > 4096 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    out.write_all(&(b.len() as u32).to_le_bytes())?;
    out.write_all(&b)?;
    out.flush()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::{AuditionControl, audition_control};
    #[test]
    fn bounded_frames_reject_oversize_and_truncation() {
        let c = AuditionControl {
            operation: Some(audition_control::Operation::Stop(true)),
        };
        let mut b = Vec::new();
        write(&mut b, &c).unwrap();
        assert_eq!(read::<AuditionControl>(&mut b.as_slice()).unwrap(), c);
        b.pop();
        assert!(read::<AuditionControl>(&mut b.as_slice()).is_err());
        for n in [0u32, 4097, u32::MAX] {
            assert!(read::<AuditionControl>(&mut n.to_le_bytes().as_slice()).is_err());
        }
    }
}

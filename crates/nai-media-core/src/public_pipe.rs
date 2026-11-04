//! One-shot preparation consumer, never playback authority. Raw PCM has its own read-only pipe.
use crate::{
    public_avatar::BuiltinAvatar,
    public_clip::{FRAME_SAMPLES, PublicClipRenderer, RENDERER_ID, expected_frames},
};
use nai_contracts::{
    prepared_media_hash,
    workbench::{PreparedMediaDescriptor, PreparedMediaReceipt, WorkbenchError as Error},
};
use std::io::Read;
use zeroize::Zeroizing;
type Result<T> = std::result::Result<T, Error>;
fn uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
/// Caller owns process deadline/kill for blocking OS reads; cancellation is checked between bounded chunks.
pub struct Consumed {
    pub receipt: PreparedMediaReceipt,
    pub samples: Zeroizing<Vec<i16>>,
}
pub fn consume(
    input: &mut impl Read,
    d: &PreparedMediaDescriptor,
    instance: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<PreparedMediaReceipt> {
    read_pcm(input, d, instance, check).map(|c| c.receipt)
}
pub fn read_pcm(
    input: &mut impl Read,
    d: &PreparedMediaDescriptor,
    instance: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<Consumed> {
    check()?;
    let a = d.audio.as_ref().ok_or(Error::IntegrityFailed)?;
    if d.protocol_major != 1
        || !uuid(&d.workspace_id)
        || !uuid(&d.task_id)
        || !uuid(instance)
        || a.artifact_id != d.task_id
        || d.task_sha256.len() != 64
        || !d.task_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || d.renderer_id != RENDERER_ID
        || d.samples_per_frame != FRAME_SAMPLES as u32
        || d.pcm_format != "s16le-mono-24000"
        || d.pixel_format != "rgb32le"
        || d.expected_frames != expected_frames(a.sample_count)?
        || a.sample_rate != 24000
        || a.channels != 1
    {
        return Err(Error::IntegrityFailed);
    }
    let avatar = BuiltinAvatar::new(check)?;
    if d.visual.as_ref() != Some(&avatar.observe(check)?) {
        return Err(Error::IntegrityFailed);
    }
    let mut pcm = Zeroizing::new(Vec::new());
    pcm.try_reserve_exact(a.sample_count as usize)
        .map_err(|_| Error::ResourceExhausted)?;
    let mut buf = Zeroizing::new([0u8; 8192]);
    let mut left = a.sample_count * 2;
    while left > 0 {
        check()?;
        let n = (left as usize).min(buf.len());
        input
            .read_exact(&mut buf[..n])
            .map_err(|_| Error::IntegrityFailed)?;
        for b in buf[..n].chunks_exact(2) {
            pcm.push(i16::from_le_bytes([b[0], b[1]]));
        }
        left -= n as u64;
    }
    check()?;
    if input
        .read(&mut buf[..1])
        .map_err(|_| Error::IntegrityFailed)?
        != 0
    {
        return Err(Error::IntegrityFailed);
    }
    let mut renderer = PublicClipRenderer::new(&pcm, a, &avatar, check)?;
    let first = renderer.frame_at(0, check)?.ok_or(Error::IntegrityFailed)?;
    if first.info.pixels_sha256 != d.first_frame_sha256 {
        return Err(Error::IntegrityFailed);
    }
    let tail = renderer
        .frame_at((d.expected_frames - 1) * FRAME_SAMPLES, check)?
        .ok_or(Error::IntegrityFailed)?;
    check()?;
    Ok(Consumed {
        receipt: PreparedMediaReceipt {
            protocol_major: 1,
            descriptor_sha256: prepared_media_hash(d),
            consumer_instance_id: instance.into(),
            pcm_sha256: a.pcm_sha256.clone(),
            sample_count: a.sample_count,
            expected_frames: d.expected_frames,
            first_frame_sha256: first.info.pixels_sha256,
            tail_frame: Some(tail.info),
        },
        samples: pcm,
    })
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use nai_contracts::workbench::PublicMediaAudio;
    pub(crate) fn fixture() -> (PreparedMediaDescriptor, Vec<u8>) {
        let pcm = vec![12000i16; 1601];
        let avatar = BuiltinAvatar::new(&|| Ok(())).unwrap();
        let a = PublicMediaAudio {
            artifact_id: "00000000-0000-4000-8000-000000000002".into(),
            pcm_sha256: crate::public_clip::pcm_hash(&pcm, &|| Ok(())).unwrap(),
            sample_count: 1601,
            sample_rate: 24000,
            channels: 1,
        };
        let mut r = PublicClipRenderer::new(&pcm, &a, &avatar, &|| Ok(())).unwrap();
        let d = PreparedMediaDescriptor {
            protocol_major: 1,
            workspace_id: "00000000-0000-4000-8000-000000000001".into(),
            task_id: a.artifact_id.clone(),
            task_sha256: "a".repeat(64),
            audio: Some(a.clone()),
            visual: Some(avatar.observe(&|| Ok(())).unwrap()),
            expected_frames: 3,
            samples_per_frame: 800,
            first_frame_sha256: r
                .frame_at(0, &|| Ok(()))
                .unwrap()
                .unwrap()
                .info
                .pixels_sha256,
            pcm_format: "s16le-mono-24000".into(),
            pixel_format: "rgb32le".into(),
            renderer_id: RENDERER_ID.into(),
        };
        (d, pcm.iter().flat_map(|n| n.to_le_bytes()).collect())
    }
    #[test]
    fn raw_pipe_binds_actual_data_and_exact_tail() {
        let (d, b) = fixture();
        let v = consume(&mut b.as_slice(), &d, &d.workspace_id, &|| Ok(())).unwrap();
        assert_eq!(v.descriptor_sha256, prepared_media_hash(&d));
        let tail = v.tail_frame.unwrap();
        assert_eq!(
            (tail.frame_index, tail.sample_start, tail.sample_count),
            (3, 1600, 1)
        );
    }
    #[test]
    fn corrupt_truncated_extra_and_forged_metadata_are_rejected() {
        for case in 0..7 {
            let (mut d, mut b) = fixture();
            match case {
                0 => b[0] ^= 1,
                1 => {
                    b.pop();
                }
                2 => b.push(0),
                3 => d.renderer_id = "forged".into(),
                4 => d.first_frame_sha256 = "0".repeat(64),
                5 => d.visual.as_mut().unwrap().license_sha256 = "0".repeat(64),
                _ => d.expected_frames = 4,
            };
            assert!(consume(&mut b.as_slice(), &d, &d.workspace_id, &|| Ok(())).is_err());
        }
    }
    #[test]
    fn cancellation_precedes_pipe_read_and_metadata_is_bounded() {
        struct Forbidden;
        impl Read for Forbidden {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("read after cancellation")
            }
        }
        let (d, _) = fixture();
        assert_eq!(
            consume(&mut Forbidden, &d, &d.workspace_id, &|| Err(
                Error::Cancelled
            )),
            Err(Error::Cancelled)
        );
        let encoded = nai_contracts::prepared_descriptor_env(&d).unwrap();
        assert_eq!(
            nai_contracts::prepared_descriptor_from_env(&encoded).unwrap(),
            d
        );
        for bad in ["f", "gg", &"a".repeat(4098)] {
            assert!(nai_contracts::prepared_descriptor_from_env(bad).is_err());
        }
    }
}

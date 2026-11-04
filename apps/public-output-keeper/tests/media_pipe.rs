//! Native process data-plane evidence, no playback.
use nai_contracts::{audition_wire as wire, workbench::*};
use nai_media_core::{
    public_avatar::BuiltinAvatar,
    public_clip::{PublicClipRenderer, pcm_hash},
};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn fixture() -> (PreparedMediaDescriptor, Vec<u8>) {
    let pcm = vec![12000i16; 1601];
    let avatar = BuiltinAvatar::new(&|| Ok(())).unwrap();
    let a = PublicMediaAudio {
        artifact_id: "00000000-0000-4000-8000-000000000002".into(),
        pcm_sha256: pcm_hash(&pcm, &|| Ok(())).unwrap(),
        sample_count: 1601,
        sample_rate: 24000,
        channels: 1,
    };
    let mut r = PublicClipRenderer::new(&pcm, &a, &avatar, &|| Ok(())).unwrap();
    (
        PreparedMediaDescriptor {
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
            renderer_id: "rms-sample-v1".into(),
        },
        pcm.iter().flat_map(|n| n.to_le_bytes()).collect(),
    )
}
struct Reap(std::process::Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn spawn(d: &PreparedMediaDescriptor) -> Reap {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nai-public-output-keeper"));
    c.arg("--verify-media")
        .env(
            "NAI_PREPARED_DESCRIPTOR",
            nai_contracts::prepared_descriptor_env(d).unwrap(),
        )
        .env("NAI_PREPARED_CONSUMER", &d.workspace_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    Reap(c.spawn().expect("actual raw-pipe child"))
}
fn wait(c: &mut Reap) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(s) = c.0.try_wait().unwrap() {
            return s.success();
        }
        assert!(Instant::now() < until, "child deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn actual_consumer_reads_raw_pcm_and_returns_bound_tail() {
    let (d, b) = fixture();
    let mut c = spawn(&d);
    let mut i = c.0.stdin.take().unwrap();
    i.write_all(&b).unwrap();
    drop(i);
    assert!(wait(&mut c));
    let r = wire::read::<PreparedMediaReceipt>(&mut c.0.stdout.take().unwrap()).unwrap();
    assert_eq!(r.descriptor_sha256, nai_contracts::prepared_media_hash(&d));
    assert_eq!(r.consumer_instance_id, d.workspace_id);
    let t = r.tail_frame.unwrap();
    assert_eq!(
        (t.frame_index, t.sample_start, t.sample_count),
        (3, 1600, 1)
    );
}
#[test]
fn actual_consumer_rejects_corrupt_short_extra_and_license() {
    for case in 0..4 {
        let (mut d, mut b) = fixture();
        match case {
            0 => b[0] ^= 1,
            1 => {
                b.pop();
            }
            2 => b.push(0),
            _ => d.visual.as_mut().unwrap().license_sha256 = "0".repeat(64),
        }
        let mut c = spawn(&d);
        let mut i = c.0.stdin.take().unwrap();
        let _ = i.write_all(&b);
        drop(i);
        assert!(!wait(&mut c));
        let mut out = Vec::new();
        c.0.stdout.take().unwrap().read_to_end(&mut out).unwrap();
        assert!(out.is_empty());
    }
}
#[test]
fn partial_read_is_killed_without_receipt_or_orphan() {
    let (d, _) = fixture();
    let mut c = spawn(&d);
    let mut input = c.0.stdin.take().unwrap();
    input.write_all(&[0, 0]).unwrap();
    c.0.kill().unwrap();
    assert!(!wait(&mut c));
    drop(input);
    let mut out = Vec::new();
    c.0.stdout.take().unwrap().read_to_end(&mut out).unwrap();
    assert!(out.is_empty());
}

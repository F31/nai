use nai_avatar_2d::{AssetBytes, Avatar, LicenseGrants, RenderedFrame};
use nai_contracts::{
    candidate_hash,
    v1::{ErrorCode, PerformanceCue},
};
use nai_media_core::timeline::audio::{AudioBlock, SampleFormat};
use nai_plugin_sdk::plugin::PluginContext;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

fn ctx(epoch: u64) -> PluginContext {
    PluginContext::new(
        "nai-avatar-2d",
        "s",
        epoch,
        10000,
        json!({}),
        [],
        Instant::now() + Duration::from_secs(10),
        Arc::new(AtomicBool::new(false)),
        1000,
    )
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
struct Fixture {
    manifest: Value,
    config: Value,
    data: [Vec<u8>; 3],
    grants: LicenseGrants,
}
impl Fixture {
    fn new() -> Self {
        let data = [
            vec![10, 20, 30, 255, 0, 0, 255, 255],
            vec![0, 0, 0, 255],
            vec![255, 0, 0, 255],
        ];
        let manifest = json!({"schema_version":1,"avatar_id":"test-only-2d","version":"1","kind":"2d","license_ref":"test-license","assets":[
            {"id":"base","path":"base.rgba","sha256":hash(&data[0]),"license_ref":"test-license"},
            {"id":"closed","path":"closed.rgba","sha256":hash(&data[1]),"license_ref":"test-license"},
            {"id":"open","path":"open.rgba","sha256":hash(&data[2]),"license_ref":"test-license"}],
            "model_binding":"software-rgba8-v1","resource_profile_ref":"test-software","supported_actions":["idle","speak"],"supported_emotions":["neutral"],"voice_binding":"test-voice","native_width":2,"native_height":1,"output_width":2,"output_height":1,"fps_num":25,"fps_den":1,"base_duration_samples":"0","switch_points":[],"personalization_refs":{}});
        let config = json!({"schema_version":1,"pixel_format":"rgba8","sample_rate":8000,"max_buffer_samples":8000,"noise_gate":0.02,"full_open_rms":0.5,"attack_ms":10,"release_ms":100,"layers":[
            {"asset_id":"base","role":"base","width":2,"height":1,"x":0,"y":0},
            {"asset_id":"closed","role":"mouth_closed","width":1,"height":1,"x":1,"y":0},
            {"asset_id":"open","role":"mouth_open","width":1,"height":1,"x":1,"y":0}]});
        Self {
            manifest,
            config,
            data,
            grants: LicenseGrants::from(["test-license".into()]),
        }
    }
    fn assets(&self) -> Vec<AssetBytes<'_>> {
        ["base", "closed", "open"]
            .into_iter()
            .zip(&self.data)
            .map(|(id, bytes)| AssetBytes { id, bytes })
            .collect()
    }
    fn admit(&self, context: &PluginContext) -> nai_contracts::v1::ContractError {
        Avatar::admit(
            context,
            &self.manifest,
            &self.config,
            &self.assets(),
            &self.grants,
        )
        .err()
        .expect("invalid fixture must reject")
    }
    fn avatar(&self, context: &PluginContext) -> Arc<Avatar> {
        Arc::new(
            Avatar::admit(
                context,
                &self.manifest,
                &self.config,
                &self.assets(),
                &self.grants,
            )
            .unwrap(),
        )
    }
}
fn cue() -> PerformanceCue {
    let mut cue = PerformanceCue {
        session_id: "s".into(),
        utterance_id: "u".into(),
        generation_epoch: 7,
        speech_text: "test only".into(),
        expires_at_utc_ms: 5000,
        hash_algorithm: "sha256-canonical-v1".into(),
        ..Default::default()
    };
    cue.candidate_hash = candidate_hash(&cue);
    cue
}
fn block(seq: u32, start: u64, count: usize, eos: bool) -> AudioBlock {
    AudioBlock {
        session_id: "s".into(),
        utterance_id: "u".into(),
        generation_epoch: 7,
        seq,
        sample_rate: 8000,
        channels: 1,
        format: SampleFormat::F32,
        sample_start: start,
        sample_count: count as u32,
        end_of_stream: eos,
    }
}

#[test]
fn admission_rejects_tamper_missing_extra_duplicates_and_ungranted_license() {
    let c = ctx(7);
    let mut f = Fixture::new();
    f.data[2][0] ^= 1;
    assert_eq!(f.admit(&c).code, ErrorCode::InvalidArgument as i32);
    f = Fixture::new();
    f.grants.clear();
    assert_eq!(f.admit(&c).code, ErrorCode::PermissionDenied as i32);
    f = Fixture::new();
    let mut assets = f.assets();
    assets.pop();
    assert!(Avatar::admit(&c, &f.manifest, &f.config, &assets, &f.grants).is_err());
    assets.push(AssetBytes {
        id: "alien",
        bytes: &[0; 4],
    });
    assert!(Avatar::admit(&c, &f.manifest, &f.config, &assets, &f.grants).is_err());
    f.manifest["assets"][1]["id"] = json!("base");
    assert!(f.admit(&c).message.contains("duplicate"));
    f = Fixture::new();
    f.config["layers"][1]["role"] = json!("base");
    assert!(f.admit(&c).message.contains("roles"));
    f = Fixture::new();
    f.manifest["assets"][1]["path"] = json!("./closed.rgba");
    assert!(f.admit(&c).message.contains("aliased"));
}

#[test]
fn admission_rejects_geometry_fps_model_unknown_fields_and_invalid_rms_range() {
    let c = ctx(7);
    for (key, value) in [
        ("output_width", json!(1921)),
        ("fps_num", json!(61)),
        ("model_binding", json!("gpu-unavailable")),
        ("supported_actions", json!(["wave"])),
    ] {
        let mut f = Fixture::new();
        f.manifest[key] = value;
        f.admit(&c);
    }
    let mut f = Fixture::new();
    f.config["noise_gate"] = json!(0.9);
    f.admit(&c);
    f = Fixture::new();
    f.config["max_buffer_samples"] = json!(319);
    f.admit(&c);
    f = Fixture::new();
    f.config["layers"][2]["x"] = json!(0);
    f.admit(&c);
    f = Fixture::new();
    f.config["unknown"] = json!(true);
    f.admit(&c);
    f = Fixture::new();
    f.config["layers"][1]["x"] = json!(2);
    f.admit(&c);
}

#[test]
fn silence_noise_attack_and_release_produce_real_scoped_pixels() {
    let c = ctx(7);
    let f = Fixture::new();
    let a = f.avatar(&c);
    let mut s = a.stream(&c, &cue()).unwrap();
    s.push(&c, &f.grants, block(0, 0, 320, false), &[0.01; 320])
        .unwrap();
    let silence = s
        .next_frame(&c, &f.grants, "frame-slot-1")
        .unwrap()
        .unwrap();
    assert_eq!(silence.rgba, vec![10, 20, 30, 255, 0, 0, 0, 255]);
    assert_eq!(silence.mouth_level, 0.0);
    assert_eq!(silence.metadata.generation_epoch, 7);
    assert_eq!(silence.metadata.time_base_den, 8000);
    assert_eq!(silence.metadata.duration, 320);
    s.push(&c, &f.grants, block(1, 320, 320, false), &[0.5; 320])
        .unwrap();
    let open = s
        .next_frame(&c, &f.grants, "frame-slot-2")
        .unwrap()
        .unwrap();
    assert!(open.mouth_level > 0.98);
    assert!(open.rgba[4] > 249);
    assert_eq!(open.metadata.pts, 320);
    s.push(&c, &f.grants, block(2, 640, 320, true), &[0.0; 320])
        .unwrap();
    let release = s
        .next_frame(&c, &f.grants, "frame-slot-3")
        .unwrap()
        .unwrap();
    assert!(release.mouth_level > 0.5 && release.mouth_level < open.mouth_level);
    assert!(s.complete());
    assert!(s.next_frame(&c, &f.grants, "unused").unwrap().is_none());
}

#[test]
fn alpha_compositing_matches_known_source_over_result() {
    let c = ctx(7);
    let mut f = Fixture::new();
    f.data[1] = vec![255, 0, 0, 128];
    f.data[2] = f.data[1].clone();
    f.manifest["assets"][1]["sha256"] = json!(hash(&f.data[1]));
    f.manifest["assets"][2]["sha256"] = json!(hash(&f.data[2]));
    let a = f.avatar(&c);
    let mut s = a.stream(&c, &cue()).unwrap();
    s.push(&c, &f.grants, block(0, 0, 1, true), &[0.0]).unwrap();
    let frame = s.next_frame(&c, &f.grants, "slot").unwrap().unwrap();
    assert_eq!(&frame.rgba[4..], &[128, 0, 127, 255]);
    assert_eq!(frame.metadata.duration, 1);
}

fn render_packets(f: &Fixture, data: &[f32], packet_size: usize) -> Vec<RenderedFrame> {
    let c = ctx(7);
    let a = f.avatar(&c);
    let mut s = a.stream(&c, &cue()).unwrap();
    let mut frames = Vec::new();
    let mut start = 0;
    for (seq, packet) in data.chunks(packet_size).enumerate() {
        s.push(
            &c,
            &f.grants,
            block(
                seq as u32,
                start as u64,
                packet.len(),
                start + packet.len() == data.len(),
            ),
            packet,
        )
        .unwrap();
        start += packet.len();
        while let Some(frame) = s.next_frame(&c, &f.grants, "slot").unwrap() {
            frames.push(frame);
        }
    }
    assert!(s.complete());
    frames
}

#[test]
fn pcm_packetization_does_not_change_rms_pixels_or_rational_timestamps() {
    let mut f = Fixture::new();
    f.manifest["fps_num"] = json!(30000);
    f.manifest["fps_den"] = json!(1001);
    let data: Vec<f32> = (0..2077)
        .map(|i| if i < 1000 { 0.2 } else { 0.7 })
        .collect();
    let whole = render_packets(&f, &data, data.len());
    let fragmented = render_packets(&f, &data, 37);
    assert_eq!(whole.len(), fragmented.len());
    for (index, (a, b)) in whole.iter().zip(&fragmented).enumerate() {
        assert_eq!(a.metadata, b.metadata);
        assert_eq!(a.rgba, b.rgba);
        assert_eq!(a.mouth_level, b.mouth_level);
        assert_eq!(a.metadata.pts, (index as u64 * 8000 * 1001 / 30000) as i64);
    }
    assert_eq!(
        whole
            .iter()
            .map(|f| f.metadata.duration as usize)
            .sum::<usize>(),
        data.len()
    );
}

#[test]
fn invalid_pcm_and_capacity_never_consume_or_replace_existing_samples() {
    let c = ctx(7);
    let mut f = Fixture::new();
    f.config["max_buffer_samples"] = json!(320);
    let a = f.avatar(&c);
    let mut s = a.stream(&c, &cue()).unwrap();
    s.push(&c, &f.grants, block(0, 0, 160, false), &[0.2; 160])
        .unwrap();
    assert!(s.next_frame(&c, &f.grants, "slot").unwrap().is_none());
    assert_eq!(
        s.push(&c, &f.grants, block(1, 160, 161, false), &[0.1; 161])
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted as i32
    );
    let mut invalid = block(1, 160, 160, false);
    invalid.session_id = "foreign".into();
    assert!(s.push(&c, &f.grants, invalid, &[0.1; 160]).is_err());
    for (format, channels, rate) in [
        (SampleFormat::I16, 1, 8000),
        (SampleFormat::F32, 2, 8000),
        (SampleFormat::F32, 1, 16000),
    ] {
        let mut invalid = block(1, 160, 160, false);
        invalid.format = format;
        invalid.channels = channels;
        invalid.sample_rate = rate;
        assert!(s.push(&c, &f.grants, invalid, &[0.1; 160]).is_err());
    }
    assert!(
        s.push(&c, &f.grants, block(2, 160, 160, false), &[0.1; 160])
            .is_err()
    );
    assert!(
        s.push(&c, &f.grants, block(1, 161, 160, false), &[0.1; 160])
            .is_err()
    );
    assert!(
        s.push(&c, &f.grants, block(1, 160, 160, false), &[f32::NAN; 160])
            .is_err()
    );
    assert!(
        s.push(&c, &f.grants, block(1, 160, 160, false), &[1.1; 160])
            .is_err()
    );
    assert_eq!(s.buffered_samples(), 160);
    s.push(&c, &f.grants, block(1, 160, 160, true), &[0.2; 160])
        .unwrap();
    assert_eq!(
        s.next_frame(&c, &f.grants, "slot")
            .unwrap()
            .unwrap()
            .metadata
            .frame_seq,
        0
    );
    assert!(
        s.push(&c, &f.grants, block(2, 320, 1, true), &[0.1])
            .is_err()
    );
}

#[test]
fn epoch_license_and_cancellation_fence_old_streams_without_consuming() {
    let c = ctx(7);
    let f = Fixture::new();
    let a = f.avatar(&c);
    for kind in 0..3 {
        let mut s = a.stream(&c, &cue()).unwrap();
        s.push(&c, &f.grants, block(0, 0, 320, true), &[0.1; 320])
            .unwrap();
        let other = ctx(if kind == 0 { 8 } else { 7 });
        if kind == 1 {
            other.cancel();
        }
        let grants = if kind == 2 {
            LicenseGrants::new()
        } else {
            f.grants.clone()
        };
        assert!(s.next_frame(&other, &grants, "slot").is_err());
        assert_eq!(s.buffered_samples(), 320);
        assert!(s.next_frame(&c, &f.grants, "slot").is_err());
        assert!(!s.complete());
    }
}

#[test]
fn expired_context_and_modified_cue_or_missing_buffer_ref_do_not_produce_frames() {
    let c = ctx(7);
    let f = Fixture::new();
    let a = f.avatar(&c);
    let mut changed = cue();
    changed.speech_text = "changed".into();
    assert!(a.stream(&c, &changed).is_err());
    let mut oversized = cue();
    oversized.speech_text = "x".repeat(65536);
    assert_eq!(
        a.stream(&c, &oversized).err().unwrap().code,
        ErrorCode::ResourceExhausted as i32
    );
    let expired = PluginContext::new(
        "nai-avatar-2d",
        "s",
        7,
        1,
        json!({}),
        [],
        Instant::now() - Duration::from_millis(1),
        Arc::new(AtomicBool::new(false)),
        1000,
    );
    assert_eq!(f.admit(&expired).code, ErrorCode::DeadlineExceeded as i32);
    let mut s = a.stream(&c, &cue()).unwrap();
    s.push(&c, &f.grants, block(0, 0, 320, true), &[0.2; 320])
        .unwrap();
    assert!(s.next_frame(&c, &f.grants, "").is_err());
    assert_eq!(s.buffered_samples(), 320);
    assert!(s.next_frame(&expired, &f.grants, "slot").is_err());
    assert_eq!(s.buffered_samples(), 320);
}

#[test]
fn rms_is_polarity_independent_and_cue_utc_expiry_is_fenced() {
    let f = Fixture::new();
    let positive = render_packets(&f, &[0.5; 640], 640);
    let negative = render_packets(&f, &[-0.5; 640], 61);
    for (a, b) in positive.iter().zip(negative) {
        assert_eq!(a.rgba, b.rgba);
        assert_eq!(a.mouth_level, b.mouth_level);
    }
    let c = ctx(7);
    let a = f.avatar(&c);
    let mut s = a.stream(&c, &cue()).unwrap();
    s.push(&c, &f.grants, block(0, 0, 320, true), &[0.1; 320])
        .unwrap();
    let expired = PluginContext::new(
        "nai-avatar-2d",
        "s",
        7,
        1000,
        json!({}),
        [],
        Instant::now() + Duration::from_secs(1),
        Arc::new(AtomicBool::new(false)),
        5000,
    );
    assert_eq!(
        s.next_frame(&expired, &f.grants, "slot")
            .err()
            .unwrap()
            .code,
        ErrorCode::DeadlineExceeded as i32
    );
    assert_eq!(s.buffered_samples(), 320);
    assert!(s.next_frame(&c, &f.grants, "slot").is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn actual_2d_rgba_enters_a_pinned_slot_and_is_read_without_control_serialization() {
    use nai_media_core::frame_slots::{
        IoBudget, MAX_BUDGET, PoolSpec,
        shared_ram::{SharedFramePool, write_frame},
    };
    use std::os::unix::fs::PermissionsExt;
    let c = ctx(7);
    let f = Fixture::new();
    let a = f.avatar(&c);
    let mut stream = a.stream(&c, &cue()).unwrap();
    stream
        .push(&c, &f.grants, block(0, 0, 320, true), &[0.5; 320])
        .unwrap();
    let produced = stream
        .next_frame(&c, &f.grants, "renderer-owned-pixels")
        .unwrap()
        .unwrap();
    assert!(produced.mouth_level > 0.98);
    let dir = tempfile::tempdir_in("/dev/shm").unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut pool = SharedFramePool::create(
        dir.path(),
        PoolSpec {
            pool_id: "root-placeholder".into(),
            session_id: "s".into(),
            generation_epoch: 7,
            producer_ref: "render-1".into(),
            consumer_ref: "keeper-1".into(),
            slots: 1,
            slot_bytes: 8,
        },
    )
    .unwrap();
    let ticket = pool
        .reserve("render-1", produced.metadata.clone(), Instant::now())
        .unwrap();
    let file = pool.producer_file("render-1").unwrap();
    let commit = write_frame(
        &file,
        &ticket,
        &produced.rgba,
        &IoBudget::new(MAX_BUDGET).unwrap(),
    )
    .unwrap();
    pool.commit("render-1", commit, Instant::now()).unwrap();
    let consumed = pool
        .read_frame("keeper-1", &ticket, &IoBudget::new(MAX_BUDGET).unwrap())
        .unwrap();
    assert_eq!(consumed.rgba, produced.rgba);
    assert_eq!(consumed.metadata.generation_epoch, 7);
    assert_eq!(consumed.metadata.duration, 320);
    assert_ne!(consumed.metadata.buffer_ref, produced.metadata.buffer_ref);
    assert!(
        pool.reserve("render-1", produced.metadata, Instant::now())
            .is_err()
    );
    pool.release("keeper-1", &ticket, Instant::now()).unwrap();
}

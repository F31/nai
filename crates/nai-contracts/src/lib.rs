//! Versioned contract boundary for nai.
//!
//! DTOs are generated from schemas/core and schemas/domain. Validation here is
//! structural only; authorization, factual validity and playback are runtime work.

pub mod v1 {
    include!("generated/nai.v1.rs");
}

/// Mirrors V5.6.1 protobuf packages so generated cross-package imports resolve.
pub mod v561 {
    pub mod continuousaudio {
        pub mod v1 {
            include!("generated/nai.v561.continuousaudio.v1.rs");
        }
    }
    pub mod live {
        pub mod v1 {
            include!("generated/nai.v561.live.v1.rs");
        }
    }
    pub mod avatarstream {
        pub mod v1 {
            include!("generated/nai.v561.avatarstream.v1.rs");
        }
    }
    pub mod media {
        pub mod v1 {
            include!("generated/nai.v561.media.v1.rs");
        }
    }
    pub mod task {
        pub mod v1 {
            include!("generated/nai.v561.task.v1.rs");
        }
    }
    pub mod audio {
        pub mod v1 {
            include!("generated/nai.v561.audio.v1.rs");
        }
    }
    pub mod streamaudio {
        pub mod v1 {
            include!("generated/nai.v561.streamaudio.v1.rs");
        }
    }
    pub mod audition {
        pub mod v1 {
            include!("generated/nai.v561.audition.v1.rs");
        }
    }
    pub mod avatar {
        pub mod v1 {
            include!("generated/nai.v561.avatar.v1.rs");
        }
    }
}
pub use v561::audition::v1 as v561_audition;
pub use v561::avatar::v1 as v561_avatar;
pub use v561::avatarstream::v1 as v561_avatar_stream;
pub use v561::continuousaudio::v1 as v561_continuous_audio;
pub use v561::live::v1 as v561_live;
pub mod v561_live_validation;
pub use v561::streamaudio::v1 as v561_stream_audio;
pub use v561::{audio::v1 as v561_audio, media::v1 as v561_media, task::v1 as v561_task};

pub mod v561_media_validation;
pub mod cosyvoice_http {
    include!("generated/cosyvoice_http.rs");
}

pub mod remote_knowledge {
    //! Generated adapter DTOs and endpoint constants (remote-knowledge.yaml).
    include!("generated/remote_knowledge.rs");
}

pub mod control_http {
    //! Generated validation schemas from control.yaml.
    include!("generated/control_http.rs");
}

include!("generated/catalog.rs");
mod validation;
pub use validation::{MAX_CONTROL_BYTES, decode_envelope, negotiate, validate_envelope};

/// sha256-canonical-v1: generated maps are BTreeMap, so encoding is stable.
/// Approval reference and hash itself are excluded from the approved object.
pub fn candidate_hash(cue: &v1::PerformanceCue) -> String {
    use prost::Message;
    use sha2::Digest;
    let mut cue = cue.clone();
    cue.candidate_hash.clear();
    cue.approval_ref.clear();
    cue.hash_algorithm = "sha256-canonical-v1".into();
    sha2::Sha256::digest(cue.encode_to_vec())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// V4.3 bootstrap namespace, deliberately incompatible with the legacy envelope.
pub mod workbench {
    include!("generated/nai.workbench.v1.rs");
}
pub mod workbench_wire;

pub mod audition_wire;

/// Canonical public AV approval object; the self-referential approval digest is excluded.
pub fn public_media_hash(bundle: &workbench::PublicMediaBundle) -> String {
    use prost::Message;
    use sha2::Digest;
    let mut canonical = bundle.clone();
    canonical.approved_sha256.clear();
    sha2::Sha256::digest(canonical.encode_to_vec())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Hash of a read-only preparation object; not an approval signature or execution permit.
pub fn public_guard_hash(object: &workbench::PublicGuardObject) -> String {
    use prost::Message;
    use sha2::Digest;
    sha2::Sha256::digest(object.encode_to_vec())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Exact UTF-8 caption bytes; no whitespace or punctuation normalization.
pub fn public_caption_hash(text: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
/// Prepared-media descriptor, including renderer identity, formats and actual first-frame hash.
pub fn prepared_media_hash(media: &workbench::PreparedMediaDescriptor) -> String {
    use prost::Message;
    use sha2::Digest;
    sha2::Sha256::digest(media.encode_to_vec())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Bounded metadata only; this channel must never contain PCM or pixel buffers.
pub fn prepared_descriptor_env(v: &workbench::PreparedMediaDescriptor) -> std::io::Result<String> {
    use prost::Message;
    let b = v.encode_to_vec();
    if b.is_empty() || b.len() > 2048 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(b.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn prepared_descriptor_from_env(
    s: &str,
) -> std::io::Result<workbench::PreparedMediaDescriptor> {
    use prost::Message;
    if s.is_empty()
        || s.len() > 4096
        || !s.len().is_multiple_of(2)
        || !s.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let b: std::io::Result<Vec<u8>> = s
        .as_bytes()
        .chunks_exact(2)
        .map(|c| {
            std::str::from_utf8(c)
                .ok()
                .and_then(|v| u8::from_str_radix(v, 16).ok())
                .ok_or_else(|| std::io::ErrorKind::InvalidData.into())
        })
        .collect();
    workbench::PreparedMediaDescriptor::decode(b?.as_slice())
        .map_err(|_| std::io::ErrorKind::InvalidData.into())
}

/// Canonical task source identity. Metadata alone never grants delivery permission.
pub fn agent_task_context_hash(context: &workbench::AgentTaskContext) -> String {
    use prost::Message;
    use sha2::Digest;
    let mut canonical = context.clone();
    canonical.context_sha256.clear();
    let mut h = sha2::Sha256::new();
    h.update(b"nai-task-context-v1\0");
    h.update(canonical.encode_to_vec());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
/// The caller applies the declared bounded normalization policy before hashing.
pub fn question_dedup_hash(normalized: &str) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(b"nai-question-key-v1\0");
    h.update(normalized.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Immutable text-plan identity; progress and the approval view are deliberately excluded.
pub fn live_speech_plan_hash(plan: &workbench::LiveSpeechPlan) -> String {
    use prost::Message;
    use sha2::Digest;
    let mut canonical = plan.clone();
    canonical.revision = 0;
    canonical.current_start = 0;
    canonical.plan_sha256.clear();
    canonical.batch_approval = None;
    let mut h = sha2::Sha256::new();
    h.update(b"nai-live-text-plan-v1\0");
    h.update(canonical.encode_to_vec());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

mod character;
pub use character::{
    CHARACTER_PUBLIC_CHANNELS, CHARACTER_PUBLIC_HEIGHT, CHARACTER_PUBLIC_MAX_SAMPLES,
    CHARACTER_PUBLIC_PCM_FORMAT, CHARACTER_PUBLIC_PIXEL_FORMAT, CHARACTER_PUBLIC_SAMPLE_RATE,
    CHARACTER_PUBLIC_SAMPLES_PER_FRAME, CHARACTER_PUBLIC_WIDTH, character_action_bound,
    character_action_request_bound, character_bounded_media_bound, character_identity_bound,
    character_runtime_view_bound,
};

/// Structural bound for one device-clock observation.
/// `authorized_epoch` is the lease epoch that approved this sentence.
/// `cue_samples` is the source-sample count this sentence has already submitted.
/// Played samples above that count are rejected even when the clip is longer.
/// The cue ceiling cannot exceed the clip. The caller treats a missing observation as unavailable.
/// This does not prove a hardware read, public playback, or a completion acknowledgement.
pub fn device_clock_bound(
    clock: &workbench::PublicDeviceClockObservation,
    output_id: &str,
    preparation_id: &str,
    submitted_samples: u64,
    sample_count: u64,
    authorized_epoch: u64,
    cue_samples: u64,
) -> bool {
    clock.output_id == output_id
        && clock.preparation_id == preparation_id
        && device_clock_session_id(&clock.session_id)
        && authorized_epoch >= 1
        && clock.epoch == authorized_epoch
        && clock.stream_generation >= 1
        && clock.device_frequency > 0
        && clock.sample_rate == 24000
        && cue_samples <= sample_count
        && clock.played_samples <= cue_samples
        && clock.played_samples <= submitted_samples
        && clock.played_samples <= sample_count
        && (clock.played_samples == 0 || clock.device_position > 0)
}

fn device_clock_session_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}

#[cfg(test)]
mod device_clock_bound_tests {
    use super::device_clock_bound;
    use super::workbench::PublicDeviceClockObservation;

    fn clock(played: u64) -> PublicDeviceClockObservation {
        PublicDeviceClockObservation {
            output_id: "00000000-0000-4000-8000-000000000004".into(),
            preparation_id: "00000000-0000-4000-8000-000000000060".into(),
            session_id: "00000000-0000-4000-8000-000000000003".into(),
            epoch: 1,
            stream_generation: 1,
            device_frequency: 384000,
            device_position: 436992,
            sample_rate: 24000,
            played_samples: played,
        }
    }

    #[test]
    fn bound_observation_stays_within_submission() {
        let output = "00000000-0000-4000-8000-000000000004";
        let preparation = "00000000-0000-4000-8000-000000000060";
        assert!(device_clock_bound(
            &clock(27312),
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        assert!(device_clock_bound(
            &clock(0),
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        assert!(device_clock_bound(
            &clock(27840),
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        assert!(!device_clock_bound(
            &clock(27841),
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        assert!(device_clock_bound(
            &clock(27840),
            output,
            preparation,
            96000,
            96000,
            1,
            27840
        ));
        assert!(!device_clock_bound(
            &clock(27841),
            output,
            preparation,
            96000,
            96000,
            1,
            27840
        ));
        assert!(!device_clock_bound(
            &clock(0),
            output,
            preparation,
            96000,
            96000,
            1,
            96001
        ));
        let mut position = clock(27312);
        position.device_position = 0;
        assert!(!device_clock_bound(
            &position,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        let mut other = clock(27312);
        other.output_id = "00000000-0000-4000-8000-000000000099".into();
        assert!(!device_clock_bound(
            &other,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        let mut generation = clock(27312);
        generation.stream_generation = 0;
        assert!(!device_clock_bound(
            &generation,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        let mut frequency = clock(27312);
        frequency.device_frequency = 0;
        assert!(!device_clock_bound(
            &frequency,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        let mut rate = clock(27312);
        rate.sample_rate = 48000;
        assert!(!device_clock_bound(
            &rate,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        let mut epoch = clock(27312);
        epoch.epoch = 0;
        assert!(!device_clock_bound(
            &epoch,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        epoch.epoch = 2;
        assert!(!device_clock_bound(
            &epoch,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
        assert!(!device_clock_bound(
            &clock(27312),
            output,
            preparation,
            27840,
            96000,
            2,
            27840
        ));
        assert!(!device_clock_bound(
            &clock(27312),
            output,
            preparation,
            27840,
            96000,
            0,
            27840
        ));
        let mut session = clock(27312);
        session.session_id.clear();
        assert!(!device_clock_bound(
            &session,
            output,
            preparation,
            27840,
            96000,
            1,
            27840
        ));
    }
}

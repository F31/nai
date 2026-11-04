//! Structural bounds for production character identity, action, runtime view,
//! and bounded media metadata. None of these checks grant playback, a lease,
//! or public output authority.

use crate::v1::{
    CharacterActionRequest, CharacterBoundedMedia, CharacterIdentity, CharacterRuntimeState,
    CharacterRuntimeView,
};

pub const CHARACTER_PUBLIC_WIDTH: u32 = 512;
pub const CHARACTER_PUBLIC_HEIGHT: u32 = 288;
pub const CHARACTER_PUBLIC_SAMPLE_RATE: u32 = 24000;
pub const CHARACTER_PUBLIC_CHANNELS: u32 = 1;
pub const CHARACTER_PUBLIC_SAMPLES_PER_FRAME: u32 = 800;
pub const CHARACTER_PUBLIC_MAX_SAMPLES: u64 = 480_000;
pub const CHARACTER_PUBLIC_PIXEL_FORMAT: &str = "rgb32le";
pub const CHARACTER_PUBLIC_PCM_FORMAT: &str = "s16le-mono-24000";

pub fn character_identity_bound(identity: &CharacterIdentity) -> bool {
    uuid(&identity.workspace_id)
        && token(&identity.character_id)
        && token(&identity.version)
        && sha256(&identity.package_sha256)
        && sha256(&identity.rights_sha256)
}

pub fn character_action_bound(action_id: &str, supported: &[String]) -> bool {
    token(action_id) && supported.iter().any(|item| item == action_id)
}

pub fn character_action_request_bound(
    request: &CharacterActionRequest,
    supported: &[String],
) -> bool {
    let Some(identity) = request.character.as_ref() else {
        return false;
    };
    character_identity_bound(identity)
        && token(&request.request_id)
        && uuid(&request.session_id)
        && request.generation_epoch >= 1
        && uuid(&request.output_id)
        && character_action_bound(&request.action_id, supported)
}

pub fn character_bounded_media_bound(media: &CharacterBoundedMedia) -> bool {
    let Some(identity) = media.identity.as_ref() else {
        return false;
    };
    let max_frames =
        CHARACTER_PUBLIC_MAX_SAMPLES.div_ceil(u64::from(CHARACTER_PUBLIC_SAMPLES_PER_FRAME));
    character_identity_bound(identity)
        && uuid(&media.session_id)
        && media.epoch >= 1
        && token(&media.action_id)
        && media.width == CHARACTER_PUBLIC_WIDTH
        && media.height == CHARACTER_PUBLIC_HEIGHT
        && media.sample_rate == CHARACTER_PUBLIC_SAMPLE_RATE
        && media.channels == CHARACTER_PUBLIC_CHANNELS
        && media.samples_per_frame == CHARACTER_PUBLIC_SAMPLES_PER_FRAME
        && media.max_samples >= 1
        && media.max_samples <= CHARACTER_PUBLIC_MAX_SAMPLES
        && media.max_frames
            == media
                .max_samples
                .div_ceil(u64::from(media.samples_per_frame))
        && media.max_frames <= max_frames
        && media.pixel_format == CHARACTER_PUBLIC_PIXEL_FORMAT
        && media.pcm_format == CHARACTER_PUBLIC_PCM_FORMAT
}

pub fn character_runtime_view_bound(view: &CharacterRuntimeView, supported: &[String]) -> bool {
    let Some(identity) = view.identity.as_ref() else {
        return false;
    };
    if !character_identity_bound(identity) || !uuid(&view.session_id) {
        return false;
    }
    let state =
        CharacterRuntimeState::try_from(view.state).unwrap_or(CharacterRuntimeState::Unspecified);
    match state {
        CharacterRuntimeState::Unspecified => false,
        CharacterRuntimeState::Unavailable
        | CharacterRuntimeState::Lost
        | CharacterRuntimeState::Revoked => view.action_id.is_empty() && view.media.is_none(),
        CharacterRuntimeState::Preview => {
            action_optional(&view.action_id, supported)
                && view.media.as_ref().is_none_or(|media| {
                    character_bounded_media_bound(media)
                        && media.identity.as_ref() == Some(identity)
                        && media.session_id == view.session_id
                        && media.epoch == view.epoch
                        && (view.action_id.is_empty() || media.action_id == view.action_id)
                })
        }
        CharacterRuntimeState::PublicStandby | CharacterRuntimeState::PublicActive => {
            view.epoch >= 1
                && action_for_public(state, &view.action_id, supported)
                && view.media.as_ref().is_some_and(|media| {
                    character_bounded_media_bound(media)
                        && media.identity.as_ref() == Some(identity)
                        && media.session_id == view.session_id
                        && media.epoch == view.epoch
                        && (view.action_id.is_empty() || media.action_id == view.action_id)
                })
        }
    }
}

fn action_optional(action_id: &str, supported: &[String]) -> bool {
    action_id.is_empty() || character_action_bound(action_id, supported)
}

fn action_for_public(state: CharacterRuntimeState, action_id: &str, supported: &[String]) -> bool {
    match state {
        CharacterRuntimeState::PublicStandby => action_optional(action_id, supported),
        CharacterRuntimeState::PublicActive => character_action_bound(action_id, supported),
        _ => false,
    }
}

fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b != b' ' && b != b'\t' && b != b'\n' && b != b'\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    const WS: &str = "00000000-0000-4000-8000-000000000001";
    const SESSION: &str = "00000000-0000-4000-8000-000000000003";
    const OUTPUT: &str = "00000000-0000-4000-8000-000000000004";

    fn identity() -> CharacterIdentity {
        CharacterIdentity {
            workspace_id: WS.into(),
            character_id: "builtin-2d".into(),
            version: "v1".into(),
            package_sha256: "a".repeat(64),
            rights_sha256: "b".repeat(64),
        }
    }

    fn supported() -> Vec<String> {
        vec!["idle".into(), "speak".into()]
    }

    fn media() -> CharacterBoundedMedia {
        CharacterBoundedMedia {
            identity: Some(identity()),
            session_id: SESSION.into(),
            epoch: 1,
            action_id: "speak".into(),
            width: 512,
            height: 288,
            sample_rate: 24000,
            channels: 1,
            max_frames: 600,
            max_samples: 480_000,
            samples_per_frame: 800,
            pixel_format: "rgb32le".into(),
            pcm_format: "s16le-mono-24000".into(),
        }
    }

    fn view(state: CharacterRuntimeState) -> CharacterRuntimeView {
        let public = matches!(
            state,
            CharacterRuntimeState::PublicStandby | CharacterRuntimeState::PublicActive
        );
        CharacterRuntimeView {
            identity: Some(identity()),
            state: state as i32,
            session_id: SESSION.into(),
            epoch: if public || state == CharacterRuntimeState::Preview {
                1
            } else {
                0
            },
            action_id: if state == CharacterRuntimeState::PublicActive {
                "speak".into()
            } else {
                String::new()
            },
            media: if public || state == CharacterRuntimeState::Preview {
                Some(media())
            } else {
                None
            },
            reason_code: String::new(),
        }
    }

    #[test]
    fn identity_requires_workspace_version_and_digests() {
        assert!(character_identity_bound(&identity()));
        let mut missing = identity();
        missing.version.clear();
        assert!(!character_identity_bound(&missing));
        missing = identity();
        missing.package_sha256 = "A".repeat(64);
        assert!(!character_identity_bound(&missing));
        missing = identity();
        missing.workspace_id = "工作区".into();
        assert!(!character_identity_bound(&missing));
    }

    #[test]
    fn unsupported_action_is_rejected() {
        assert!(character_action_bound("speak", &supported()));
        assert!(!character_action_bound("dance", &supported()));
        assert!(!character_action_bound("", &supported()));
        let request = CharacterActionRequest {
            request_id: "req".into(),
            character: Some(identity()),
            session_id: SESSION.into(),
            generation_epoch: 1,
            output_id: OUTPUT.into(),
            action_id: "wave".into(),
            remaining_budget_ms: 250,
            ..Default::default()
        };
        assert!(!character_action_request_bound(&request, &supported()));
        let mut ok = request.clone();
        ok.action_id = "idle".into();
        assert!(character_action_request_bound(&ok, &supported()));
    }

    #[test]
    fn bounded_media_rejects_oversize_and_wrong_format() {
        assert!(character_bounded_media_bound(&media()));
        let mut oversize = media();
        oversize.max_samples = 480_001;
        oversize.max_frames = 481;
        assert!(!character_bounded_media_bound(&oversize));
        let mut empty = media();
        empty.max_samples = 0;
        empty.max_frames = 0;
        assert!(!character_bounded_media_bound(&empty));
        let mut frames = media();
        frames.max_frames = 599;
        assert!(!character_bounded_media_bound(&frames));
        let mut format = media();
        format.pcm_format = "f32le".into();
        assert!(!character_bounded_media_bound(&format));
        let mut size = media();
        size.width = 960;
        size.height = 540;
        assert!(!character_bounded_media_bound(&size));
        let mut epoch = media();
        epoch.epoch = 0;
        assert!(!character_bounded_media_bound(&epoch));
    }

    #[test]
    fn revoked_or_lost_cannot_look_public_active() {
        assert!(character_runtime_view_bound(
            &view(CharacterRuntimeState::PublicActive),
            &supported()
        ));
        assert!(character_runtime_view_bound(
            &view(CharacterRuntimeState::PublicStandby),
            &supported()
        ));
        assert!(character_runtime_view_bound(
            &view(CharacterRuntimeState::Revoked),
            &supported()
        ));
        let mut revoked_active = view(CharacterRuntimeState::Revoked);
        revoked_active.state = CharacterRuntimeState::PublicActive as i32;
        revoked_active.epoch = 1;
        revoked_active.action_id = "speak".into();
        revoked_active.media = Some(media());
        assert!(character_runtime_view_bound(&revoked_active, &supported()));
        let mut revoked_with_media = view(CharacterRuntimeState::Revoked);
        revoked_with_media.media = Some(media());
        assert!(!character_runtime_view_bound(
            &revoked_with_media,
            &supported()
        ));
        let mut lost_with_action = view(CharacterRuntimeState::Lost);
        lost_with_action.action_id = "speak".into();
        assert!(!character_runtime_view_bound(
            &lost_with_action,
            &supported()
        ));
        let mut unspecified = view(CharacterRuntimeState::Unavailable);
        unspecified.state = 0;
        assert!(!character_runtime_view_bound(&unspecified, &supported()));
        let mut preview = view(CharacterRuntimeState::Preview);
        preview.action_id = "dance".into();
        assert!(!character_runtime_view_bound(&preview, &supported()));
        preview.action_id.clear();
        assert!(character_runtime_view_bound(&preview, &supported()));
        let mut standby = view(CharacterRuntimeState::PublicStandby);
        standby.media = None;
        assert!(!character_runtime_view_bound(&standby, &supported()));
    }

    #[test]
    fn bounded_media_message_has_no_raw_payload_fields() {
        let source = include_str!("../../../schemas/domain/media.proto");
        let start = source
            .find("message CharacterBoundedMedia {")
            .expect("bounded media message");
        let body = source[start..]
            .split("message ")
            .nth(1)
            .expect("message body");
        assert!(!body.contains("bytes "));
        assert!(!body.contains("pcm_sha256"));
        assert!(!body.contains("pixels"));
        assert!(!body.contains("buffer_ref"));
    }
}

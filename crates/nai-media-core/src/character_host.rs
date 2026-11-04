//! Independent private-preview host for production character contracts.
//!
//! Software sample clock only: frame index follows played source samples at
//! 800 samples per frame, capped at 480000. Opens as Preview and never emits
//! PublicActive. Cancel becomes Revoked; loss becomes Lost. After either, the
//! view has no action or media and further ticks are refused. Preview is not a
//! playback permit, lease, or public output authority.

use nai_contracts::v1::{
    CharacterBoundedMedia, CharacterIdentity, CharacterRuntimeState, CharacterRuntimeView,
};
use nai_contracts::{
    CHARACTER_PUBLIC_CHANNELS, CHARACTER_PUBLIC_HEIGHT, CHARACTER_PUBLIC_PCM_FORMAT,
    CHARACTER_PUBLIC_PIXEL_FORMAT, CHARACTER_PUBLIC_SAMPLE_RATE,
    CHARACTER_PUBLIC_SAMPLES_PER_FRAME, CHARACTER_PUBLIC_WIDTH, character_action_bound,
    character_bounded_media_bound, character_identity_bound, character_runtime_view_bound,
};

pub const CANCELLED: &str = "CANCELLED";
pub const LOST: &str = "LOST";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterHostError {
    Identity,
    Session,
    Epoch,
    Action,
    Media,
    Closed,
    Bounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterClockReading {
    pub played_samples: u64,
    pub frame_index: u64,
    pub sample_start: u64,
}

pub struct CharacterPreviewHost {
    identity: CharacterIdentity,
    session_id: String,
    epoch: u64,
    supported: Vec<String>,
    state: CharacterRuntimeState,
    action_id: String,
    media: Option<CharacterBoundedMedia>,
    played_samples: u64,
    reason_code: String,
}

impl CharacterPreviewHost {
    pub fn open(
        identity: CharacterIdentity,
        session_id: String,
        epoch: u64,
        supported: Vec<String>,
    ) -> Result<Self, CharacterHostError> {
        if !character_identity_bound(&identity) {
            return Err(CharacterHostError::Identity);
        }
        if epoch == 0 {
            return Err(CharacterHostError::Epoch);
        }
        let host = Self {
            identity,
            session_id,
            epoch,
            supported,
            state: CharacterRuntimeState::Preview,
            action_id: String::new(),
            media: None,
            played_samples: 0,
            reason_code: String::new(),
        };
        if !character_runtime_view_bound(&host.view(), &host.supported) {
            return Err(CharacterHostError::Session);
        }
        Ok(host)
    }

    pub fn view(&self) -> CharacterRuntimeView {
        CharacterRuntimeView {
            identity: Some(self.identity.clone()),
            state: self.state as i32,
            session_id: self.session_id.clone(),
            epoch: self.epoch,
            action_id: self.action_id.clone(),
            media: self.media.clone(),
            reason_code: self.reason_code.clone(),
        }
    }

    pub fn begin_action(
        &mut self,
        action_id: &str,
        max_samples: u64,
    ) -> Result<CharacterRuntimeView, CharacterHostError> {
        self.require_preview()?;
        if !character_action_bound(action_id, &self.supported) {
            return Err(CharacterHostError::Action);
        }
        let media = self.bounded_media(action_id, max_samples);
        if !character_bounded_media_bound(&media) {
            return Err(CharacterHostError::Media);
        }
        self.action_id = action_id.into();
        self.media = Some(media);
        self.played_samples = 0;
        self.reason_code.clear();
        Ok(self.view())
    }

    pub fn tick(
        &mut self,
        played_samples: u64,
    ) -> Result<CharacterClockReading, CharacterHostError> {
        self.require_preview()?;
        let media = self.media.as_ref().ok_or(CharacterHostError::Media)?;
        if played_samples < self.played_samples || played_samples > media.max_samples {
            return Err(CharacterHostError::Bounds);
        }
        let reading = reading_at(played_samples, media.max_samples);
        self.played_samples = played_samples;
        Ok(reading)
    }

    pub fn cancel(&mut self) -> CharacterRuntimeView {
        self.close(CharacterRuntimeState::Revoked, CANCELLED)
    }

    pub fn lose(&mut self) -> CharacterRuntimeView {
        self.close(CharacterRuntimeState::Lost, LOST)
    }

    fn require_preview(&self) -> Result<(), CharacterHostError> {
        if self.state == CharacterRuntimeState::Preview {
            Ok(())
        } else {
            Err(CharacterHostError::Closed)
        }
    }

    fn close(&mut self, state: CharacterRuntimeState, reason: &str) -> CharacterRuntimeView {
        if self.state != CharacterRuntimeState::Preview {
            return self.view();
        }
        self.state = state;
        self.action_id.clear();
        self.media = None;
        self.played_samples = 0;
        self.reason_code = reason.into();
        self.view()
    }

    fn bounded_media(&self, action_id: &str, max_samples: u64) -> CharacterBoundedMedia {
        let samples_per_frame = CHARACTER_PUBLIC_SAMPLES_PER_FRAME;
        CharacterBoundedMedia {
            identity: Some(self.identity.clone()),
            session_id: self.session_id.clone(),
            epoch: self.epoch,
            action_id: action_id.into(),
            width: CHARACTER_PUBLIC_WIDTH,
            height: CHARACTER_PUBLIC_HEIGHT,
            sample_rate: CHARACTER_PUBLIC_SAMPLE_RATE,
            channels: CHARACTER_PUBLIC_CHANNELS,
            max_frames: max_samples.div_ceil(u64::from(samples_per_frame)),
            max_samples,
            samples_per_frame,
            pixel_format: CHARACTER_PUBLIC_PIXEL_FORMAT.into(),
            pcm_format: CHARACTER_PUBLIC_PCM_FORMAT.into(),
        }
    }
}

fn reading_at(played_samples: u64, max_samples: u64) -> CharacterClockReading {
    let samples_per_frame = u64::from(CHARACTER_PUBLIC_SAMPLES_PER_FRAME);
    let max_frames = max_samples.div_ceil(samples_per_frame).max(1);
    let frame_index = (played_samples / samples_per_frame).min(max_frames - 1);
    CharacterClockReading {
        played_samples,
        frame_index,
        sample_start: frame_index.saturating_mul(samples_per_frame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WS: &str = "00000000-0000-4000-8000-000000000001";
    const SESSION: &str = "00000000-0000-4000-8000-000000000003";

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

    fn host() -> CharacterPreviewHost {
        CharacterPreviewHost::open(identity(), SESSION.into(), 1, supported()).unwrap()
    }

    fn assert_preview(view: &CharacterRuntimeView) {
        assert_eq!(view.state, CharacterRuntimeState::Preview as i32);
        assert_ne!(view.state, CharacterRuntimeState::PublicActive as i32);
        assert!(character_runtime_view_bound(view, &supported()));
    }

    #[test]
    fn open_starts_preview_and_never_emits_public_active() {
        let view = host().view();
        assert_preview(&view);
        assert!(view.action_id.is_empty());
        assert!(view.media.is_none());
        assert!(view.reason_code.is_empty());
        let impl_source = include_str!("character_host.rs");
        let production = impl_source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("CharacterRuntimeState::PublicActive"));
        assert!(!production.contains("PublicStandby"));
    }

    #[test]
    fn sample_clock_maps_played_samples_to_frame_index() {
        let mut host = host();
        let view = host.begin_action("speak", 3_200).unwrap();
        assert_preview(&view);
        assert_eq!(view.action_id, "speak");
        let first = host.tick(0).unwrap();
        assert_eq!(
            first,
            CharacterClockReading {
                played_samples: 0,
                frame_index: 0,
                sample_start: 0,
            }
        );
        let second = host.tick(800).unwrap();
        assert_eq!(second.frame_index, 1);
        assert_eq!(second.sample_start, 800);
        let mid = host.tick(801).unwrap();
        assert_eq!(mid.frame_index, 1);
        let last = host.tick(3_200).unwrap();
        assert_eq!(last.frame_index, 3);
        assert_eq!(last.sample_start, 2_400);
        assert_eq!(host.tick(3_201), Err(CharacterHostError::Bounds));
        assert_eq!(host.tick(3_199), Err(CharacterHostError::Bounds));
        assert_preview(&host.view());
    }

    #[test]
    fn cancel_clears_action_and_rejects_further_ticks() {
        let mut host = host();
        host.begin_action("idle", 800).unwrap();
        host.tick(0).unwrap();
        let view = host.cancel();
        assert_eq!(view.state, CharacterRuntimeState::Revoked as i32);
        assert!(view.action_id.is_empty());
        assert!(view.media.is_none());
        assert_eq!(view.reason_code, CANCELLED);
        assert!(character_runtime_view_bound(&view, &supported()));
        assert_eq!(host.tick(800), Err(CharacterHostError::Closed));
        assert_eq!(
            host.begin_action("speak", 800),
            Err(CharacterHostError::Closed)
        );
        let again = host.cancel();
        assert_eq!(again.state, CharacterRuntimeState::Revoked as i32);
        let lost = host.lose();
        assert_eq!(lost.state, CharacterRuntimeState::Revoked as i32);
    }

    #[test]
    fn loss_clears_action_and_rejects_further_ticks() {
        let mut host = host();
        host.begin_action("speak", 1_600).unwrap();
        host.tick(800).unwrap();
        let view = host.lose();
        assert_eq!(view.state, CharacterRuntimeState::Lost as i32);
        assert!(view.action_id.is_empty());
        assert!(view.media.is_none());
        assert_eq!(view.reason_code, LOST);
        assert!(character_runtime_view_bound(&view, &supported()));
        assert_eq!(host.tick(1_600), Err(CharacterHostError::Closed));
        let again = host.lose();
        assert_eq!(again.state, CharacterRuntimeState::Lost as i32);
        let cancelled = host.cancel();
        assert_eq!(cancelled.state, CharacterRuntimeState::Lost as i32);
    }

    #[test]
    fn invalid_identity_and_session_are_rejected() {
        let mut bad = identity();
        bad.version.clear();
        assert!(matches!(
            CharacterPreviewHost::open(bad, SESSION.into(), 1, supported()),
            Err(CharacterHostError::Identity)
        ));
        assert!(matches!(
            CharacterPreviewHost::open(identity(), "工作区".into(), 1, supported()),
            Err(CharacterHostError::Session)
        ));
        assert!(matches!(
            CharacterPreviewHost::open(identity(), SESSION.into(), 0, supported()),
            Err(CharacterHostError::Epoch)
        ));
    }

    #[test]
    fn oversize_or_unsupported_action_is_rejected() {
        let mut host = host();
        assert_eq!(
            host.begin_action("dance", 800),
            Err(CharacterHostError::Action)
        );
        assert_eq!(
            host.begin_action("speak", 480_001),
            Err(CharacterHostError::Media)
        );
        assert_eq!(
            host.begin_action("speak", 0),
            Err(CharacterHostError::Media)
        );
        assert!(host.view().media.is_none());
        assert_eq!(host.tick(0), Err(CharacterHostError::Media));
        assert_preview(&host.view());
    }

    #[test]
    fn preview_view_is_not_a_playback_permit() {
        let mut host = host();
        let view = host.begin_action("speak", 800).unwrap();
        assert_preview(&view);
        assert!(character_runtime_view_bound(&view, &supported()));
        assert_ne!(view.state, CharacterRuntimeState::PublicActive as i32);
        assert_ne!(view.state, CharacterRuntimeState::PublicStandby as i32);
        let source = include_str!("character_host.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        assert!(production.contains("playback permit"));
        assert!(!production.contains("PlaybackLease"));
    }
}

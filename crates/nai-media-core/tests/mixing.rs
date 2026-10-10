use nai_contracts::v1::ErrorCode;
use nai_media_core::mixing::*;

fn device(id: &str) -> DeviceId {
    DeviceId::new(id).unwrap()
}

fn path(owner: &str, id: &str) -> CapturePath {
    CapturePath::new(owner, device(id)).unwrap()
}

fn request(echo: EchoState, now_utc_ms: u64) -> TakeoverRequest {
    TakeoverRequest {
        echo,
        talent_gain: Gain::try_new(800).unwrap(),
        fact_version: "facts-7".into(),
        now_utc_ms,
    }
}

#[test]
fn starts_in_auto_with_tts_only() {
    let mixer = AudioMixer::new();
    assert_eq!(mixer.mode(), MixMode::Auto);
    let levels = mixer.render();
    assert_eq!(levels.tts, Gain::FULL);
    assert_eq!(levels.talent, Gain::MUTE);
    assert!(levels.any_audible());
    assert_eq!(mixer.epoch(), 0);
}

#[test]
fn rendered_tracks_are_never_both_audible() {
    // The muted track is derived from the mode, so neither mode can sum the two.
    let mut mixer = AudioMixer::new();
    let auto = mixer.render();
    assert!(auto.tts.get() > 0 && auto.talent.get() == 0);

    mixer
        .takeover(TakeoverRequest {
            echo: EchoState::pass(0),
            talent_gain: Gain::try_new(500).unwrap(),
            fact_version: "f".into(),
            now_utc_ms: 0,
        })
        .unwrap();
    let takeover = mixer.render();
    assert!(takeover.tts.get() == 0 && takeover.talent.get() > 0);
}

#[test]
fn takeover_requires_a_fresh_passing_echo_test() {
    for (state, code) in [
        (EchoState::Unrun, ErrorCode::CapabilityNotReady),
        (EchoState::Running, ErrorCode::CapabilityNotReady),
        (EchoState::Failed, ErrorCode::CapabilityNotReady),
    ] {
        let mut mixer = AudioMixer::new();
        let err = mixer.takeover(request(state, 0)).unwrap_err();
        assert_eq!(err.code, code, "{state:?}");
        assert_eq!(mixer.mode(), MixMode::Auto);
    }

    let mut mixer = AudioMixer::new();
    let err = mixer
        .takeover(request(EchoState::pass(0), ECHO_TEST_TTL_MS + 1))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::EvidenceExpired);

    // Exactly at the TTL boundary the pass is still acceptable.
    let mut mixer = AudioMixer::new();
    assert!(
        mixer
            .takeover(request(EchoState::pass(0), ECHO_TEST_TTL_MS))
            .is_ok()
    );
}

#[test]
fn echo_test_is_invalid_when_stale_or_dated_in_the_future() {
    assert!(!EchoState::pass(100).valid_at(99));
    assert!(EchoState::pass(100).valid_at(100));
    assert!(!EchoState::pass(0).valid_at(ECHO_TEST_TTL_MS + 1));
    assert!(!EchoState::Unrun.valid_at(1000));
    assert!(!EchoState::Failed.valid_at(1000));
}

#[test]
fn takeover_pauses_auto_mutes_tts_and_clears_the_queue() {
    let mut mixer = AudioMixer::new();
    mixer.enqueue_auto(3).unwrap();
    assert_eq!(mixer.queued(), 3);

    let report = mixer
        .takeover(TakeoverRequest {
            echo: EchoState::pass(10),
            talent_gain: Gain::try_new(600).unwrap(),
            fact_version: "facts-7".into(),
            now_utc_ms: 20,
        })
        .unwrap();

    assert!(report.paused_auto);
    assert!(report.tts_muted);
    assert!(report.echo_accepted);
    assert_eq!(report.cleared_utterances, 3);
    assert_eq!(report.talent.get(), 600);
    assert_eq!(mixer.queued(), 0);
    assert_eq!(mixer.cleared_total(), 3);
    assert_eq!(mixer.mode(), MixMode::Takeover);
    assert_eq!(mixer.epoch(), 1);
    assert_eq!(mixer.render().tts, Gain::MUTE);
}

#[test]
fn second_takeover_is_refused_so_two_capture_paths_never_converge() {
    let mut mixer = AudioMixer::new();
    mixer
        .takeover(TakeoverRequest {
            echo: EchoState::pass(0),
            talent_gain: Gain::MUTE,
            fact_version: "f".into(),
            now_utc_ms: 0,
        })
        .unwrap();
    let err = mixer
        .takeover(TakeoverRequest {
            echo: EchoState::pass(1),
            talent_gain: Gain::FULL,
            fact_version: "f".into(),
            now_utc_ms: 1,
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
}

#[test]
fn takeover_rejects_an_empty_fact_version() {
    let mut mixer = AudioMixer::new();
    let err = mixer
        .takeover(TakeoverRequest {
            echo: EchoState::pass(0),
            talent_gain: Gain::MUTE,
            fact_version: String::new(),
            now_utc_ms: 0,
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
}

fn in_takeover() -> AudioMixer {
    let mut mixer = AudioMixer::new();
    mixer
        .takeover(TakeoverRequest {
            echo: EchoState::pass(0),
            talent_gain: Gain::try_new(700).unwrap(),
            fact_version: "facts-7".into(),
            now_utc_ms: 0,
        })
        .unwrap();
    mixer
}

#[test]
fn resume_requires_authorization_fact_version_and_duty_lease() {
    let good = ResumeProof {
        authorized: true,
        fact_version: "facts-7".into(),
        duty_lease_until_utc_ms: Some(1_000),
    };
    let mut mixer = in_takeover();
    assert!(mixer.resume_auto(&good, 999).is_ok());
    assert_eq!(mixer.mode(), MixMode::Auto);

    for (label, proof, now) in [
        (
            "unauthorized",
            ResumeProof {
                authorized: false,
                ..good.clone()
            },
            999,
        ),
        (
            "fact version",
            ResumeProof {
                fact_version: "facts-8".into(),
                ..good.clone()
            },
            999,
        ),
        (
            "empty fact version",
            ResumeProof {
                fact_version: String::new(),
                ..good.clone()
            },
            999,
        ),
        (
            "expired lease",
            ResumeProof {
                duty_lease_until_utc_ms: Some(1_000),
                ..good.clone()
            },
            1_000,
        ),
        (
            "absent lease",
            ResumeProof {
                duty_lease_until_utc_ms: None,
                ..good.clone()
            },
            0,
        ),
    ] {
        let mut mixer = in_takeover();
        let err = mixer.resume_auto(&proof, now).unwrap_err();
        assert_eq!(err.code, ErrorCode::PermissionDenied, "{label}");
        assert_eq!(mixer.mode(), MixMode::Takeover, "{label}");
    }
}

#[test]
fn resume_is_refused_when_already_auto() {
    let mut mixer = AudioMixer::new();
    let proof = ResumeProof {
        authorized: true,
        fact_version: "facts-7".into(),
        duty_lease_until_utc_ms: Some(10),
    };
    assert_eq!(
        mixer.resume_auto(&proof, 0).unwrap_err().code,
        ErrorCode::Conflict
    );
}

#[test]
fn talent_silence_never_resumes_auto() {
    let mixer = in_takeover();
    // Ending speech is an event; resuming auto is a decision. The event reports
    // the mode and changes nothing.
    assert_eq!(mixer.on_talent_silence(), MixMode::Takeover);
    assert_eq!(mixer.mode(), MixMode::Takeover);
}

#[test]
fn resume_restores_tts_and_mutes_the_talent_track() {
    let mut mixer = in_takeover();
    mixer
        .resume_auto(
            &ResumeProof {
                authorized: true,
                fact_version: "facts-7".into(),
                duty_lease_until_utc_ms: Some(100),
            },
            0,
        )
        .unwrap();
    let levels = mixer.render();
    assert_eq!(levels.tts, Gain::FULL);
    assert_eq!(levels.talent, Gain::MUTE);
}

#[test]
fn only_one_live_capture_path_is_allowed() {
    let mut registry = CaptureRegistry::new();
    let first = registry.acquire(path("keeper", "mic-a")).unwrap();
    assert_eq!(registry.live_count(), 1);

    // A different owner on a different device still conflicts: that is the
    // OBS-plus-companion echo case.
    let err = registry.acquire(path("obs", "mic-b")).unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(registry.live_count(), 1);

    registry.release(&first).unwrap();
    assert_eq!(registry.live_count(), 0);

    // A new holder takes the path at a higher generation; the old lease can no
    // longer free it.
    let second = registry.acquire(path("obs", "mic-b")).unwrap();
    assert_eq!(
        registry.release(&first).unwrap_err().code,
        ErrorCode::StaleEpoch
    );
    assert_eq!(registry.live_count(), 1);
    registry.release(&second).unwrap();
    assert_eq!(registry.live_count(), 0);
}

#[test]
fn releasing_an_unheld_path_is_refused() {
    let mut registry = CaptureRegistry::new();
    let orphan = registry.acquire(path("k", "m")).unwrap();
    registry.release(&orphan).unwrap();
    assert_eq!(
        registry.release(&orphan).unwrap_err().code,
        ErrorCode::Conflict
    );
}

#[test]
fn default_device_change_is_observed_not_followed() {
    let mut registry = CaptureRegistry::new();
    assert_eq!(
        registry.observe_default(device("mic-a")),
        DeviceObservation::First(device("mic-a"))
    );
    assert_eq!(
        registry.observe_default(device("mic-a")),
        DeviceObservation::Unchanged
    );
    assert_eq!(
        registry.observe_default(device("mic-b")),
        DeviceObservation::Changed {
            from: device("mic-a"),
            to: device("mic-b"),
        }
    );
    assert_eq!(registry.default_device(), Some(&device("mic-b")));
}

#[test]
fn identifiers_and_gains_are_validated() {
    assert_eq!(
        DeviceId::new("").unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        DeviceId::new("has space").unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        CapturePath::new("", device("m")).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        Gain::try_new(1001).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(Gain::try_new(0).unwrap(), Gain::MUTE);
    assert_eq!(Gain::try_new(1000).unwrap(), Gain::FULL);
}

#[test]
fn auto_queue_is_bounded_and_auto_only() {
    let mut mixer = AudioMixer::new();
    assert_eq!(
        mixer.enqueue_auto(0).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        mixer.enqueue_auto(MAX_AUTO_QUEUE + 1).unwrap_err().code,
        ErrorCode::ResourceExhausted
    );
    mixer.enqueue_auto(MAX_AUTO_QUEUE).unwrap();

    let mut takeover = in_takeover();
    assert_eq!(
        takeover.enqueue_auto(1).unwrap_err().code,
        ErrorCode::Conflict
    );
}

#[test]
fn audit_reports_a_clean_snapshot_as_violation_free() {
    let mixer = in_takeover();
    let snapshot = mixer.snapshot(1, true);
    assert!(audit_mix(&snapshot).is_empty());
}

fn tokens(snapshot: &MixSnapshot) -> Vec<&'static str> {
    audit_mix(snapshot).into_iter().map(|v| v.token).collect()
}

#[test]
fn audit_detects_each_violation_by_token() {
    assert_eq!(
        tokens(&MixSnapshot {
            mode: MixMode::Takeover,
            tts: Gain::FULL,
            talent: Gain::MUTE,
            live_captures: 1,
            echo_valid: true,
        }),
        vec!["takeover_with_tts_audible"]
    );
    assert_eq!(
        tokens(&MixSnapshot {
            mode: MixMode::Takeover,
            tts: Gain::MUTE,
            talent: Gain::FULL,
            live_captures: 0,
            echo_valid: true,
        }),
        vec!["takeover_without_capture_path"]
    );
    assert_eq!(
        tokens(&MixSnapshot {
            mode: MixMode::Takeover,
            tts: Gain::MUTE,
            talent: Gain::FULL,
            live_captures: 1,
            echo_valid: false,
        }),
        vec!["takeover_without_valid_echo_test"]
    );
    assert_eq!(
        tokens(&MixSnapshot {
            mode: MixMode::Auto,
            tts: Gain::MUTE,
            talent: Gain::try_new(500).unwrap(),
            live_captures: 0,
            echo_valid: true,
        }),
        vec!["auto_with_talent_audible"]
    );
    assert_eq!(
        tokens(&MixSnapshot {
            mode: MixMode::Auto,
            tts: Gain::MUTE,
            talent: Gain::MUTE,
            live_captures: 2,
            echo_valid: true,
        }),
        vec!["multiple_live_capture_paths"]
    );
    // Both tracks audible, independent of the mode rule.
    assert_eq!(
        tokens(&MixSnapshot {
            mode: MixMode::Auto,
            tts: Gain::FULL,
            talent: Gain::FULL,
            live_captures: 0,
            echo_valid: true,
        }),
        vec!["both_tracks_audible", "auto_with_talent_audible"]
    );
    // Four rules at once, in the order the audit reports them.
    assert_eq!(
        tokens(&MixSnapshot {
            mode: MixMode::Takeover,
            tts: Gain::FULL,
            talent: Gain::FULL,
            live_captures: 2,
            echo_valid: false,
        }),
        vec![
            "both_tracks_audible",
            "takeover_with_tts_audible",
            "takeover_without_valid_echo_test",
            "multiple_live_capture_paths",
        ]
    );
}

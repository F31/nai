use nai_contracts::{public_media_hash, workbench::*};
use nai_runtime_core::{
    foundation::{Operation, Result, StopFence},
    public_media::*,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}
fn bundle() -> PublicMediaBundle {
    let mut b = PublicMediaBundle {
        protocol_major: 1,
        bundle_id: id(1),
        workspace_id: id(2),
        session_id: id(3),
        epoch: 1,
        output_id: id(4),
        snapshot_id: id(5),
        snapshot_sha256: "a".repeat(64),
        facts_sha256: "b".repeat(64),
        approval_id: id(6),
        approved_sha256: String::new(),
        guard_permit_id: id(7),
        caption_sha256: "c".repeat(64),
        audio: Some(PublicMediaAudio {
            artifact_id: id(8),
            pcm_sha256: "d".repeat(64),
            sample_count: 24000,
            sample_rate: 24000,
            channels: 1,
        }),
        visual: Some(PublicMediaVisual {
            artifact_id: id(9),
            package_sha256: "e".repeat(64),
            license_sha256: "f".repeat(64),
            width: 512,
            height: 288,
            fps: 30,
        }),
        lease_ms: 500,
        scope: DeliveryScope::Public as i32,
        prepared_media_sha256: "7".repeat(64),
    };
    b.approved_sha256 = public_media_hash(&b);
    b
}
struct Authority {
    state: Mutex<PublicMediaAuthorityState>,
    hook: Option<Box<dyn Fn()>>,
}
impl PublicMediaAuthority for Authority {
    fn read(&self, _: &PublicMediaBundle, op: &Operation) -> Result<PublicMediaAuthorityState> {
        op.check()?;
        if let Some(f) = &self.hook {
            f();
        }
        Ok(self.state.lock().unwrap().clone())
    }
}
fn authority(b: &PublicMediaBundle) -> Authority {
    Authority {
        state: Mutex::new(PublicMediaAuthorityState {
            workspace_id: b.workspace_id.clone(),
            session_id: b.session_id.clone(),
            epoch: b.epoch,
            output_id: b.output_id.clone(),
            snapshot_id: b.snapshot_id.clone(),
            snapshot_sha256: b.snapshot_sha256.clone(),
            facts_sha256: b.facts_sha256.clone(),
            approval_id: b.approval_id.clone(),
            approved_sha256: b.approved_sha256.clone(),
            guard_permit_id: b.guard_permit_id.clone(),
            guard_bundle_sha256: b.approved_sha256.clone(),
            prepared_media_sha256: b.prepared_media_sha256.clone(),
            audio: b.audio.clone().unwrap(),
            visual: b.visual.clone().unwrap(),
            caption_sha256: b.caption_sha256.clone(),
            public_rights: true,
            approval_allowed: true,
            guard_allowed: true,
            assets_licensed: true,
            output_supported: true,
            valid_until_tick: 1000,
        }),
        hook: None,
    }
}
#[test]
fn complete_bundle_is_short_lived_and_failure_is_terminal() {
    let b = bundle();
    let a = authority(&b);
    let fence = Arc::default();
    let op = Operation::new(4000).unwrap();
    let tick = AtomicU64::new(0);
    let clock = || tick.load(Ordering::SeqCst);
    let mut p = admit_public_media(b.clone(), Some(&a), fence, &op, &clock).unwrap();
    tick.store(499, Ordering::SeqCst);
    assert_eq!(p.checked_bundle(Some(&a), &op, &clock).unwrap(), &b);
    tick.store(500, Ordering::SeqCst);
    assert_eq!(
        p.recheck(Some(&a), &op, &clock),
        Err(WorkbenchError::DeadlineExceeded)
    );
    tick.store(100, Ordering::SeqCst);
    assert_eq!(
        p.recheck(Some(&a), &op, &clock),
        Err(WorkbenchError::Cancelled)
    );
}
#[test]
fn no_production_authority_means_no_permission() {
    let b = bundle();
    assert!(matches!(
        admit_public_media(
            b,
            None,
            Arc::default(),
            &Operation::new(4000).unwrap(),
            &|| 0
        ),
        Err(WorkbenchError::CapabilityUnavailable)
    ));
}
#[test]
fn private_scope_missing_axes_and_resource_overruns_are_rejected() {
    for case in 0..9 {
        let mut b = bundle();
        let a = authority(&b);
        match case {
            0 => b.scope = DeliveryScope::Private as i32,
            1 => b.audio = None,
            2 => b.visual = None,
            3 => b.caption_sha256.clear(),
            4 => b.audio.as_mut().unwrap().sample_rate = 48000,
            5 => b.audio.as_mut().unwrap().sample_count = 480001,
            6 => b.visual.as_mut().unwrap().width = 1281,
            7 => b.lease_ms = 501,
            _ => b.audio.as_mut().unwrap().artifact_id = id(9),
        };
        assert!(matches!(
            admit_public_media(
                b,
                Some(&a),
                Arc::default(),
                &Operation::new(4000).unwrap(),
                &|| 0
            ),
            Err(WorkbenchError::InvalidArgument)
        ));
    }
}
#[test]
fn recomputing_self_hash_cannot_replace_stored_approval_or_guard() {
    for guard in [false, true] {
        let mut b = bundle();
        let a = authority(&b);
        if guard {
            a.state.lock().unwrap().guard_bundle_sha256 = "0".repeat(64);
        } else {
            b.audio.as_mut().unwrap().sample_count += 1;
            b.approved_sha256 = public_media_hash(&b);
        }
        assert!(matches!(
            admit_public_media(
                b,
                Some(&a),
                Arc::default(),
                &Operation::new(4000).unwrap(),
                &|| 0
            ),
            Err(WorkbenchError::IntegrityFailed)
        ));
    }
}
#[test]
fn current_identity_fact_and_output_changes_revoke_existing_permit() {
    for case in 0..6 {
        let b = bundle();
        let a = authority(&b);
        let op = Operation::new(4000).unwrap();
        let mut p = admit_public_media(b, Some(&a), Arc::default(), &op, &|| 0).unwrap();
        {
            let mut s = a.state.lock().unwrap();
            match case {
                0 => s.epoch += 1,
                1 => s.output_id = id(10),
                2 => s.facts_sha256 = "0".repeat(64),
                3 => s.snapshot_sha256 = "0".repeat(64),
                4 => s.workspace_id = id(10),
                _ => s.session_id = id(10),
            }
        }
        assert_eq!(
            p.recheck(Some(&a), &op, &|| 1),
            Err(WorkbenchError::RevisionConflict)
        );
        assert_eq!(
            p.recheck(Some(&a), &op, &|| 1),
            Err(WorkbenchError::Cancelled)
        );
    }
}
#[test]
fn revocation_license_or_actual_resource_change_refuses_consumption() {
    for case in 0..7 {
        let b = bundle();
        let a = authority(&b);
        let op = Operation::new(4000).unwrap();
        let mut p = admit_public_media(b, Some(&a), Arc::default(), &op, &|| 0).unwrap();
        {
            let mut s = a.state.lock().unwrap();
            match case {
                0 => s.approval_allowed = false,
                1 => s.guard_allowed = false,
                2 => s.assets_licensed = false,
                3 => s.output_supported = false,
                4 => s.audio.pcm_sha256 = "0".repeat(64),
                5 => s.visual.license_sha256 = "0".repeat(64),
                _ => s.prepared_media_sha256 = "0".repeat(64),
            }
        }
        assert!(p.recheck(Some(&a), &op, &|| 1).is_err());
    }
}
#[test]
fn p0_is_checked_before_any_potentially_blocking_authority_read() {
    let b = bundle();
    let mut a = authority(&b);
    let fence = Arc::new(StopFence::default());
    let op = Operation::new(4000).unwrap();
    let mut p = admit_public_media(b.clone(), Some(&a), fence.clone(), &op, &|| 0).unwrap();
    a.hook = Some(Box::new(|| {
        panic!("authority must not be queried after P0")
    }));
    fence.stop();
    assert_eq!(
        p.recheck(Some(&a), &op, &|| 1),
        Err(WorkbenchError::Cancelled)
    );
    assert!(matches!(
        admit_public_media(b, Some(&a), fence, &op, &|| 1),
        Err(WorkbenchError::Cancelled)
    ));
}
#[test]
fn slow_read_and_p0_during_read_cannot_cross_the_deadline() {
    for stop in [false, true] {
        let b = bundle();
        let mut a = authority(&b);
        let fence = Arc::new(StopFence::default());
        let tick = Arc::new(AtomicU64::new(0));
        let t = tick.clone();
        let f = fence.clone();
        a.hook = Some(Box::new(move || {
            t.store(600, Ordering::SeqCst);
            if stop {
                f.stop();
            }
        }));
        let r = admit_public_media(b, Some(&a), fence, &Operation::new(4000).unwrap(), &|| {
            tick.load(Ordering::SeqCst)
        });
        assert!(matches!(
            r,
            Err(WorkbenchError::DeadlineExceeded) | Err(WorkbenchError::Cancelled)
        ));
    }
}

#[test]
fn content_review_never_creates_playback_capability_for_unready_output() {
    let b = bundle();
    let a = authority(&b);
    a.state.lock().unwrap().output_supported = false;
    let fence = StopFence::default();
    let op = Operation::new(4000).unwrap();
    review_public_media_approval(&b, Some(&a), &fence, &op, &|| 0).unwrap();
    assert_eq!(
        admit_public_media(b.clone(), Some(&a), Arc::default(), &op, &|| 0).err(),
        Some(WorkbenchError::CapabilityUnavailable)
    );
    a.state.lock().unwrap().public_rights = false;
    assert_eq!(
        review_public_media_approval(&b, Some(&a), &fence, &op, &|| 0),
        Err(WorkbenchError::PermissionDenied)
    );
}

#[test]
fn content_review_refuses_slow_observation_and_p0_without_execution() {
    let b = bundle();
    let mut a = authority(&b);
    let clock = Arc::new(AtomicU64::new(0));
    let advance = clock.clone();
    a.hook = Some(Box::new(move || {
        advance.store(500, Ordering::Relaxed);
    }));
    assert_eq!(
        review_public_media_approval(
            &b,
            Some(&a),
            &StopFence::default(),
            &Operation::new(4000).unwrap(),
            &|| clock.load(Ordering::Relaxed)
        ),
        Err(WorkbenchError::DeadlineExceeded)
    );
    let fence = Arc::new(StopFence::default());
    let revoke = fence.clone();
    a.hook = Some(Box::new(move || revoke.stop()));
    assert_eq!(
        review_public_media_approval(&b, Some(&a), &fence, &Operation::new(4000).unwrap(), &|| 0),
        Err(WorkbenchError::Cancelled)
    );
}

fn execution_fixture() -> (
    PublicMediaBundle,
    PreparedMediaDescriptor,
    PreparedMediaReceipt,
) {
    let mut b = bundle();
    let d = PreparedMediaDescriptor {
        protocol_major: 1,
        workspace_id: b.workspace_id.clone(),
        task_id: b.audio.as_ref().unwrap().artifact_id.clone(),
        task_sha256: "a".repeat(64),
        audio: b.audio.clone(),
        visual: b.visual.clone(),
        expected_frames: 30,
        samples_per_frame: 800,
        first_frame_sha256: "e".repeat(64),
        pcm_format: "s16le-mono-24000".into(),
        pixel_format: "rgb32le".into(),
        renderer_id: "rms-sample-v1".into(),
    };
    b.prepared_media_sha256 = nai_contracts::prepared_media_hash(&d);
    b.approved_sha256 = public_media_hash(&b);
    let r = PreparedMediaReceipt {
        protocol_major: 1,
        descriptor_sha256: b.prepared_media_sha256.clone(),
        consumer_instance_id: b.output_id.clone(),
        pcm_sha256: b.audio.as_ref().unwrap().pcm_sha256.clone(),
        sample_count: 24000,
        expected_frames: 30,
        first_frame_sha256: d.first_frame_sha256.clone(),
        tail_frame: Some(PublicMediaFrameInfo {
            frame_index: 30,
            sample_start: 23200,
            sample_count: 800,
            sample_rate: 24000,
            width: 512,
            height: 288,
            pixels_sha256: "f".repeat(64),
            openness_q16: 1,
        }),
    };
    (b, d, r)
}
#[test]
fn execution_issuance_rechecks_current_rights_and_limits_remaining_ttl() {
    use nai_runtime_core::public_execution::*;
    let (b, _, _) = execution_fixture();
    let a = authority(&b);
    let op = Operation::new(4000).unwrap();
    let mut p = admit_public_media(b, Some(&a), Arc::default(), &op, &|| 0).unwrap();
    let key = "f".repeat(64);
    let preparation = id(60);
    let request = IssueRequest {
        authorization: &key,
        preparation_id: &preparation,
        challenge: 1,
        seq: 1,
        max_lease_ms: 500,
    };
    assert_eq!(
        p.execution_lease(Some(&a), &op, &|| 125, &request)
            .unwrap()
            .lease_ms,
        375
    );
    a.state.lock().unwrap().guard_allowed = false;
    assert_eq!(
        p.execution_lease(Some(&a), &op, &|| 126, &request).err(),
        Some(WorkbenchError::PermissionDenied)
    );
    a.state.lock().unwrap().guard_allowed = true;
    assert_eq!(
        p.execution_lease(Some(&a), &op, &|| 127, &request).err(),
        Some(WorkbenchError::Cancelled)
    );
}
#[test]
fn execution_challenge_is_local_and_callback_expiry_cannot_be_renewed() {
    use nai_runtime_core::public_execution::*;
    let (b, d, r) = execution_fixture();
    let a = authority(&b);
    let op = Operation::new(4000).unwrap();
    let key = "f".repeat(64);
    let prep = id(60);
    let mut g = PublicExecutionGate::new(key.clone(), b.clone(), &d, &r, prep.clone()).unwrap();
    let f = g.callback_fence();
    assert!(!f.allows(999));
    let challenge = g.challenge(1000).unwrap();
    let mut p = admit_public_media(b, Some(&a), Arc::default(), &op, &|| 0).unwrap();
    let mut lease = p
        .execution_lease(
            Some(&a),
            &op,
            &|| 0,
            &IssueRequest {
                authorization: &key,
                preparation_id: &prep,
                challenge,
                seq: 1,
                max_lease_ms: 500,
            },
        )
        .unwrap();
    g.accept(&lease, 1200, &op).unwrap();
    assert!(f.allows(1499));
    assert!(!f.allows(1500));
    lease.seq = 2;
    assert!(g.accept(&lease, 1500, &op).is_err());
    assert!(!f.allows(1501));
}
#[test]
fn execution_wrong_identity_token_purpose_replay_and_cancel_are_terminal() {
    use nai_runtime_core::public_execution::*;
    for case in 0..10 {
        let (b, d, r) = execution_fixture();
        let a = authority(&b);
        let op = Operation::new(4000).unwrap();
        let key = "f".repeat(64);
        let prep = id(60);
        let mut g = PublicExecutionGate::new(key.clone(), b.clone(), &d, &r, prep.clone()).unwrap();
        let challenge = g.challenge(0).unwrap();
        let f = g.callback_fence();
        let mut p = admit_public_media(b, Some(&a), Arc::default(), &op, &|| 0).unwrap();
        let mut x = p
            .execution_lease(
                Some(&a),
                &op,
                &|| 0,
                &IssueRequest {
                    authorization: &key,
                    preparation_id: &prep,
                    challenge,
                    seq: 1,
                    max_lease_ms: 500,
                },
            )
            .unwrap();
        let good = x.clone();
        match case {
            0 => x.authorization = "0".repeat(64),
            1 => x.preparation_id = id(61),
            2 => x.output_id = id(61),
            3 => x.epoch += 1,
            4 => x.bundle_sha256 = "0".repeat(64),
            5 => x.prepared_media_sha256 = "0".repeat(64),
            6 => x.purpose = "health".into(),
            7 => {
                g.accept(&x, 1, &op).unwrap();
            }
            8 => op.cancel(),
            _ => x.session_id = id(61),
        };
        assert!(g.accept(&x, 2, &op).is_err(), "case {case}");
        assert!(g.accept(&good, 3, &Operation::new(4000).unwrap()).is_err());
        assert!(!f.allows(3));
    }
}
#[test]
fn execution_registration_stale_challenge_clock_reversal_and_owner_drop_revoke() {
    use nai_runtime_core::public_execution::*;
    let (b, d, r) = execution_fixture();
    let mut bad = r.clone();
    bad.tail_frame.as_mut().unwrap().sample_start = 0;
    assert!(PublicExecutionGate::new("f".repeat(64), b.clone(), &d, &bad, id(60)).is_err());
    for kind in 0..3 {
        let mut g = PublicExecutionGate::new("f".repeat(64), b.clone(), &d, &r, id(60)).unwrap();
        let f = g.callback_fence();
        let ch = g.challenge(100).unwrap();
        let a = authority(&b);
        let op = Operation::new(4000).unwrap();
        let mut p = admit_public_media(b.clone(), Some(&a), Arc::default(), &op, &|| 0).unwrap();
        let x = p
            .execution_lease(
                Some(&a),
                &op,
                &|| 0,
                &IssueRequest {
                    authorization: &"f".repeat(64),
                    preparation_id: &id(60),
                    challenge: ch,
                    seq: 1,
                    max_lease_ms: 500,
                },
            )
            .unwrap();
        if kind == 0 {
            assert!(g.accept(&x, 351, &op).is_err());
        } else {
            g.accept(&x, 101, &op).unwrap();
            assert!(f.allows(102));
            if kind == 1 {
                assert!(!f.allows(101));
            } else {
                drop(g);
            }
        }
        assert!(f.stopped());
    }
}

#[test]
fn initialized_device_requires_new_challenge_and_cannot_reinitialize_after_start() {
    use nai_runtime_core::public_execution::*;
    let (b, d, r) = execution_fixture();
    let key = "f".repeat(64);
    let prep = id(60);
    let a = authority(&b);
    let op = Operation::new(4000).unwrap();
    for stale in [false, true] {
        let mut g = PublicExecutionGate::new(key.clone(), b.clone(), &d, &r, prep.clone()).unwrap();
        let old = g.challenge(0).unwrap();
        g.initialization_barrier().unwrap();
        let fresh = g.challenge(100).unwrap();
        assert_ne!(old, fresh);
        let mut p = admit_public_media(b.clone(), Some(&a), Arc::default(), &op, &|| 0).unwrap();
        let x = p
            .execution_lease(
                Some(&a),
                &op,
                &|| 0,
                &IssueRequest {
                    authorization: &key,
                    preparation_id: &prep,
                    challenge: if stale { old } else { fresh },
                    seq: 1,
                    max_lease_ms: 500,
                },
            )
            .unwrap();
        if stale {
            assert!(g.accept(&x, 101, &op).is_err());
            assert_eq!(g.deadline(), 0);
        } else {
            g.accept(&x, 101, &op).unwrap();
            assert!(g.active(102));
            assert!(g.initialization_barrier().is_err());
            assert!(!g.callback_fence().allows(103));
        }
    }
}

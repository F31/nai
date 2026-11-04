use super::*;
use crate::PublicMediaApprovalRequest;
use nai_contracts::public_media_hash;
use nai_runtime_core::public_media::*;

// Synthetic root observer only; no production guard, asset license or device is asserted.
struct Observations {
    state: std::cell::RefCell<PublicMediaAuthorityState>,
    reads: std::cell::Cell<u32>,
}
impl PublicMediaAuthority for Observations {
    fn read(&self, _: &PublicMediaBundle, op: &Operation) -> Result<PublicMediaAuthorityState> {
        op.check()?;
        self.reads.set(self.reads.get() + 1);
        Ok(self.state.borrow().clone())
    }
}
fn prepared() -> (TempDir, Store, PublicMediaBundle, Observations) {
    let (t, mut s) = init();
    let r = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &r)).unwrap();
    let value: Value = serde_json::from_str(&snapshot.body).unwrap();
    let mut b = PublicMediaBundle {
        protocol_major: 1,
        bundle_id: id(100),
        workspace_id: id(10),
        session_id: id(30),
        epoch: 1,
        output_id: id(101),
        snapshot_id: id(90),
        snapshot_sha256: snapshot.sha256,
        facts_sha256: hash(value["refs"].to_string().as_bytes()),
        approval_id: id(102),
        approved_sha256: String::new(),
        guard_permit_id: id(103),
        caption_sha256: "a".repeat(64),
        audio: Some(PublicMediaAudio {
            artifact_id: id(104),
            pcm_sha256: "b".repeat(64),
            sample_count: 24000,
            sample_rate: 24000,
            channels: 1,
        }),
        visual: Some(PublicMediaVisual {
            artifact_id: id(105),
            package_sha256: "c".repeat(64),
            license_sha256: "d".repeat(64),
            width: 512,
            height: 288,
            fps: 30,
        }),
        lease_ms: 500,
        scope: DeliveryScope::Public as i32,
        prepared_media_sha256: "7".repeat(64),
    };
    b.approved_sha256 = public_media_hash(&b);
    let observations = Observations {
        state: std::cell::RefCell::new(PublicMediaAuthorityState {
            workspace_id: b.workspace_id.clone(),
            session_id: b.session_id.clone(),
            epoch: b.epoch,
            output_id: b.output_id.clone(),
            snapshot_id: b.snapshot_id.clone(),
            snapshot_sha256: b.snapshot_sha256.clone(),
            facts_sha256: b.facts_sha256.clone(),
            approval_id: String::new(),
            approved_sha256: String::new(),
            guard_permit_id: b.guard_permit_id.clone(),
            guard_bundle_sha256: b.approved_sha256.clone(),
            prepared_media_sha256: b.prepared_media_sha256.clone(),
            audio: b.audio.clone().unwrap(),
            visual: b.visual.clone().unwrap(),
            caption_sha256: b.caption_sha256.clone(),
            public_rights: true,
            approval_allowed: false,
            guard_allowed: true,
            assets_licensed: true,
            output_supported: true,
            valid_until_tick: s.public_media_tick() + 10000,
        }),
        reads: std::cell::Cell::new(0),
    };
    (t, s, b, observations)
}
fn approve(
    s: &mut Store,
    b: &PublicMediaBundle,
    o: Option<&dyn PublicMediaAuthority>,
) -> Result<()> {
    s.approve_public_media(
        &Operation::new(4000)?,
        &id(1),
        &PublicMediaApprovalRequest {
            bundle: b.clone(),
            expected_sha256: b.approved_sha256.clone(),
            ttl_ms: 30000,
        },
        o,
    )
}
fn consume(
    s: &Store,
    b: &PublicMediaBundle,
    o: Option<&dyn PublicMediaAuthority>,
) -> Result<PublicMediaPermit> {
    admit_public_media(
        b.clone(),
        Some(&s.public_media_authority(&id(1), o)),
        s.fence.clone(),
        &Operation::new(4000)?,
        &|| s.public_media_tick(),
    )
}
#[test]
fn public_media_ledger_requires_confirmation_observation_and_durable_approval() {
    let (_t, mut s, b, o) = prepared();
    assert_eq!(
        consume(&s, &b, Some(&o)).err(),
        Some(Error::PermissionDenied)
    );
    assert_eq!(approve(&mut s, &b, None), Err(Error::CapabilityUnavailable));
    let mut request = PublicMediaApprovalRequest {
        bundle: b.clone(),
        expected_sha256: "0".repeat(64),
        ttl_ms: 30000,
    };
    assert_eq!(
        s.approve_public_media(&Operation::new(4000).unwrap(), &id(1), &request, Some(&o)),
        Err(Error::InvalidArgument)
    );
    request.expected_sha256 = b.approved_sha256.clone();
    request.ttl_ms = 30001;
    assert_eq!(
        s.approve_public_media(&Operation::new(4000).unwrap(), &id(1), &request, Some(&o)),
        Err(Error::InvalidArgument)
    );
    approve(&mut s, &b, Some(&o)).unwrap();
    assert!(consume(&s, &b, Some(&o)).is_ok());
    assert_eq!(
        consume(&s, &b, None).err(),
        Some(Error::CapabilityUnavailable)
    );
    assert_eq!(approve(&mut s, &b, Some(&o)), Err(Error::RevisionConflict));
    let audit: u64 = s
        .conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE action='public_media_approve'",
            [],
            |r| uint(r, 0),
        )
        .unwrap();
    assert_eq!(audit, 1);
}
#[test]
fn public_media_ledger_rechecks_immutable_bundle_role_session_and_expiry() {
    for change in 0..7 {
        let (_t, mut s, b, o) = prepared();
        approve(&mut s, &b, Some(&o)).unwrap();
        let mut permit = consume(&s, &b, Some(&o)).unwrap();
        let mut changed = b.clone();
        match change {
            0 => {
                s.conn
                    .execute(
                        "UPDATE roles SET revision=revision+1 WHERE workspace_id=?1",
                        [id(10)],
                    )
                    .unwrap();
            }
            1 => {
                s.conn
                    .execute(
                        "UPDATE sessions SET stopped=1 WHERE workspace_id=?1",
                        [id(10)],
                    )
                    .unwrap();
            }
            2 => {
                s.conn
                    .execute(
                        "UPDATE sessions SET epoch=epoch+1 WHERE workspace_id=?1",
                        [id(10)],
                    )
                    .unwrap();
            }
            3 => {
                s.conn
                    .execute("UPDATE public_media_approvals SET expires_tick=0", [])
                    .unwrap();
            }
            4 => {
                changed.caption_sha256 = "f".repeat(64);
                changed.approved_sha256 = public_media_hash(&changed);
            }
            5 => {
                s.conn
                    .execute("UPDATE roles SET role=3 WHERE workspace_id=?1", [id(10)])
                    .unwrap();
            }
            _ => {
                s.conn
                    .execute(
                        "UPDATE public_media_approvals SET created_at=expires_at+1",
                        [],
                    )
                    .unwrap();
            }
        }
        assert!(consume(&s, &changed, Some(&o)).is_err(), "case {change}");
        if change != 4 {
            assert!(
                permit
                    .recheck(
                        Some(&s.public_media_authority(&id(1), Some(&o))),
                        &Operation::new(4000).unwrap(),
                        &|| s.public_media_tick()
                    )
                    .is_err()
            );
        }
    }
}
#[test]
fn public_media_ledger_guard_resource_and_license_remain_live_root_observations() {
    for change in 0..6 {
        let (_t, mut s, b, o) = prepared();
        approve(&mut s, &b, Some(&o)).unwrap();
        match change {
            0 => o.state.borrow_mut().guard_allowed = false,
            1 => o.state.borrow_mut().assets_licensed = false,
            2 => o.state.borrow_mut().output_supported = false,
            3 => o.state.borrow_mut().audio.sample_count += 1,
            4 => o.state.borrow_mut().guard_bundle_sha256 = "f".repeat(64),
            _ => o.state.borrow_mut().public_rights = false,
        }
        assert!(consume(&s, &b, Some(&o)).is_err());
    }
}
#[test]
fn public_media_ledger_revoke_is_fail_closed_even_if_audit_fails_and_after_restart() {
    let (t, mut s, b, o) = prepared();
    approve(&mut s, &b, Some(&o)).unwrap();
    s.conn.execute_batch("CREATE TRIGGER reject_public_revoke BEFORE INSERT ON audit_events WHEN NEW.action='public_media_revoke' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        s.revoke_public_media(
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &b.approval_id
        ),
        Err(Error::IoFailed)
    );
    assert!(consume(&s, &b, Some(&o)).is_err());
    drop(s);
    let s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert!(consume(&s, &b, Some(&o)).is_err());
    let state: String = s
        .conn
        .query_row("SELECT state FROM public_media_approvals", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "revoked");
}
#[test]
fn public_media_ledger_approval_and_audit_commit_atomically_and_restart_keeps_history() {
    let (t, mut s, b, o) = prepared();
    s.conn.execute_batch("CREATE TRIGGER reject_public_approve BEFORE INSERT ON audit_events WHEN NEW.action='public_media_approve' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(approve(&mut s, &b, Some(&o)), Err(Error::IoFailed));
    assert!(consume(&s, &b, Some(&o)).is_err());
    s.conn
        .execute_batch("DROP TRIGGER reject_public_approve")
        .unwrap();
    approve(&mut s, &b, Some(&o)).unwrap();
    drop(s);
    let s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let (rows, active): (u64, u64) = s
        .conn
        .query_row(
            "SELECT COUNT(*),SUM(state='approved') FROM public_media_approvals",
            [],
            |r| Ok((uint(r, 0)?, uint(r, 1)?)),
        )
        .unwrap();
    assert_eq!((rows, active), (1, 0));
    let audits:u64=s.conn.query_row("SELECT COUNT(*) FROM audit_events WHERE action IN('public_media_approve','public_media_restart_revoke')",[],|r|uint(r,0)).unwrap();
    assert_eq!(audits, 2);
}
#[test]
fn public_media_ledger_fact_revocation_and_p0_precede_root_observation() {
    for stop in [false, true] {
        let (_t, mut s, b, o) = prepared();
        approve(&mut s, &b, Some(&o)).unwrap();
        let reads = o.reads.get();
        if stop {
            s.fence.stop();
        } else {
            exec(
                &mut s,
                1,
                C::RevokePublication(FoundationPublicationRevoke {
                    workspace_id: id(10),
                    revocation_id: id(120),
                    snapshot_id: b.snapshot_id.clone(),
                    snapshot_sha256: b.snapshot_sha256.clone(),
                    reason: "事实错误".into(),
                }),
            )
            .unwrap();
        }
        assert!(consume(&s, &b, Some(&o)).is_err());
        assert_eq!(o.reads.get(), reads);
    }
}

#[test]
fn public_media_ledger_bounds_pending_work_and_refuses_cross_workspace_or_viewer() {
    let (_t, mut s, b, o) = prepared();
    let mut altered = b.clone();
    altered.workspace_id = id(11);
    altered.approved_sha256 = public_media_hash(&altered);
    assert!(approve(&mut s, &altered, Some(&o)).is_err());
    s.conn
        .execute("UPDATE roles SET role=3 WHERE workspace_id=?1", [id(10)])
        .unwrap();
    assert_eq!(approve(&mut s, &b, Some(&o)), Err(Error::PermissionDenied));
    s.conn
        .execute("UPDATE roles SET role=1 WHERE workspace_id=?1", [id(10)])
        .unwrap();
    approve(&mut s, &b, Some(&o)).unwrap();
    // Populate only the pending-count boundary; synthetic rows are never consumed as authority.
    for n in 1..200 {
        s.conn.execute("INSERT INTO public_media_approvals SELECT workspace_id,?1,session_id,epoch,principal_id,role_revision,bundle_sha256,bundle,state,expires_tick,expires_at,created_at,updated_at FROM public_media_approvals WHERE approval_id=?2",params![id(200+n),b.approval_id]).unwrap();
    }
    let mut next = b.clone();
    next.approval_id = id(500);
    next.approved_sha256 = public_media_hash(&next);
    assert_eq!(
        approve(&mut s, &next, Some(&o)),
        Err(Error::ResourceExhausted)
    );
}

#[test]
fn public_media_ledger_recycle_revokes_and_confirmed_purge_removes_associations() {
    let (_t, mut s, b, o) = prepared();
    approve(&mut s, &b, Some(&o)).unwrap();
    exec(
        &mut s,
        1,
        C::WorkspaceLifecycle(FoundationWorkspaceLifecycle {
            workspace_id: id(10),
            expected_revision: 1,
            action: WorkspaceAction::Recycle as i32,
        }),
    )
    .unwrap();
    let state: String = s
        .conn
        .query_row("SELECT state FROM public_media_approvals", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "revoked");
    assert!(consume(&s, &b, Some(&o)).is_err());
    let plan = exec(
        &mut s,
        1,
        C::PreviewWorkspacePurge(WorkspacePurgeRead {
            workspace_id: id(10),
            expected_revision: 2,
        }),
    )
    .unwrap()
    .workspace_purge_plan
    .unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    let before: u64 = s
        .conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE action='public_media_approve'",
            [],
            |r| uint(r, 0),
        )
        .unwrap();
    assert_eq!(before, 1);
    let result = exec(
        &mut s,
        1,
        C::PurgeWorkspace(WorkspacePurgeCommand {
            workspace_id: id(10),
            expected_revision: 2,
            plan_sha256: plan.plan_sha256,
            acknowledge: true,
        }),
    )
    .unwrap();
    assert!(result.workspace_purge_status.unwrap().deleted);
    let rows: u64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM public_media_approvals", [], |r| {
            uint(r, 0)
        })
        .unwrap();
    assert_eq!(rows, 0);
    let retained: u64 = s
        .conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE action='public_media_approve'",
            [],
            |r| uint(r, 0),
        )
        .unwrap();
    assert_eq!(retained, before);
}

#[test]
fn content_approval_is_persistent_but_unready_output_still_refuses_execution() {
    let (_t, mut s, b, o) = prepared();
    o.state.borrow_mut().output_supported = false;
    approve(&mut s, &b, Some(&o)).unwrap();
    assert!(consume(&s, &b, Some(&o)).is_err());
    s.revoke_public_media(
        &Operation::new(4000).unwrap(),
        &id(1),
        &b.workspace_id,
        &b.approval_id,
    )
    .unwrap();
    assert!(consume(&s, &b, Some(&o)).is_err());
}

#[test]
fn execution_audit_is_owner_scoped_and_never_accepts_hardware_ack() {
    let (_t, mut s, b, _) = prepared();
    let op = Operation::new(4000).unwrap();
    let count = |s: &Store| {
        s.conn
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE action LIKE 'public_execution_%'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert!(
        s.record_public_execution(&op, &id(2), &b, "public_execution_requested")
            .is_err()
    );
    assert!(
        s.record_public_execution(&op, &id(1), &b, "hardware_playback_completed")
            .is_err()
    );
    assert_eq!(count(&s), 0);
    s.record_public_execution(&op, &id(1), &b, "public_execution_requested")
        .unwrap();
    assert_eq!(count(&s), 1);
    s.conn.execute_batch("CREATE TRIGGER execution_audit_failure BEFORE INSERT ON audit_events BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(
        s.record_public_execution(&op, &id(1), &b, "public_execution_stopped")
            .is_err()
    );
    assert_eq!(count(&s), 1);
}

#[test]
fn request_scoped_source_proof_still_reads_output_and_approval_each_time() {
    let (_t, mut s, b, o) = prepared();
    approve(&mut s, &b, Some(&o)).unwrap();
    let actor = id(1);
    let op = Operation::new(4000).unwrap();
    let a = s.scoped_public_media_authority(&actor, &op, Some(&o));
    assert!(a.read(&b, &op).unwrap().guard_allowed);
    let before = o.reads.get();
    o.state.borrow_mut().guard_allowed = false;
    assert!(!a.read(&b, &op).unwrap().guard_allowed);
    assert_eq!(o.reads.get(), before + 1);
    s.conn
        .execute(
            "UPDATE public_media_approvals SET state='revoked' WHERE approval_id=?1",
            [&b.approval_id],
        )
        .unwrap();
    assert!(a.read(&b, &op).is_err());
}

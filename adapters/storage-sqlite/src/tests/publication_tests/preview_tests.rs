use super::*;
#[test]
fn owner_restart_and_role_revision_change_invalidate_active_preview() {
    for restart in [false, true] {
        let (t, mut s) = init();
        let r = report(&mut s, 85, None);
        let snapshot = exec(&mut s, 1, publish(90, &r)).unwrap();
        exec(&mut s, 1, preview(&snapshot)).unwrap();
        if restart {
            drop(s);
            s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
        } else {
            s.conn.execute("UPDATE roles SET revision=revision+1 WHERE workspace_id=?1 AND principal_id=?2",params![id(10),id(1)]).unwrap();
        }
        let result = exec(&mut s, 1, read_preview()).unwrap();
        assert!(result.body.is_empty());
        assert_ne!(result.state, DeliveryState::Delivering as i32);
    }
}
fn preview(snapshot: &FoundationResult) -> C {
    C::Preview(FoundationPreview {
        workspace_id: id(10),
        preview_id: id(91),
        snapshot_id: id(90),
        snapshot_sha256: snapshot.sha256.clone(),
        cue_id: "intro".into(),
        session_id: id(92),
        expected_epoch: 0,
    })
}
fn read_preview() -> C {
    C::ReadPreview(FoundationPreviewRead {
        workspace_id: id(10),
        preview_id: id(91),
    })
}
#[test]
fn private_preview_receipt_stop_and_restart_never_claim_playback() {
    let (t, mut s) = init();
    let r = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &r)).unwrap();
    let card = exec(&mut s, 1, preview(&snapshot)).unwrap();
    assert_eq!(card.state, DeliveryState::Delivering as i32);
    assert_eq!(card.evidence, EvidenceKind::Submitted as i32);
    let body: Value = serde_json::from_str(&card.body).unwrap();
    assert_eq!(body["target"], "private_workbench");
    assert!(!body["text"].as_str().unwrap().is_empty());
    let ack = exec(
        &mut s,
        1,
        C::PreviewPresented(FoundationPreviewPresented {
            workspace_id: id(10),
            preview_id: id(91),
            preview_sha256: card.sha256.clone(),
            receipt_id: id(93),
        }),
    )
    .unwrap();
    assert_eq!(ack.evidence, EvidenceKind::FrontendPresented as i32);
    assert_eq!(ack.state, DeliveryState::Delivering as i32);
    assert_eq!(
        exec(&mut s, 1, read_preview()).unwrap().evidence,
        EvidenceKind::FrontendPresented as i32
    );
    exec(
        &mut s,
        1,
        C::StopPreview(FoundationPreviewStop {
            workspace_id: id(10),
            session_id: id(92),
            expected_epoch: 1,
        }),
    )
    .unwrap();
    let stopped = exec(&mut s, 1, read_preview()).unwrap();
    assert!(stopped.body.is_empty());
    assert_eq!(stopped.state, DeliveryState::Cancelled as i32);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert!(exec(&mut s, 1, read_preview()).unwrap().body.is_empty());
}
#[test]
fn preview_hash_cue_expiry_and_version_revocation_are_checked() {
    let (_t, mut s) = init();
    let r = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &r)).unwrap();
    let C::Preview(mut request) = preview(&snapshot) else {
        unreachable!()
    };
    request.snapshot_sha256 = "0".repeat(64);
    assert_eq!(
        exec(&mut s, 1, C::Preview(request.clone())),
        Err(Error::RevisionConflict)
    );
    request.snapshot_sha256 = snapshot.sha256.clone();
    request.cue_id = "missing".into();
    assert_eq!(exec(&mut s, 1, C::Preview(request)), Err(Error::NotFound));
    let card = exec(&mut s, 1, preview(&snapshot)).unwrap();
    let body: Value = serde_json::from_str(&card.body).unwrap();
    let expired = crate::preview::read(
        &s.conn,
        &s.root,
        &s.fence,
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &id(91),
        0,
        body["expires_at_ms"].as_i64().unwrap(),
    )
    .unwrap();
    assert_eq!(expired.state, DeliveryState::Expired as i32);
    assert!(expired.body.is_empty());
    exec(
        &mut s,
        1,
        C::RevokePublication(FoundationPublicationRevoke {
            workspace_id: id(10),
            snapshot_id: id(90),
            snapshot_sha256: snapshot.sha256.clone(),
            revocation_id: id(94),
            reason: "事实错误".into(),
        }),
    )
    .unwrap();
    let revoked = exec(&mut s, 1, read_preview()).unwrap();
    assert!(revoked.body.is_empty());
    assert_ne!(revoked.state, DeliveryState::Delivering as i32);
}
#[test]
fn browse_is_scoped_paged_and_excludes_private_artifacts() {
    let (_t, mut s) = init();
    let r = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &r)).unwrap();
    exec(&mut s, 1, preview(&snapshot)).unwrap();
    let query = FoundationBrowse {
        workspace_id: id(10),
        kind: String::new(),
        after_id: String::new(),
        limit: 2,
    };
    let first = exec(&mut s, 1, C::ListPreparation(query.clone())).unwrap();
    assert_eq!(first.resources.len(), 2);
    assert!(!first.next_cursor.is_empty());
    let second = exec(
        &mut s,
        1,
        C::ListPreparation(FoundationBrowse {
            after_id: first.next_cursor.clone(),
            limit: 32,
            ..query.clone()
        }),
    )
    .unwrap();
    assert_eq!(first.resources.len() + second.resources.len(), 5);
    assert!(
        second
            .resources
            .iter()
            .all(|r| r.kind != "publication_private_preview" && r.kind != "publication_impact")
    );
    assert_eq!(
        exec(&mut s, 2, C::ListPreparation(query)),
        Err(Error::PermissionDenied)
    );
    let other = exec(
        &mut s,
        2,
        C::ListWorkspaces(FoundationBrowse {
            limit: 32,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(other.resources.is_empty());
}

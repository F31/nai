use super::*;
fn setup_run() -> (TempDir, Store, FoundationResult, Value) {
    let (t, mut s) = init();
    let r = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &r)).unwrap();
    let manifest: Value = serde_json::from_str(&snapshot.body).unwrap();
    let doc = json!({"schema_version":1,"kind":"manual_run_draft","name":"自己的产品提词","show":manifest["refs"]["show"],"snapshot":pointer(90,&snapshot),"cue_ids":["intro"],"cursor":0,"status":"open","marks":[],"mode":"private_manual","evidence":"self_reported"});
    (t, s, snapshot, doc)
}
fn put(doc: &Value, revision: u64) -> C {
    C::PutPreparation(FoundationObject {
        workspace_id: id(10),
        object_id: id(94),
        expected_revision: revision,
        body: doc.to_string(),
    })
}
fn get(s: &mut Store) -> FoundationResult {
    exec(
        s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(94),
            revision: 0,
        }),
    )
    .unwrap()
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
#[test]
fn zero_model_path_persists_end_marks_and_restarts_without_replay() {
    let (t, mut s, snapshot, mut doc) = setup_run();
    exec(&mut s, 1, put(&doc, 0)).unwrap();
    let shown = exec(&mut s, 1, preview(&snapshot)).unwrap();
    assert_eq!(shown.state, DeliveryState::Delivering as i32);
    exec(
        &mut s,
        1,
        C::StopPreview(FoundationPreviewStop {
            workspace_id: id(10),
            session_id: id(92),
            expected_epoch: shown.epoch,
        }),
    )
    .unwrap();
    doc["status"] = "ended".into();
    doc["marks"] = json!([{"id":id(95),"cue_id":"intro","note":"下场补充说明"}]);
    exec(&mut s, 1, put(&doc, 1)).unwrap();
    let jobs: i64 = s
        .conn
        .query_row(
            "SELECT COUNT(*) FROM outbox WHERE event_id LIKE 'llm:%' OR event_id LIKE 'tts:%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(jobs, 0);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&get(&mut s).body).unwrap(),
        doc
    );
    let old = exec(
        &mut s,
        1,
        C::ReadPreview(FoundationPreviewRead {
            workspace_id: id(10),
            preview_id: id(91),
        }),
    )
    .unwrap();
    assert!(old.body.is_empty());
    let sessions: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM sessions WHERE stopped=0", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(sessions, 0);
    let browse = exec(
        &mut s,
        1,
        C::ListPreparation(FoundationBrowse {
            workspace_id: id(10),
            kind: "manual_run_draft".into(),
            after_id: String::new(),
            limit: 32,
        }),
    )
    .unwrap();
    assert_eq!(browse.resources.len(), 1);
    assert_eq!(browse.resources[0].parent_id, id(83));
    doc["status"] = "open".into();
    assert_eq!(exec(&mut s, 1, put(&doc, 2)), Err(Error::RevisionConflict));
}
#[test]
fn immutable_binding_invalid_cursor_fake_evidence_and_cas_are_rejected() {
    let (_t, mut s, _, doc) = setup_run();
    exec(&mut s, 1, put(&doc, 0)).unwrap();
    for field in ["snapshot", "show", "cue_ids"] {
        let mut bad = doc.clone();
        bad[field] = if field == "cue_ids" {
            json!(["changed"])
        } else {
            let mut p = doc[field].clone();
            p["sha256"] = "0".repeat(64).into();
            p
        };
        assert_eq!(exec(&mut s, 1, put(&bad, 1)), Err(Error::RevisionConflict));
    }
    for field in ["cursor", "evidence", "marks"] {
        let mut bad = doc.clone();
        bad[field] = match field {
            "cursor" => json!(1),
            "evidence" => json!("played"),
            _ => json!([{"id":id(95),"cue_id":"missing","note":"问题"}]),
        };
        assert_eq!(exec(&mut s, 1, put(&bad, 1)), Err(Error::InvalidArgument));
    }
    exec(&mut s, 1, put(&doc, 1)).unwrap();
    assert_eq!(exec(&mut s, 1, put(&doc, 1)), Err(Error::RevisionConflict));
}
#[test]
fn creation_checks_exact_published_hash_and_cue_order_before_writing() {
    for failure in [0, 1, 2] {
        let (_t, mut s, _, mut doc) = setup_run();
        match failure {
            0 => doc["snapshot"]["sha256"] = "0".repeat(64).into(),
            1 => doc["cue_ids"] = json!(["wrong"]),
            _ => doc["show"]["revision"] = json!(2),
        };
        assert!(exec(&mut s, 1, put(&doc, 0)).is_err());
        let count: i64 = s
            .conn
            .query_row("SELECT COUNT(*) FROM objects WHERE id=?1", [id(94)], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
#[test]
fn revoked_snapshot_cannot_resume_display_but_manual_notes_can_be_preserved() {
    let (_t, mut s, snapshot, mut doc) = setup_run();
    exec(&mut s, 1, put(&doc, 0)).unwrap();
    exec(
        &mut s,
        1,
        C::RevokePublication(FoundationPublicationRevoke {
            workspace_id: id(10),
            revocation_id: id(96),
            snapshot_id: id(90),
            snapshot_sha256: snapshot.sha256.clone(),
            reason: "事实变更".into(),
        }),
    )
    .unwrap();
    assert!(exec(&mut s, 1, preview(&snapshot)).is_err());
    doc["status"] = "ended".into();
    doc["marks"] = json!([{"id":id(95),"cue_id":"intro","note":"已停止，待核对新事实"}]);
    exec(&mut s, 1, put(&doc, 1)).unwrap();
    assert_eq!(get(&mut s).revision, 2);
}

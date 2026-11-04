use super::*;
use serde_json::{Value, json};
mod assistance_tests;
mod llm_tests;
mod manual_run_tests;
mod preview_tests;
mod public_media_tests;
mod review_tests;
mod speech_tests;
#[test]
fn publication_rejects_stale_wrong_kind_missing_and_cross_workspace_references() {
    for failure in 0..4 {
        let (_t, mut s) = init();
        let saved = exec(
            &mut s,
            1,
            C::ReadObject(FoundationReadObject {
                workspace_id: id(10),
                object_id: id(83),
                revision: 0,
            }),
        )
        .unwrap();
        let mut show: Value = serde_json::from_str(&saved.body).unwrap();
        let expected = match failure {
            0 => {
                save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
                Error::RevisionConflict
            }
            1 => {
                show["host"] = show["product"].clone();
                save(&mut s, 83, 1, show);
                Error::InvalidArgument
            }
            2 => {
                show["product"]["sha256"] = "0".repeat(64).into();
                save(&mut s, 83, 1, show);
                Error::RevisionConflict
            }
            _ => {
                exec(
                    &mut s,
                    1,
                    C::CreateWorkspace(FoundationWorkspace {
                        workspace_id: id(11),
                        name: "另一工作区".into(),
                        budget_limit_minor: 100,
                        currency: "CNY".into(),
                    }),
                )
                .unwrap();
                let source = exec(
                    &mut s,
                    1,
                    C::PutPreparation(FoundationObject {
                        workspace_id: id(11),
                        object_id: id(84),
                        expected_revision: 0,
                        body: PRODUCT_DRAFT.into(),
                    }),
                )
                .unwrap();
                show["product"] = pointer(84, &source);
                save(&mut s, 83, 1, show);
                Error::NotFound
            }
        };
        let revision = if failure == 0 { 1 } else { 2 };
        assert_eq!(
            exec(
                &mut s,
                1,
                C::PublicationImpact(FoundationImpact {
                    workspace_id: id(10),
                    report_id: id(85),
                    show_id: id(83),
                    show_revision: revision,
                    previous_snapshot_id: String::new()
                })
            ),
            Err(expected)
        );
    }
}
#[test]
fn snapshot_expiry_and_cross_workspace_reads_never_reuse_a_cached_active_state() {
    let (_t, mut s) = init();
    let expires = utc().unwrap() + 60000;
    let mut product: Value = serde_json::from_str(PRODUCT_DRAFT).unwrap();
    product["facts"][0]["valid_until_ms"] = expires.into();
    let p = save(&mut s, 80, 1, product);
    let old = exec(
        &mut s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(83),
            revision: 0,
        }),
    )
    .unwrap();
    let mut show: Value = serde_json::from_str(&old.body).unwrap();
    show["product"] = pointer(80, &p);
    save(&mut s, 83, 1, show);
    let r = report(&mut s, 85, None);
    exec(&mut s, 1, publish(90, &r)).unwrap();
    assert_eq!(
        crate::publication::read(
            &s.conn,
            &s.root,
            &s.fence,
            &Operation::new(4000).unwrap(),
            &id(10),
            &id(90),
            expires
        )
        .unwrap()
        .publication_status,
        PublicationStatus::Blocked as i32
    );
    exec(
        &mut s,
        1,
        C::CreateWorkspace(FoundationWorkspace {
            workspace_id: id(11),
            name: "另一空间".into(),
            budget_limit_minor: 100,
            currency: "CNY".into(),
        }),
    )
    .unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ReadPublication(FoundationPublicationRead {
                workspace_id: id(11),
                snapshot_id: id(90)
            })
        ),
        Err(Error::NotFound)
    );
}
#[test]
fn failed_audit_revocation_changes_watermark_even_when_only_old_versions_are_affected() {
    let (_t, mut s) = init();
    let r = report(&mut s, 85, None);
    let old = exec(&mut s, 1, publish(90, &r)).unwrap();
    updated(&mut s);
    let reviewed = report(&mut s, 86, Some(90));
    s.conn.execute_batch("CREATE TRIGGER reject_revoke BEFORE INSERT ON audit_events WHEN NEW.action='revoke_product_fact_version' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::RevokePublication(FoundationPublicationRevoke {
                workspace_id: id(10),
                revocation_id: id(95),
                snapshot_id: id(90),
                snapshot_sha256: old.sha256,
                reason: "旧版本事实错误".into()
            })
        ),
        Err(Error::IoFailed)
    );
    assert_eq!(
        exec(&mut s, 1, publish(91, &reviewed)),
        Err(Error::RevisionConflict)
    );
    let refreshed = report(&mut s, 87, Some(90));
    assert_eq!(
        exec(&mut s, 1, publish(91, &refreshed))
            .unwrap()
            .publication_status,
        PublicationStatus::PrivateContent as i32
    );
}
fn save(s: &mut Store, n: u64, rev: u64, doc: Value) -> FoundationResult {
    exec(
        s,
        1,
        C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(n),
            expected_revision: rev,
            body: doc.to_string(),
        }),
    )
    .unwrap()
}
fn pointer(n: u64, r: &FoundationResult) -> Value {
    json!({"object_id":id(n),"revision":r.revision,"sha256":r.sha256})
}
fn init() -> (TempDir, Store) {
    let (t, mut s, _) = setup();
    let p = save(&mut s, 80, 0, serde_json::from_str(PRODUCT_DRAFT).unwrap());
    let h = save(
        &mut s,
        81,
        0,
        json!({"schema_version":1,"kind":"host_draft","name":"主持","style":"friendly","preferences":"先讲限制"}),
    );
    let series = save(
        &mut s,
        82,
        0,
        json!({"schema_version":1,"kind":"series_draft","name":"产品介绍","goal":"解释用途与限制"}),
    );
    save(
        &mut s,
        83,
        0,
        json!({"schema_version":1,"kind":"show_draft","name":"首场","product":pointer(80,&p),"host":pointer(81,&h),"series":pointer(82,&series),"cues":[{"id":"intro","title":"产品介绍","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}),
    );
    (t, s)
}
fn report(s: &mut Store, n: u64, previous: Option<u64>) -> FoundationResult {
    let rev: u64 = s
        .conn
        .query_row(
            "SELECT revision FROM objects WHERE workspace_id=?1 AND id=?2",
            params![id(10), id(83)],
            |r| uint(r, 0),
        )
        .unwrap();
    exec(
        s,
        1,
        C::PublicationImpact(FoundationImpact {
            workspace_id: id(10),
            report_id: id(n),
            show_id: id(83),
            show_revision: rev,
            previous_snapshot_id: previous.map(id).unwrap_or_default(),
        }),
    )
    .unwrap()
}
fn publish(n: u64, report: &FoundationResult) -> C {
    C::Publish(FoundationPublish {
        workspace_id: id(10),
        snapshot_id: id(n),
        report_id: report.resource_id.clone(),
        report_sha256: report.sha256.clone(),
        acknowledge_unknown: true,
    })
}
fn read(s: &mut Store, n: u64) -> FoundationResult {
    exec(
        s,
        1,
        C::ReadPublication(FoundationPublicationRead {
            workspace_id: id(10),
            snapshot_id: id(n),
        }),
    )
    .unwrap()
}
fn updated(s: &mut Store) {
    let mut product: Value = serde_json::from_str(PRODUCT_DRAFT).unwrap();
    product["facts"][0]["value"] = "550".into();
    let p = save(s, 80, 1, product);
    let current = exec(
        s,
        1,
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(83),
            revision: 0,
        }),
    )
    .unwrap();
    let mut show: Value = serde_json::from_str(&current.body).unwrap();
    show["product"] = pointer(80, &p);
    save(s, 83, 1, show);
}
#[test]
fn snapshot_keeps_exact_versions_and_reports_field_changes_without_revoking_normal_updates() {
    let (t, mut s) = init();
    let r = report(&mut s, 85, None);
    let published = exec(&mut s, 1, publish(90, &r)).unwrap();
    assert_eq!(
        published.publication_status,
        PublicationStatus::PrivateContent as i32
    );
    let manifest: Value = serde_json::from_str(&published.body).unwrap();
    assert_eq!(manifest["scope"], "private");
    assert_eq!(manifest["presentation"], "content_only");
    updated(&mut s);
    assert_eq!(
        read(&mut s, 90).publication_status,
        PublicationStatus::PrivateContent as i32
    );
    let r = report(&mut s, 86, Some(90));
    let impact: Value = serde_json::from_str(&r.body).unwrap();
    assert!(
        impact["changed_fields"]
            .as_array()
            .unwrap()
            .contains(&json!("product/facts/capacity/value"))
    );
    assert!(
        impact["affected_snapshots"]
            .as_array()
            .unwrap()
            .contains(&json!(id(90)))
    );
    assert!(
        impact["affected_sessions"]
            .as_array()
            .unwrap()
            .contains(&json!(id(30)))
    );
    assert!(!impact["unknown_items"].as_array().unwrap().is_empty());
    exec(&mut s, 1, publish(91, &r)).unwrap();
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(read(&mut s, 90).sha256, published.sha256);
    assert_eq!(read(&mut s, 90).body, published.body);
    assert_eq!(
        read(&mut s, 91).publication_status,
        PublicationStatus::PrivateContent as i32
    );
}
#[test]
fn every_review_watermark_change_requires_a_new_report() {
    for mutation in 0..4 {
        let (_t, mut s) = init();
        let r = report(&mut s, 85, None);
        match mutation {
            0 => {
                updated(&mut s);
            }
            1 => {
                exec(
                    &mut s,
                    1,
                    C::CreateSession(FoundationSession {
                        workspace_id: id(10),
                        session_id: id(31),
                    }),
                )
                .unwrap();
            }
            2 => {
                exec(
                    &mut s,
                    1,
                    C::Grant(FoundationGrant {
                        workspace_id: id(10),
                        principal_id: id(2),
                        role: FoundationRole::Viewer as i32,
                        expected_revision: 0,
                    }),
                )
                .unwrap();
            }
            _ => {
                let sha: String = s
                    .conn
                    .query_row(
                        "SELECT sha256 FROM object_versions WHERE workspace_id=?1 AND object_id=?2",
                        params![id(10), id(20)],
                        |r| r.get(0),
                    )
                    .unwrap();
                prepare(&mut s, 40, &sha);
            }
        }
        assert_eq!(
            exec(&mut s, 1, publish(90, &r)),
            Err(Error::RevisionConflict)
        );
        let exists: bool = s
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM objects WHERE id=?1)",
                params![id(90)],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!exists);
    }
}
#[test]
fn publication_authority_exact_hash_ack_and_expiry_are_enforced() {
    let (_t, mut s) = init();
    let r = report(&mut s, 85, None);
    let mut bad = publish(90, &r);
    if let C::Publish(ref mut p) = bad {
        p.report_sha256 = "0".repeat(64);
    }
    assert_eq!(exec(&mut s, 1, bad), Err(Error::RevisionConflict));
    let mut bad = publish(90, &r);
    if let C::Publish(ref mut p) = bad {
        p.acknowledge_unknown = false;
    }
    assert_eq!(exec(&mut s, 1, bad), Err(Error::InvalidArgument));
    exec(
        &mut s,
        1,
        C::Grant(FoundationGrant {
            workspace_id: id(10),
            principal_id: id(2),
            role: FoundationRole::Operator as i32,
            expected_revision: 0,
        }),
    )
    .unwrap();
    let r = report(&mut s, 86, None);
    assert_eq!(
        exec(&mut s, 2, publish(90, &r)),
        Err(Error::PermissionDenied)
    );
    let actor2 = exec(
        &mut s,
        2,
        C::PublicationImpact(FoundationImpact {
            workspace_id: id(10),
            report_id: id(87),
            show_id: id(83),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }),
    )
    .unwrap();
    assert_eq!(
        exec(&mut s, 1, publish(90, &actor2)),
        Err(Error::PermissionDenied)
    );
    let p = match publish(90, &r) {
        C::Publish(p) => p,
        _ => unreachable!(),
    };
    let expired = serde_json::from_str::<Value>(&r.body).unwrap()["expires_at_ms"]
        .as_i64()
        .unwrap();
    let tx = s.conn.transaction().unwrap();
    assert_eq!(
        crate::publication::publish(
            &tx,
            &s.root,
            &s.fence,
            &Operation::new(4000).unwrap(),
            &id(1),
            1,
            expired,
            &p
        ),
        Err(Error::DeadlineExceeded)
    );
}
#[test]
fn publication_audit_failure_rolls_back_snapshot_outbox_and_idempotency() {
    let (_t, mut s) = init();
    let r = report(&mut s, 85, None);
    let command = command(publish(90, &r));
    s.conn.execute_batch("CREATE TRIGGER reject_publish BEFORE INSERT ON audit_events WHEN NEW.action='publish_private_content' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &command),
        Err(Error::IoFailed)
    );
    for table in ["objects", "outbox"] {
        let column = if table == "objects" {
            "id"
        } else {
            "resource_id"
        };
        let count: i64 = s
            .conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE {column}=?1"),
                params![id(90)],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
    s.conn.execute_batch("DROP TRIGGER reject_publish").unwrap();
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &command)
            .unwrap()
            .publication_status,
        PublicationStatus::PrivateContent as i32
    );
}
#[test]
fn revoked_fact_version_blocks_all_related_snapshots_despite_audit_failure_and_restart() {
    let (t, mut s) = init();
    let r = report(&mut s, 85, None);
    let publish_command = command(publish(90, &r));
    let p = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &publish_command)
        .unwrap();
    let r = report(&mut s, 86, Some(90));
    exec(&mut s, 1, publish(91, &r)).unwrap();
    s.conn.execute_batch("CREATE TRIGGER reject_revoke BEFORE INSERT ON audit_events WHEN NEW.action='revoke_product_fact_version' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let revoke = C::RevokePublication(FoundationPublicationRevoke {
        workspace_id: id(10),
        revocation_id: id(95),
        snapshot_id: id(90),
        snapshot_sha256: p.sha256.clone(),
        reason: "容量事实已确认错误".into(),
    });
    assert_eq!(exec(&mut s, 1, revoke.clone()), Err(Error::IoFailed));
    for n in [90, 91] {
        assert_eq!(
            read(&mut s, n).publication_status,
            PublicationStatus::Blocked as i32
        );
    }
    assert_eq!(
        s.execute(&Operation::new(4000).unwrap(), &id(1), &publish_command)
            .unwrap()
            .publication_status,
        PublicationStatus::Blocked as i32
    );
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    assert_eq!(read(&mut s, 90).sha256, p.sha256);
    assert_eq!(
        read(&mut s, 91).publication_status,
        PublicationStatus::Blocked as i32
    );
    s.conn.execute_batch("DROP TRIGGER reject_revoke").unwrap();
    exec(&mut s, 1, revoke).unwrap();
    updated(&mut s);
    let r = report(&mut s, 87, Some(90));
    let corrected = exec(&mut s, 1, publish(92, &r)).unwrap();
    assert_eq!(
        corrected.publication_status,
        PublicationStatus::PrivateContent as i32
    );
    assert_eq!(
        read(&mut s, 90).publication_status,
        PublicationStatus::Blocked as i32
    );
}
#[test]
fn publication_artifacts_are_immutable_and_no_generic_path_grants_delivery() {
    let (_t, mut s) = init();
    let r = report(&mut s, 85, None);
    let p = exec(&mut s, 1, publish(90, &r)).unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::DeleteObject(FoundationDelete {
                workspace_id: id(10),
                object_id: id(90),
                expected_revision: 1
            })
        ),
        Err(Error::PermissionDenied)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PutObject(FoundationObject {
                workspace_id: id(10),
                object_id: id(90),
                expected_revision: 1,
                body: "erase".into()
            })
        ),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PutObject(FoundationObject {
                workspace_id: id(10),
                object_id: id(96),
                expected_revision: 0,
                body: p.body.clone()
            })
        ),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PrepareDelivery(FoundationDelivery {
                workspace_id: id(10),
                delivery_id: id(97),
                session_id: id(30),
                object_id: id(90),
                object_revision: 1,
                object_sha256: p.sha256,
                target: FoundationTarget::PrivateCard as i32,
                scope: DeliveryScope::Private as i32,
                ttl_ms: 1000
            })
        ),
        Err(Error::CapabilityUnavailable)
    );
}
#[test]
fn impact_capacity_is_explicit_instead_of_silently_omitting_sessions() {
    let (_t, mut s) = init();
    for n in 200..264 {
        exec(
            &mut s,
            1,
            C::CreateSession(FoundationSession {
                workspace_id: id(10),
                session_id: id(n),
            }),
        )
        .unwrap();
    }
    assert_eq!(
        exec(
            &mut s,
            1,
            C::PublicationImpact(FoundationImpact {
                workspace_id: id(10),
                report_id: id(85),
                show_id: id(83),
                show_revision: 1,
                previous_snapshot_id: String::new()
            })
        ),
        Err(Error::ResourceExhausted)
    );
}
mod live_tests;
mod question_pool_tests;

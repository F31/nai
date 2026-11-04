use super::*;
use nai_runtime_core::llm::CredentialVault;
struct FixtureVault;
impl CredentialVault for FixtureVault {
    fn seal(&self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Value> {
        Ok(json!({"reference":id(600),"nonce":"00".repeat(12),"ciphertext":"c0".repeat(32)}))
    }
    fn open(&self, _: &str, _: &str, _: &str, _: &Value) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        Ok(zeroize::Zeroizing::new(b"synthetic-key".to_vec()))
    }
}
fn model(revision: u64) -> FoundationLlmModel {
    FoundationLlmModel {
        workspace_id: id(10),
        model_id: id(100),
        expected_revision: revision,
        name: "测试模型".into(),
        base_url: "https://example.invalid/v1".into(),
        model: "test-text".into(),
        input_minor_per_million: 100,
        output_minor_per_million: 1000,
        enabled: true,
        api_key: if revision == 0 {
            b"synthetic-key-must-not-leak".to_vec()
        } else {
            vec![]
        },
        prices_confirmed: true,
    }
}
fn prepared() -> (TempDir, Store, FoundationResult) {
    let (t, mut s) = init();
    s.attach_llm_vault(Arc::new(FixtureVault));
    exec(&mut s, 1, C::SaveLlmModel(model(0))).unwrap();
    let r = report(&mut s, 85, None);
    let snapshot = exec(&mut s, 1, publish(90, &r)).unwrap();
    (t, s, snapshot)
}
fn start(snapshot: &FoundationResult, n: u64) -> C {
    C::StartLlm(FoundationLlmStart {
        workspace_id: id(10),
        task_id: id(n),
        snapshot_id: id(90),
        snapshot_sha256: snapshot.sha256.clone(),
        cue_id: "intro".into(),
        model_id: id(100),
        model_revision: 1,
        max_cost_minor: 1,
    })
}
fn read_task(n: u64) -> C {
    C::ReadLlmTask(FoundationLlmTaskRead {
        workspace_id: id(10),
        task_id: id(n),
    })
}
fn plan(snapshot: &FoundationResult, max_cost_minor: u64) -> FoundationLlmPlan {
    FoundationLlmPlan {
        workspace_id: id(10),
        snapshot_id: id(90),
        snapshot_sha256: snapshot.sha256.clone(),
        cue_id: "intro".into(),
        model_id: id(100),
        model_revision: 1,
        max_cost_minor,
    }
}
fn checked(s: &mut Store, x: FoundationLlmPlan) -> LlmLocalCheck {
    exec(s, 1, C::CheckLlm(x)).unwrap().llm_check.unwrap()
}
fn changes(s: &Store) -> i64 {
    s.conn
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap()
}
#[test]
fn local_check_is_read_only_and_matches_actual_reservation_without_granting_authority() {
    let (_t, mut s, snapshot) = prepared();
    let before = changes(&s);
    let mut reserve = 0;
    for _ in 0..3 {
        let c = checked(&mut s, plan(&snapshot, 1));
        assert_eq!(c.reason, LlmLocalReason::ReadyLocal as i32);
        assert!(c.credential_checked && c.estimate_known);
        assert!(c.input_bytes > 0);
        reserve = c.estimated_reserve_minor;
    }
    assert_eq!(
        changes(&s),
        before,
        "checking must not even write an idempotency or audit row"
    );
    let r = exec(&mut s, 1, start(&snapshot, 91)).unwrap();
    assert_eq!(r.reserved_minor, reserve);
    let before = changes(&s);
    assert_eq!(
        checked(&mut s, plan(&snapshot, 1)).reason,
        LlmLocalReason::Busy as i32
    );
    assert_eq!(changes(&s), before);
    exec(&mut s, 1, cancel(91)).unwrap();
    // The old successful check cannot authorize a new request after the selected model changes.
    exec(&mut s, 1, C::SaveLlmModel(model(1))).unwrap();
    assert_eq!(
        checked(&mut s, plan(&snapshot, 1)).reason,
        LlmLocalReason::StaleContext as i32
    );
    assert_eq!(
        exec(&mut s, 1, start(&snapshot, 92)),
        Err(Error::RevisionConflict)
    );
    assert_eq!(
        exec(&mut s, 999, C::CheckLlm(plan(&snapshot, 1))),
        Err(Error::PermissionDenied)
    );
}
#[test]
fn local_check_distinguishes_budget_and_unknown_occupancy_without_spending() {
    let (_t, mut s, snapshot) = prepared();
    let b = crate::llm::budget(&s.conn, &id(10)).unwrap();
    exec(
        &mut s,
        1,
        C::SetLlmBudget(FoundationLlmBudget {
            workspace_id: id(10),
            expected_revision: b.revision,
            limit_minor: 0,
        }),
    )
    .unwrap();
    let c = checked(&mut s, plan(&snapshot, 1));
    assert_eq!(c.reason, LlmLocalReason::BudgetDisabled as i32);
    assert!(c.estimate_known && c.estimated_reserve_minor > 0);
    let b = crate::llm::budget(&s.conn, &id(10)).unwrap();
    exec(
        &mut s,
        1,
        C::SetLlmBudget(FoundationLlmBudget {
            workspace_id: id(10),
            expected_revision: b.revision,
            limit_minor: 1,
        }),
    )
    .unwrap();
    let before = changes(&s);
    assert_eq!(
        checked(&mut s, plan(&snapshot, 0)).reason,
        LlmLocalReason::CallLimit as i32
    );
    assert_eq!(changes(&s), before);
    exec(&mut s, 1, start(&snapshot, 91)).unwrap();
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    exec(&mut s, 1, cancel(91)).unwrap();
    let before = changes(&s);
    let c = checked(&mut s, plan(&snapshot, 1));
    assert_eq!(c.reason, LlmLocalReason::WorkspaceLimit as i32);
    assert_eq!(c.available_minor, 0);
    assert_eq!(changes(&s), before);
    assert_eq!(
        crate::llm::budget(&s.conn, &id(10)).unwrap().unknown_minor,
        1
    );
}
#[test]
fn local_check_rejects_missing_native_binding_and_stale_facts_without_leaking_keys() {
    let (_t, mut s, snapshot) = prepared();
    s.llm_vault = None;
    let before = changes(&s);
    let result = exec(&mut s, 1, C::CheckLlm(plan(&snapshot, 1))).unwrap();
    assert_eq!(
        result.llm_check.unwrap().reason,
        LlmLocalReason::CredentialUnavailable as i32
    );
    assert!(result.body.is_empty());
    assert_eq!(changes(&s), before);
    struct BrokenVault;
    impl CredentialVault for BrokenVault {
        fn seal(&self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Value> {
            Err(Error::CapabilityUnavailable)
        }
        fn open(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &Value,
        ) -> Result<zeroize::Zeroizing<Vec<u8>>> {
            Err(Error::IntegrityFailed)
        }
    }
    s.attach_llm_vault(Arc::new(BrokenVault));
    assert_eq!(
        checked(&mut s, plan(&snapshot, 1)).reason,
        LlmLocalReason::CredentialUnavailable as i32
    );
    let mut no_key = model(0);
    no_key.model_id = id(101);
    no_key.api_key.clear();
    exec(&mut s, 1, C::SaveLlmModel(no_key)).unwrap();
    let mut x = plan(&snapshot, 1);
    x.model_id = id(101);
    assert_eq!(
        checked(&mut s, x.clone()).reason,
        LlmLocalReason::CredentialMissing as i32
    );
    let mut disabled = model(1);
    disabled.model_id = id(101);
    disabled.enabled = false;
    exec(&mut s, 1, C::SaveLlmModel(disabled)).unwrap();
    x.model_revision = 2;
    assert_eq!(
        checked(&mut s, x).reason,
        LlmLocalReason::ModelDisabled as i32
    );
    s.attach_llm_vault(Arc::new(FixtureVault));
    save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
    assert_eq!(
        checked(&mut s, plan(&snapshot, 1)).reason,
        LlmLocalReason::StaleContext as i32
    );
    s.fence.stop();
    assert_eq!(
        checked(&mut s, plan(&snapshot, 1)).reason,
        LlmLocalReason::Stopped as i32
    );
}
fn cancel(n: u64) -> C {
    C::CancelLlm(FoundationLlmTaskRead {
        workspace_id: id(10),
        task_id: id(n),
    })
}
fn reply(n: u64, known: bool) -> LlmWorkerResult {
    LlmWorkerResult {
        task_id: id(n),
        text: json!({"text":"标准款容量500毫升。","fact_ids":["capacity"]}).to_string(),
        usage_known: known,
        input_tokens: 100,
        output_tokens: 20,
        error: String::new(),
        request_started: true,
    }
}
#[test]
fn model_cipher_is_not_returned_and_endpoint_price_and_role_changes_are_explicit() {
    let (_t, mut s, _) = prepared();
    for c in [
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(10),
        }),
        C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(100),
            revision: 0,
        }),
    ] {
        let r = exec(&mut s, 1, c).unwrap();
        assert!(r.body.is_empty());
        assert!(r.llm_models[0].credential_saved);
        assert!(!String::from_utf8_lossy(&r.encode_to_vec()).contains("synthetic-key"));
    }
    let mut changed = model(1);
    changed.base_url = "https://other.invalid/v1".into();
    assert_eq!(
        exec(&mut s, 1, C::SaveLlmModel(changed)),
        Err(Error::InvalidArgument)
    );
    let mut missing = model(1);
    missing.prices_confirmed = false;
    assert_eq!(
        exec(&mut s, 1, C::SaveLlmModel(missing)),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        exec(&mut s, 2, C::SaveLlmModel(model(1))),
        Err(Error::PermissionDenied)
    );
}
#[test]
fn zero_budget_refuses_and_reservation_dispatch_completion_adoption_are_atomic() {
    let (_t, mut s, snapshot) = prepared();
    exec(
        &mut s,
        1,
        C::SetLlmBudget(FoundationLlmBudget {
            workspace_id: id(10),
            expected_revision: 1,
            limit_minor: 0,
        }),
    )
    .unwrap();
    assert_eq!(
        exec(&mut s, 1, start(&snapshot, 91)),
        Err(Error::ResourceExhausted)
    );
    exec(
        &mut s,
        1,
        C::SetLlmBudget(FoundationLlmBudget {
            workspace_id: id(10),
            expected_revision: 2,
            limit_minor: 10,
        }),
    )
    .unwrap();
    let command = command(start(&snapshot, 91));
    let queued = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &command)
        .unwrap();
    assert_eq!(queued.task_state, AgentTaskState::Queued as i32);
    assert_eq!(queued.reserved_minor, 1);
    let request = s
        .llm_request(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    assert!(request.input.contains("标准款"));
    assert_eq!(request.max_output_tokens, 256);
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    assert!(
        s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
            .is_err()
    );
    s.finish_llm(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &reply(91, true),
    )
    .unwrap();
    let r = exec(&mut s, 1, read_task(91)).unwrap();
    assert_eq!(r.suggestion_state, SuggestionState::Proposed as i32);
    assert_eq!(r.charge_status, "known");
    assert_eq!(r.actual_minor, 1);
    assert!(!r.body.contains("\"input\""));
    let adopted = exec(
        &mut s,
        1,
        C::ResolveLlm(FoundationLlmResolve {
            workspace_id: id(10),
            task_id: id(91),
            task_sha256: r.sha256,
            decision: FaqDecision::Accept as i32,
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(adopted.adopted_revision, 2);
    let replay = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &command)
        .unwrap();
    assert_eq!(replay.suggestion_state, SuggestionState::Accepted as i32);
    assert!(replay.body.is_empty());
}
#[test]
fn cancelled_dispatched_tasks_never_reveal_late_output_and_unknown_budget_survives_restart() {
    let (t, mut s, snapshot) = prepared();
    exec(&mut s, 1, start(&snapshot, 91)).unwrap();
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    let r = exec(&mut s, 1, cancel(91)).unwrap();
    assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
    assert_eq!(r.charge_status, "unknown");
    s.finish_llm(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &reply(91, false),
    )
    .unwrap();
    let r = exec(&mut s, 1, read_task(91)).unwrap();
    assert!(r.body.is_empty());
    assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
    drop(s);
    let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
    let r = exec(&mut s, 1, read_task(91)).unwrap();
    assert_eq!(r.charge_status, "unknown");
    assert_eq!(r.llm_budget.unwrap().unknown_minor, 1);
}
#[test]
fn recovery_distinguishes_never_dispatched_from_unknown_and_does_not_replay() {
    for dispatched in [false, true] {
        let (t, mut s, snapshot) = prepared();
        let command = command(start(&snapshot, 91));
        s.execute(&Operation::new(4000).unwrap(), &id(1), &command)
            .unwrap();
        if dispatched {
            s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
                .unwrap();
        }
        drop(s);
        let mut s = Store::open(t.path(), &id(1), false, Arc::default()).unwrap();
        let replay = s
            .execute(&Operation::new(4000).unwrap(), &id(1), &command)
            .unwrap();
        assert_ne!(replay.task_state, AgentTaskState::Queued as i32);
        assert_eq!(
            replay.charge_status,
            if dispatched { "unknown" } else { "released" }
        );
    }
}
#[test]
fn audit_failure_keeps_cancellation_tight_but_adoption_rolls_back() {
    let (_t, mut s, snapshot) = prepared();
    exec(&mut s, 1, start(&snapshot, 91)).unwrap();
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    s.conn.execute_batch("CREATE TRIGGER fail_cancel BEFORE INSERT ON audit_events WHEN NEW.action='llm_task_cancel' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert_eq!(exec(&mut s, 1, cancel(91)), Err(Error::IoFailed));
    assert_eq!(
        exec(&mut s, 1, read_task(91)).unwrap().task_state,
        AgentTaskState::Cancelled as i32
    );
    s.conn.execute_batch("DROP TRIGGER fail_cancel").unwrap();
    s.finish_llm(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &reply(91, true),
    )
    .unwrap();
    assert!(exec(&mut s, 1, read_task(91)).unwrap().body.is_empty());
    exec(&mut s, 1, start(&snapshot, 92)).unwrap();
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(92))
        .unwrap();
    s.finish_llm(
        &Operation::new(4000).unwrap(),
        &id(1),
        &id(10),
        &reply(92, true),
    )
    .unwrap();
    let r = exec(&mut s, 1, read_task(92)).unwrap();
    s.conn.execute_batch("CREATE TRIGGER fail_adopt BEFORE INSERT ON audit_events WHEN NEW.action='llm_task_resolve' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ResolveLlm(FoundationLlmResolve {
                workspace_id: id(10),
                task_id: id(92),
                task_sha256: r.sha256,
                decision: FaqDecision::Accept as i32,
                ..Default::default()
            })
        ),
        Err(Error::IoFailed)
    );
    assert_eq!(object(&s.conn, &id(10), &id(80), 0).unwrap().0, 1);
}
#[test]
fn stale_context_invalid_output_and_overrun_never_erase_incurred_cost() {
    for changed in 0..3 {
        let (_t, mut s, snapshot) = prepared();
        exec(&mut s, 1, start(&snapshot, 91)).unwrap();
        s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
            .unwrap();
        let mut reply = reply(91, true);
        if changed == 0 {
            save(&mut s, 80, 1, serde_json::from_str(PRODUCT_DRAFT).unwrap());
        } else if changed == 1 {
            reply.text = json!({"text":"标准款600毫升","fact_ids":["capacity"]}).to_string();
        } else {
            reply.output_tokens = 1000000;
        }
        s.finish_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &reply)
            .unwrap();
        let r = exec(&mut s, 1, read_task(91)).unwrap();
        assert_eq!(r.charge_status, "known");
        if changed != 2 {
            assert!(r.body.is_empty());
            assert_eq!(exec(&mut s, 1, cancel(91)), Err(Error::RevisionConflict));
            assert_eq!(
                exec(&mut s, 1, read_task(91)).unwrap().task_state,
                AgentTaskState::Failed as i32
            );
        } else {
            assert!(r.actual_minor > r.reserved_minor);
            assert_eq!(
                exec(&mut s, 1, start(&snapshot, 92)),
                Err(Error::ResourceExhausted)
            );
        }
    }
}
#[test]
fn model_test_needs_no_publication_and_cannot_be_adopted_or_replayed() {
    let (_t, mut s) = init();
    s.attach_llm_vault(Arc::new(FixtureVault));
    exec(&mut s, 1, C::SaveLlmModel(model(0))).unwrap();
    let test = FoundationModelTest {
        workspace_id: id(10),
        task_id: id(91),
        model_id: id(100),
        model_revision: 1,
    };
    let command = command(C::TestLlm(test));
    let before: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM objects", [], |r| r.get(0))
        .unwrap();
    let r = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &command)
        .unwrap();
    assert_eq!(r.task_state, AgentTaskState::Queued as i32);
    let after: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM objects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, before + 1, "test creates no product/show/snapshot");
    let request = s
        .llm_request(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    assert!(request.connection_test);
    assert_eq!(request.input, nai_runtime_core::llm::TEST_INPUT);
    assert!(!request.input.contains("标准款"));
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    let mut response = reply(91, true);
    response.text = "NAI_OK".into();
    s.finish_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &response)
        .unwrap();
    let r = exec(&mut s, 1, read_task(91)).unwrap();
    assert_eq!(r.task_reason, "API_RESPONSE_OK");
    assert_eq!(r.suggestion_state, 0);
    assert_eq!(r.actual_minor, 0);
    assert!(r.body.contains("NAI_OK"));
    assert!(!r.body.contains("\"input\""));
    assert_eq!(
        exec(
            &mut s,
            1,
            C::ResolveLlm(FoundationLlmResolve {
                workspace_id: id(10),
                task_id: id(91),
                task_sha256: r.sha256.clone(),
                decision: FaqDecision::Accept as i32,
                ..Default::default()
            })
        ),
        Err(Error::InvalidArgument)
    );
    let r = s
        .execute(&Operation::new(4000).unwrap(), &id(1), &command)
        .unwrap();
    assert_eq!(r.task_state, AgentTaskState::Completed as i32);
    assert!(
        s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
            .is_err()
    );
}
#[test]
fn model_test_rechecks_binding_and_keeps_cancelled_sent_cost_unknown() {
    let (_t, mut s) = init();
    s.attach_llm_vault(Arc::new(FixtureVault));
    exec(&mut s, 1, C::SaveLlmModel(model(0))).unwrap();
    let test = FoundationModelTest {
        workspace_id: id(10),
        task_id: id(91),
        model_id: id(100),
        model_revision: 1,
    };
    assert_eq!(
        exec(&mut s, 2, C::TestLlm(test.clone())),
        Err(Error::PermissionDenied)
    );
    let mut bad = test.clone();
    bad.model_revision = 2;
    assert_eq!(
        exec(&mut s, 1, C::TestLlm(bad)),
        Err(Error::RevisionConflict)
    );
    let mut bad = test.clone();
    bad.task_id = "not-a-uuid".into();
    assert_eq!(
        exec(&mut s, 1, C::TestLlm(bad)),
        Err(Error::InvalidArgument)
    );
    exec(&mut s, 1, C::TestLlm(test)).unwrap();
    s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
        .unwrap();
    exec(
        &mut s,
        1,
        C::CancelLlm(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(91),
        }),
    )
    .unwrap();
    let mut late = reply(91, false);
    late.text = "NAI_OK".into();
    s.finish_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &late)
        .unwrap();
    let r = exec(&mut s, 1, read_task(91)).unwrap();
    assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
    assert_eq!(r.charge_status, "unknown");
    assert_eq!(r.body, "");
}

#[test]
fn unknown_model_price_test_has_no_cap_or_reservation_and_retains_send_audit() {
    for sent in [false, true] {
        let (_t, mut s) = init();
        s.attach_llm_vault(Arc::new(FixtureVault));
        let mut config = model(0);
        s.conn
            .execute("UPDATE workspaces SET budget_limit=0 WHERE id=?1", [id(10)])
            .unwrap();
        config.prices_confirmed = false;
        config.input_minor_per_million = 0;
        config.output_minor_per_million = 0;
        let saved = exec(&mut s, 1, C::SaveLlmModel(config)).unwrap();
        assert!(saved.llm_models[0].pricing_unknown);
        let r = exec(
            &mut s,
            1,
            C::TestLlm(FoundationModelTest {
                workspace_id: id(10),
                task_id: id(91),
                model_id: id(100),
                model_revision: 1,
            }),
        )
        .unwrap();
        assert_eq!(r.reserved_minor, 0);
        s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
            .unwrap();
        let mut response = reply(91, true);
        response.request_started = sent;
        response.text = "NAI_OK".into();
        s.finish_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &response)
            .unwrap();
        let r = exec(&mut s, 1, read_task(91)).unwrap();
        assert_eq!(r.charge_status, if sent { "unknown" } else { "not_sent" });
        assert_eq!(r.actual_minor, 0);
        assert_eq!(
            s.conn
                .query_row("SELECT COUNT(*) FROM budget_reservations", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn unknown_price_business_check_and_dispatch_use_the_explicit_call_cap() {
    let (_t, mut s, snapshot) = prepared();
    let mut config = model(1);
    config.prices_confirmed = false;
    config.input_minor_per_million = 0;
    config.output_minor_per_million = 0;
    exec(&mut s, 1, C::SaveLlmModel(config)).unwrap();
    let mut request = plan(&snapshot, 7);
    request.model_revision = 2;
    let check = checked(&mut s, request);
    assert_eq!(check.reason, LlmLocalReason::ReadyLocal as i32);
    assert!(!check.estimate_known);
    assert!(check.credential_checked);
    assert_eq!(check.estimated_reserve_minor, 7);
    let C::StartLlm(mut command) = start(&snapshot, 91) else {
        panic!("start command")
    };
    command.model_revision = 2;
    command.max_cost_minor = 7;
    let r = exec(&mut s, 1, C::StartLlm(command)).unwrap();
    assert_eq!(r.reserved_minor, 7);
}

#[test]
fn business_llm_uses_the_same_source_context_as_the_question_pool() {
    let (_t, mut s, snapshot) = prepared();
    let queued = exec(&mut s, 1, start(&snapshot, 91)).unwrap();
    let pool = exec(
        &mut s,
        1,
        C::ReadQuestionPool(QuestionPoolRead {
            workspace_id: id(10),
            snapshot_id: id(90),
            snapshot_sha256: snapshot.sha256,
        }),
    )
    .unwrap();
    assert_eq!(queued.agent_context, pool.question_pool.unwrap().context);
    assert!(queued.agent_context.is_some());
}
#[test]
fn missing_or_forged_llm_context_discards_late_text_but_settles_known_usage() {
    for case in 0..3 {
        let (_t, mut s, snapshot) = prepared();
        exec(&mut s, 1, start(&snapshot, 91)).unwrap();
        s.claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(91))
            .unwrap();
        let (mut v, rev, _) = crate::llm::raw(
            &s.conn,
            &s.root,
            &id(10),
            &id(91),
            0,
            "publication_llm_task",
        )
        .unwrap();
        match case {
            0 => {
                v.as_object_mut().unwrap().remove("task_context");
            }
            1 => v["task_context"]["context_sha256"] = "b".repeat(64).into(),
            _ => {
                v["task_context"] = nai_runtime_core::task_context::to_value(
                    &nai_runtime_core::task_context::source(
                        &id(10),
                        &id(999),
                        &snapshot.sha256,
                        &"a".repeat(64),
                    )
                    .unwrap(),
                );
            }
        }
        let tx = s.conn.transaction().unwrap();
        crate::llm::put(&tx, &s.root, &id(10), &id(91), rev, &v, utc().unwrap()).unwrap();
        tx.commit().unwrap();
        s.finish_llm(
            &Operation::new(4000).unwrap(),
            &id(1),
            &id(10),
            &reply(91, true),
        )
        .unwrap();
        let r = exec(&mut s, 1, read_task(91)).unwrap();
        assert!(r.body.is_empty());
        assert!(r.agent_context.is_none());
        assert_ne!(r.suggestion_state, SuggestionState::Proposed as i32);
        assert_eq!(r.charge_status, "known");
        assert!(r.llm_budget.unwrap().used_minor > 0);
    }
}

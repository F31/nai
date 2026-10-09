use super::*;
use nai_contracts::v1::ErrorCode;

fn entries(store: &Store, workspace: &str) -> i64 {
    store
        .conn
        .query_row(
            "SELECT COUNT(*) FROM decision_log WHERE workspace_id=?1",
            [workspace],
            |row| row.get(0),
        )
        .unwrap()
}

fn traces(store: &Store) -> Vec<String> {
    store
        .conn
        .prepare("SELECT trace_id FROM decision_log ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|row| row.unwrap())
        .collect()
}

#[test]
fn newer_telemetry_cannot_overtake_a_failed_buffer_head() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    storage.create_workspace(&op(), "w", "log").unwrap();
    storage
        .append_decision(
            &op(),
            "absent",
            None,
            DecisionKind::Event,
            &serde_json::json!({}),
            "old",
        )
        .unwrap();
    let ack = storage
        .append_decision(
            &op(),
            "w",
            None,
            DecisionKind::Event,
            &serde_json::json!({}),
            "new",
        )
        .unwrap();
    assert!(!ack.persisted);
    assert!(traces(&storage).is_empty());
    assert_eq!(storage.decision_buffer_stats().0, 2);
    storage.create_workspace(&op(), "absent", "log").unwrap();
    storage.flush_decisions(&op()).unwrap();
    assert_eq!(traces(&storage), ["old", "new"]);
}

#[test]
fn failed_flush_rolls_back_all_previous_inserts_and_keeps_buffer() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    for (workspace, trace) in [("w", "first"), ("absent", "second")] {
        storage
            .append_decision(
                &op(),
                workspace,
                None,
                DecisionKind::Event,
                &serde_json::json!({}),
                trace,
            )
            .unwrap();
    }
    storage.create_workspace(&op(), "w", "log").unwrap();
    assert!(storage.flush_decisions(&op()).is_err());
    assert!(traces(&storage).is_empty());
    assert_eq!(storage.decision_buffer_stats().0, 2);
    storage.create_workspace(&op(), "absent", "log").unwrap();
    storage.flush_decisions(&op()).unwrap();
    assert_eq!(traces(&storage), ["first", "second"]);
}

#[test]
fn new_telemetry_and_buffer_are_one_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    storage
        .append_decision(
            &op(),
            "w",
            None,
            DecisionKind::Event,
            &serde_json::json!({}),
            "buffered",
        )
        .unwrap();
    storage.create_workspace(&op(), "w", "log").unwrap();
    storage.conn.execute_batch("CREATE TEMP TRIGGER reject_new BEFORE INSERT ON decision_log WHEN NEW.trace_id='new' BEGIN SELECT RAISE(ABORT,'test injected failure'); END;").unwrap();
    let ack = storage
        .append_decision(
            &op(),
            "w",
            None,
            DecisionKind::Event,
            &serde_json::json!({}),
            "new",
        )
        .unwrap();
    assert!(!ack.persisted);
    assert!(traces(&storage).is_empty());
    assert_eq!(storage.decision_buffer_stats().0, 2);
    storage
        .conn
        .execute_batch("DROP TRIGGER reject_new;")
        .unwrap();
    let ack = storage
        .append_decision(
            &op(),
            "w",
            None,
            DecisionKind::Event,
            &serde_json::json!({}),
            "last",
        )
        .unwrap();
    assert!(ack.persisted);
    assert_eq!(traces(&storage), ["buffered", "new", "last"]);
    storage.verify_decision_chain(&op(), "w").unwrap();
}

#[test]
fn critical_write_does_not_drain_or_depend_on_telemetry_buffer() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    storage
        .append_decision(
            &op(),
            "w",
            None,
            DecisionKind::Event,
            &serde_json::json!({}),
            "buffered",
        )
        .unwrap();
    storage.create_workspace(&op(), "w", "log").unwrap();
    let ack = storage
        .append_decision(
            &op(),
            "w",
            None,
            DecisionKind::Approval,
            &serde_json::json!({}),
            "critical",
        )
        .unwrap();
    assert!(ack.persisted);
    assert_eq!(traces(&storage), ["critical"]);
    assert_eq!(storage.decision_buffer_stats().0, 1);
    storage.flush_decisions(&op()).unwrap();
    assert_eq!(traces(&storage), ["critical", "buffered"]);
    storage.verify_decision_chain(&op(), "w").unwrap();
}

#[test]
fn append_persists_and_chain_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    storage
        .create_workspace(&op(), "w", "decision log")
        .unwrap();
    let first = storage
        .append_decision(
            &op(),
            "w",
            Some("s"),
            DecisionKind::Event,
            &serde_json::json!({"type": "comment"}),
            "trace-1",
        )
        .unwrap();
    assert!(first.persisted);
    assert!(first.entry_hash.is_some());
    for (kind, trace) in [
        (DecisionKind::Observation, "trace-2"),
        (DecisionKind::Decision, "trace-3"),
        (DecisionKind::Guard, "trace-4"),
    ] {
        let ack = storage
            .append_decision(
                &op(),
                "w",
                Some("s"),
                kind,
                &serde_json::json!({"step": kind.as_str()}),
                trace,
            )
            .unwrap();
        assert!(ack.persisted);
    }
    assert_eq!(entries(&storage, "w"), 4);
    storage.verify_decision_chain(&op(), "w").unwrap();
    // Critical kinds persist the same way and stay in the same chain.
    storage
        .append_decision(
            &op(),
            "w",
            Some("s"),
            DecisionKind::Approval,
            &serde_json::json!({"candidate": "c1"}),
            "trace-5",
        )
        .unwrap();
    storage.verify_decision_chain(&op(), "w").unwrap();
    let (buffered, dropped) = storage.decision_buffer_stats();
    assert_eq!((buffered, dropped), (0, 0));
}

#[test]
fn forged_entry_and_mutation_are_detected() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    storage
        .create_workspace(&op(), "w", "decision log")
        .unwrap();
    storage
        .append_decision(
            &op(),
            "w",
            None,
            DecisionKind::Event,
            &serde_json::json!({"n": 1}),
            "trace-1",
        )
        .unwrap();
    // Bypass the API with a wrong prev_hash.
    storage
        .conn
        .execute(
            "INSERT INTO decision_log(workspace_id,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id) VALUES('w',NULL,'event','telemetry','{}','x','forged','forged',0,'forged')",
            [],
        )
        .unwrap();
    let failure = storage.verify_decision_chain(&op(), "w").unwrap_err();
    assert_eq!(failure.code, ErrorCode::Internal);
    assert_eq!(failure.position, Some(2), "reports the first divergent row");
    assert!(
        storage
            .conn
            .execute("UPDATE decision_log SET payload_json='{\"n\":2}'", [])
            .is_err()
    );
    assert!(
        storage
            .conn
            .execute("DELETE FROM decision_log", [])
            .is_err()
    );
    assert_eq!(entries(&storage, "w"), 2);
}

#[test]
fn critical_failures_surface_and_never_enter_the_buffer() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    for kind in [
        DecisionKind::Approval,
        DecisionKind::Cue,
        DecisionKind::Cancellation,
        DecisionKind::DutyLease,
    ] {
        assert_eq!(kind.criticality(), Criticality::Critical);
        assert!(
            storage
                .append_decision(&op(), "absent", None, kind, &serde_json::json!({}), "t")
                .is_err()
        );
    }
    assert_eq!(storage.decision_buffer_stats(), (0, 0));
}

#[test]
fn telemetry_failures_buffer_bounded_and_flush_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    // The workspace does not exist yet, so every telemetry write fails.
    let ack = storage
        .append_decision(
            &op(),
            "late",
            None,
            DecisionKind::Event,
            &serde_json::json!({"n": 1}),
            "t1",
        )
        .unwrap();
    assert!(!ack.persisted);
    assert!(ack.entry_hash.is_none());
    assert!(!ack.dropped_oldest);
    assert_eq!(storage.decision_buffer_stats(), (1, 0));
    for index in 0..TELEMETRY_BUFFER_CAPACITY {
        let ack = storage
            .append_decision(
                &op(),
                "late",
                None,
                DecisionKind::Usage,
                &serde_json::json!({"n": index}),
                "t",
            )
            .unwrap();
        assert!(!ack.persisted);
        assert_eq!(ack.dropped_oldest, index + 1 >= TELEMETRY_BUFFER_CAPACITY);
    }
    let (buffered, dropped) = storage.decision_buffer_stats();
    assert_eq!(buffered, TELEMETRY_BUFFER_CAPACITY);
    assert_eq!(dropped, 1, "overflow drops exactly the oldest entry");
    // Once the workspace exists the buffer flushes and the chain verifies.
    storage
        .create_workspace(&op(), "late", "appeared later")
        .unwrap();
    storage.flush_decisions(&op()).unwrap();
    assert_eq!(storage.decision_buffer_stats(), (0, 1));
    assert_eq!(entries(&storage, "late"), TELEMETRY_BUFFER_CAPACITY as i64);
    storage.verify_decision_chain(&op(), "late").unwrap();
}

#[test]
fn workspaces_keep_independent_chains() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    for workspace in ["w1", "w2"] {
        storage
            .create_workspace(&op(), workspace, "decision log")
            .unwrap();
        storage
            .append_decision(
                &op(),
                workspace,
                None,
                DecisionKind::Event,
                &serde_json::json!({"ws": workspace}),
                "t1",
            )
            .unwrap();
        storage
            .append_decision(
                &op(),
                workspace,
                None,
                DecisionKind::Decision,
                &serde_json::json!({"ws": workspace}),
                "t2",
            )
            .unwrap();
        storage.verify_decision_chain(&op(), workspace).unwrap();
    }
    let genesis_of_w2: String = storage
        .conn
        .query_row(
            "SELECT prev_hash FROM decision_log WHERE workspace_id='w2' ORDER BY seq LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(genesis_of_w2, GENESIS_HASH);
    let first_w1: String = storage
        .conn
        .query_row(
            "SELECT entry_hash FROM decision_log WHERE workspace_id='w1' ORDER BY seq LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_ne!(first_w1, genesis_of_w2);
}

#[test]
fn oversized_payload_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    storage
        .create_workspace(&op(), "w", "decision log")
        .unwrap();
    let huge = serde_json::json!({"blob": "x".repeat(70000)});
    assert_eq!(
        storage
            .append_decision(&op(), "w", None, DecisionKind::Event, &huge, "t")
            .unwrap_err()
            .code,
        ErrorCode::ResourceExhausted
    );
    assert_eq!(entries(&storage, "w"), 0);
}

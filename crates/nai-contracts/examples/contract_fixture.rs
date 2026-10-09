use nai_contracts::v1::*;
use prost::Message;
use std::{collections::BTreeMap, fs, path::Path};

const EPOCH: u64 = 9_007_199_254_740_993;

fn transfer<M: Message + Default + PartialEq + std::fmt::Debug>(
    dir: &Path,
    name: &str,
    expected: M,
    write: bool,
) {
    let path = dir.join(format!("{name}.bin"));
    if write {
        let mut bytes = expected.encode_to_vec();
        bytes.extend([0x98, 0x06, 0x07]); // unknown future field 99
        fs::write(path, bytes).unwrap();
    } else {
        assert_eq!(
            M::decode(fs::read(path).unwrap().as_slice()).unwrap(),
            expected,
            "cross-language {name}"
        );
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "contract_fixture write|verify directory");
    let write = match args[1].as_str() {
        "write" => true,
        "verify" => false,
        _ => panic!("unknown fixture operation"),
    };
    let dir = Path::new(&args[2]);
    fs::create_dir_all(dir).unwrap();
    transfer(
        dir,
        "envelope",
        Envelope {
            protocol_major: 1,
            message_type: "nai.v1.ControlCommand".into(),
            message_id: "msg-fixture".into(),
            request_id: "req-fixture".into(),
            trace_id: "trace-fixture".into(),
            session_id: "session-fixture".into(),
            generation_epoch: Some(EPOCH),
            sent_at_utc_ms: Some(u64::MAX),
            payload: vec![8, 1],
            remaining_budget_ms: 500,
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "cue",
        PerformanceCue {
            session_id: "session-fixture".into(),
            utterance_id: "utterance-fixture".into(),
            generation_epoch: EPOCH,
            display_text: "测试：一千二百九十九元".into(),
            speech_text: "测试：一千二百九十九元".into(),
            expires_at_utc_ms: u64::MAX,
            risk: 2,
            candidate_hash: "a".repeat(64),
            fact_versions: BTreeMap::from([("sku".into(), "rev-3".into())]),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "audio",
        AudioBlockMetadata {
            session_id: "session-fixture".into(),
            utterance_id: "utterance-fixture".into(),
            generation_epoch: EPOCH,
            seq: EPOCH,
            sample_start: u64::MAX,
            sample_count: 480,
            buffer_ref: "test-only-buffer".into(),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "permissions",
        PermissionSnapshot {
            revision: "test-only".into(),
            capabilities: BTreeMap::from([("unknown-test".into(), 77), ("known-denied".into(), 2)]),
            valid_until_utc_ms: Some(u64::MAX),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "error",
        ContractError {
            request_id: "req-fixture".into(),
            code: ErrorCode::Cancelled as i32,
            category: "cancellation".into(),
            message: "测试取消".into(),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "stream",
        CapabilityResult {
            request_id: "req-fixture".into(),
            seq: 1,
            end: StreamEnd::StreamError as i32,
            error: Some(ContractError {
                code: ErrorCode::DeadlineExceeded as i32,
                ..Default::default()
            }),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "fact",
        AuthoritativeFact {
            workspace_id: "test-workspace".into(),
            product_id: "test-sku".into(),
            field_key: "price".into(),
            revision: "rev-test".into(),
            value: Some(authoritative_fact::Value::Money(Money {
                minor_units: 129900,
                currency: "CNY".into(),
                scale: 2,
            })),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "prepare",
        PrepareReport {
            readiness: 3,
            snapshot: Some(BindingSnapshot {
                snapshot_id: "test-boot:session-fixture:1".into(),
                session_id: "session-fixture".into(),
                revision: EPOCH.to_string(),
                bindings: vec![Binding {
                    capability_id: "guard.check@1".into(),
                    primary_plugin_id: "test-guard".into(),
                    selected: Some(BoundProvider {
                        plugin_id: "test-guard".into(),
                        plugin_version: "0.1.0".into(),
                        artifact_hash: "a".repeat(64),
                        manifest_hash: "b".repeat(64),
                        host: 1,
                        policy_scope_ref: "test-scope".into(),
                    }),
                    ..Default::default()
                }],
            }),
            issues: vec![BindingIssue {
                capability_id: "memory.access@1".into(),
                code: ErrorCode::CapabilityAbsent as i32,
                reason_code: "test-only-missing".into(),
                required: false,
                ..Default::default()
            }],
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "ledger",
        UsageLedgerEntry {
            workspace_id: "test-workspace".into(),
            session_id: "session-fixture".into(),
            provider_ref: "test-provider".into(),
            request_id: "test-request".into(),
            generation_epoch: EPOCH,
            estimate: Some(BudgetCharge {
                currency: "CNY".into(),
                cost_minor: u64::MAX,
                units: BTreeMap::from([("requests".into(), 1), ("tokens".into(), EPOCH)]),
                price_revision: "price-test".into(),
            }),
            state: BillingState::NeedsReconciliation as i32,
            cancelled: true,
            revision: EPOCH,
            concurrent: BTreeMap::from([("provider_slots".into(), 1)]),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "platform",
        PlatformEvent {
            event_id: "test-platform-event".into(),
            generation_epoch: Some(EPOCH),
            connection_generation: u64::MAX,
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "watchdog_command",
        OutputWatchdogCommand {
            schema_version: 1,
            session_id: "fixture-session".into(),
            seq: u64::MAX,
            remaining_budget_ms: 250,
            action: Some(output_watchdog_command::Action::Arm(PlaybackLease {
                session_id: "fixture-session".into(),
                generation_epoch: EPOCH,
                mode: SessionMode::Rehearsal as i32,
                ..Default::default()
            })),
        },
        write,
    );
    transfer(
        dir,
        "watchdog_status",
        OutputWatchdogStatus {
            schema_version: 1,
            session_id: "fixture-session".into(),
            epoch_floor: EPOCH,
            last_command_seq: u64::MAX,
            output_available: false,
            skipped_statuses: u64::MAX,
            backend_error: Some(ContractError {
                code: ErrorCode::CapabilityNotReady as i32,
                message: "fixture-not-hardware".into(),
                ..Default::default()
            }),
            ..Default::default()
        },
        write,
    );
    transfer(
        dir,
        "frame_slot",
        FrameSlotEvent {
            event: Some(frame_slot_event::Event::Committed(Box::new(
                FrameSlotCommit {
                    ticket: Some(FrameSlotTicket {
                        schema_version: 1,
                        pool_id: "test-pool".into(),
                        session_id: "fixture-session".into(),
                        generation_epoch: EPOCH,
                        slot_index: 15,
                        slot_generation: u64::MAX,
                        producer_ref: "test-producer".into(),
                        consumer_ref: "test-consumer".into(),
                        max_slot_bytes: 8 * 1024 * 1024,
                        remaining_budget_ms: 250,
                        frame: Some(FrameMetadata {
                            session_id: "fixture-session".into(),
                            utterance_id: "fixture-utterance".into(),
                            generation_epoch: EPOCH,
                            frame_seq: u64::MAX,
                            pts: i64::MAX,
                            time_base_num: 1,
                            time_base_den: 8000,
                            duration: 320,
                            width: 2,
                            height: 1,
                            pixel_format: "rgba8".into(),
                            buffer_ref: "test-pool:15:18446744073709551615".into(),
                            ..Default::default()
                        }),
                    }),
                    byte_length: 8,
                    sha256: "a".repeat(64),
                },
            ))),
        },
        write,
    );
    println!(
        "Rust cross-language fixture {} passed (13 message types)",
        args[1]
    );
}

//! Real-process rehearsal pipeline tests: a manual event reaches the decision
//! log through the real store, the chain verifies, the stub replay matches the
//! logged decisions, and an injected stage failure halts in order.
//!
//! A script may declare a render closure. With one, the real speech seam, the
//! real software 2D renderer and the playback lane run end to end, and only a
//! verified actual-playback completion is logged as a `PlaybackAck`. Without
//! one the chain honestly stops at text.
//!
//! These are Linux/Windows engineering drills. They certify no real provider, no
//! real TTS, no frame slot, no keeper consume and no device: frames stay in
//! memory and the output identity is a rehearsal sink, not hardware.
mod common;

use common::{
    ACTOR, PROFILE, WORKSPACE, open_store, runtime_profile, store_operation, write_avatar,
    write_token,
};
use nai_engine::pipeline::{self, Script};
use nai_storage::DecisionKind;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::tempdir;

/// Far-future candidate/approval expiry: the rehearsal runs in real wall-clock
/// time, so these must not lapse mid-test.
const FAR_FUTURE_MS: u64 = 4_000_000_000_000;
/// The epoch a fresh session holds after the declared bring-up. A session starts
/// at epoch 0, and `created -> preparing` revokes (epoch + 1); `ready` and
/// `running` are state-revision only, and `assisted` is already the session's
/// initial mode, so no mode change revokes either. Events must carry this epoch
/// or they are correctly refused as stale.
const REHEARSAL_LIVE_EPOCH: u64 = 1;

struct Run {
    status: Option<i32>,
    pipeline: Option<Value>,
    stderr: String,
}

impl Run {
    fn assert_failed(&self) {
        assert_eq!(
            self.status,
            Some(2),
            "an injected stage failure must exit 2; stderr={}",
            self.stderr
        );
        assert!(
            self.pipeline.is_none(),
            "a failed run must not print a success line"
        );
    }
}

/// Provisions through the engine's own trusted-owner path.
fn provision_with_engine(root: &Path) {
    let profile_path = root.join("runtime-profile.json");
    std::fs::write(&profile_path, runtime_profile(WORKSPACE).to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nai-engine"))
        .args([
            "--data-root",
            &root.display().to_string(),
            "--workspace",
            WORKSPACE,
            "--actor",
            ACTOR,
            "--profile",
            PROFILE,
            "--provision",
            &profile_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "provisioning failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<i32> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().unwrap() {
            Some(status) => return status.code(),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Runs the engine once and returns its exit status plus the pipeline summary
/// line. The engine prints its control address first and the pipeline line
/// second, then waits for the halt file.
fn run_engine(root: &Path, script: &Value, inject: Option<&str>) -> Run {
    write_token(&root.join("token"));
    let script_path = root.join("script.json");
    std::fs::write(&script_path, script.to_string()).unwrap();
    let halt = root.join("halt");
    let mut args: Vec<String> = vec![
        "--data-root".into(),
        root.display().to_string(),
        "--workspace".into(),
        WORKSPACE.into(),
        "--actor".into(),
        ACTOR.into(),
        "--profile".into(),
        PROFILE.into(),
        "--token-file".into(),
        root.join("token").display().to_string(),
        "--halt-file".into(),
        halt.display().to_string(),
        "--control-port".into(),
        "0".into(),
        "--event-file".into(),
        script_path.display().to_string(),
        "--rehearsal-provider".into(),
    ];
    if let Some(stage) = inject {
        args.push("--inject".into());
        args.push(stage.into());
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_nai-engine"))
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("engine binary spawns");
    let stdout = child.stdout.take().unwrap();
    let mut stderr_pipe = child.stderr.take().unwrap();
    let stderr_handle = thread::spawn(move || {
        let mut buffer = String::new();
        let _ = stderr_pipe.read_to_string(&mut buffer);
        buffer
    });
    let mut reader = BufReader::new(stdout);
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut pipeline = None;
    while Instant::now() < deadline {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(rest) = line.trim().strip_prefix("nai-engine pipeline: ") {
                    pipeline = serde_json::from_str::<Value>(rest).ok();
                    break;
                }
            }
            Err(_) => break,
        }
    }
    if pipeline.is_some() {
        std::fs::write(&halt, b"halt").unwrap();
    }
    let status = wait_for_exit(&mut child, Duration::from_secs(20));
    if pipeline.is_none() {
        let mut sink = String::new();
        let _ = reader.read_to_string(&mut sink);
    }
    Run {
        status,
        pipeline,
        stderr: stderr_handle.join().unwrap_or_default(),
    }
}

fn knowledge() -> Value {
    json!([
        {
            "document_id": "kb-doc-1",
            "chunk_id": "kb-1",
            "text": "商品价格是 199 元",
            "revision": "kb-doc-1@1",
            "permission_revision": "kb-rev-1"
        },
        {
            "document_id": "kb-doc-2",
            "chunk_id": "kb-2",
            "text": "发货时间是三天内到达",
            "revision": "kb-doc-2@1",
            "permission_revision": "kb-rev-1"
        }
    ])
}

fn event(id: &str, text: &str, epoch: u64) -> Value {
    json!({
        "event_id": id,
        "utterance_id": format!("utt-{id}"),
        "source": "manual",
        "kind": "comment",
        "text": text,
        "generation_epoch": epoch,
        "expires_at_utc_ms": FAR_FUTURE_MS
    })
}

/// Builds a script. `approved` decides whether the operator approval records are
/// computed from the engine's own candidate preview (the approval draft a
/// trusted owner would review).
fn script(events: Vec<Value>, approved: bool) -> Value {
    let mut value = json!({
        "schema_version": 1,
        "session_id": WORKSPACE,
        "workspace_id": WORKSPACE,
        "knowledge_revision": "kb-rev-1",
        "top_k": 3,
        "session": {
            "operator": "local-operator",
            "duty_ttl_ms": 300_000,
            "mode": "assisted",
            "lifecycle": ["preparing", "ready", "running"]
        },
        "knowledge": knowledge(),
        "events": events,
        "approvals": []
    });
    if approved {
        let parsed: Script = serde_json::from_value(value.clone()).expect("script parses");
        let previews = pipeline::preview_candidates(&parsed).expect("candidates preview");
        value["approvals"] = json!(
            previews
                .iter()
                .map(|preview| json!({
                    "utterance_id": preview.utterance_id,
                    "approval_id": format!("ap-{}", preview.event_id),
                    "candidate_hash": preview.candidate_hash,
                    "expires_at_utc_ms": FAR_FUTURE_MS
                }))
                .collect::<Vec<_>>()
        );
    }
    value
}

fn outcome_of<'a>(summary: &'a Value, event_id: &str) -> &'a Value {
    summary["outcomes"]
        .as_array()
        .expect("outcomes")
        .iter()
        .find(|outcome| outcome["event_id"] == event_id)
        .unwrap_or_else(|| panic!("no outcome for {event_id}: {summary}"))
}

fn stage_of(summary: &Value, event_id: &str) -> String {
    outcome_of(summary, event_id)["stage"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The render closure the operator wrote plus the real RGBA assets it names.
/// The closure's relative paths resolve against the script's own directory, so
/// the assets are written next to `script.json` in the run root.
fn render_script(root: &Path, events: Vec<Value>, playback: &str) -> Value {
    let files = write_avatar(root);
    assert!(
        files.manifest.is_file() && files.config.is_file() && files.assets.len() == 3,
        "the render assets must exist before the engine is asked to admit them"
    );
    let mut value = script(events, true);
    value["render"] = json!({
        "manifest": "manifest.json",
        "config": "config.json",
        "assets": [
            {"id": "base", "path": "base.rgba"},
            {"id": "closed", "path": "closed.rgba"},
            {"id": "open", "path": "open.rgba"}
        ],
        "licenses": ["test-license"],
        "voice_ref": "voice-rehearsal",
        "output_id": "engine-rehearsal-out",
        "playback": playback
    });
    value
}

/// The playback decisions the pipeline logged, in chain order.
fn decision_entries(root: &Path) -> Vec<(String, String, String)> {
    let store = open_store(root);
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .expect("the decision chain verifies");
    store
        .decision_entries(&store_operation(), WORKSPACE, 100)
        .unwrap()
        .into_iter()
        .map(|entry| (entry.trace_id, entry.kind, entry.payload_json))
        .collect()
}

#[test]
fn approved_manual_event_reaches_the_log_with_a_verified_chain_and_matching_replay() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &script(
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            true,
        ),
        None,
    );
    let summary = run.pipeline.expect("pipeline summary");
    assert_eq!(summary["generated"], 1, "{summary}");
    assert_eq!(summary["chain_verified"], true, "{summary}");
    assert_eq!(summary["replay_matched"], true, "{summary}");
    assert_eq!(summary["rehearsal"], true);
    assert_eq!(summary["real_provider"], false);
    assert_eq!(summary["devices"], false);
    assert_eq!(summary["playback_ack"], false);
    assert_eq!(summary["output_available"], false);
    assert_eq!(summary["session_lifecycle"], "running", "{summary}");
    assert_eq!(summary["session_mode"], "assisted", "{summary}");
    assert_eq!(summary["session_epoch"], REHEARSAL_LIVE_EPOCH, "{summary}");
    assert_eq!(summary["duty_operator"], "local-operator", "{summary}");
    assert_eq!(stage_of(&summary, "ev-1"), "generated");
    assert_eq!(run.status, Some(0), "engine halts cleanly: {}", run.stderr);

    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .expect("the decision chain verifies");
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 100)
        .unwrap();
    let generated = entries
        .iter()
        .find(|entry| entry.trace_id == "engine-pipeline-ev-1")
        .expect("the pipeline entry is persisted");
    assert_eq!(generated.kind, DecisionKind::Synthesis.as_str());
    assert!(
        generated
            .payload_json
            .contains("rehearsal-deterministic-v1"),
        "the rehearsal model identity is recorded: {}",
        generated.payload_json
    );
    assert!(
        generated.payload_json.contains("[rehearsal:"),
        "the generated text is recorded: {}",
        generated.payload_json
    );
    assert!(
        generated.payload_json.contains("\"delivery\":\"none\""),
        "the pipeline never claims delivery: {}",
        generated.payload_json
    );
}

#[test]
fn guard_block_silent_and_stale_paths_never_generate() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    // ev-block carries an unchecked promise; ev-miss has no knowledge hit
    // ("你好啊朋友们" shares no token with the corpus); ev-stale carries an epoch
    // the live session does not hold.
    let run = run_engine(
        root.path(),
        &script(
            vec![
                event("ev-ok", "商品价格是多少", REHEARSAL_LIVE_EPOCH),
                event("ev-block", "我们保证价格最优", REHEARSAL_LIVE_EPOCH),
                event("ev-miss", "你好啊朋友们", REHEARSAL_LIVE_EPOCH),
                event("ev-stale", "商品价格是多少", REHEARSAL_LIVE_EPOCH + 4),
            ],
            true,
        ),
        None,
    );
    let summary = run.pipeline.expect("pipeline summary");
    assert_eq!(summary["generated"], 1, "{summary}");
    assert_eq!(summary["chain_verified"], true, "{summary}");
    assert_eq!(summary["replay_matched"], true, "{summary}");
    assert_eq!(stage_of(&summary, "ev-ok"), "generated");
    assert_eq!(stage_of(&summary, "ev-block"), "guard_output");
    assert_eq!(stage_of(&summary, "ev-miss"), "knowledge_no_hit");
    assert_eq!(stage_of(&summary, "ev-stale"), "stale_epoch");
    assert_eq!(run.status, Some(0), "{}", run.stderr);
}

#[test]
fn missing_approval_blocks_at_preplay_before_generation() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &script(
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            false,
        ),
        None,
    );
    let summary = run.pipeline.expect("pipeline summary");
    assert_eq!(summary["generated"], 0, "{summary}");
    assert_eq!(stage_of(&summary, "ev-1"), "guard_preplay");
    assert_eq!(summary["chain_verified"], true, "{summary}");
    assert_eq!(summary["replay_matched"], true, "{summary}");
}

#[test]
fn a_second_process_reproduces_the_same_log_digest() {
    let first_root = tempdir().unwrap();
    let second_root = tempdir().unwrap();
    provision_with_engine(first_root.path());
    provision_with_engine(second_root.path());
    let script = script(
        vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
        true,
    );
    let first = run_engine(first_root.path(), &script, None)
        .pipeline
        .expect("first summary");
    let second = run_engine(second_root.path(), &script, None)
        .pipeline
        .expect("second summary");
    assert_eq!(first["generated"], 1, "{first}");
    assert_eq!(
        first["digest"], second["digest"],
        "the same script must reproduce the same logged decisions"
    );
}

#[test]
fn injected_generation_failure_halts_in_order_without_a_success_line() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &script(
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            true,
        ),
        Some("generate"),
    );
    run.assert_failed();
    assert!(
        run.stderr.contains("generation failed"),
        "the failure is reported: {}",
        run.stderr
    );
    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .expect("the chain survives the halt");
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 100)
        .unwrap();
    assert!(
        entries.iter().any(|entry| {
            entry.kind == DecisionKind::Cancellation.as_str()
                && entry.payload_json.contains("\"action\":\"mute\"")
        }),
        "the ordered halt must log a critical cancellation: {entries:?}"
    );
}

#[test]
fn injected_knowledge_failure_halts_in_order() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &script(
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            true,
        ),
        Some("knowledge"),
    );
    run.assert_failed();
    assert!(
        run.stderr.contains("knowledge-stage failure"),
        "the failure is reported: {}",
        run.stderr
    );
}

#[test]
fn injected_storage_failure_does_not_roll_back_the_mute() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &script(
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            true,
        ),
        Some("storage"),
    );
    run.assert_failed();
    assert!(
        run.stderr.contains("decision log write failed"),
        "the storage failure is reported: {}",
        run.stderr
    );
    let store = open_store(root.path());
    store
        .verify_decision_chain(&store_operation(), WORKSPACE)
        .expect("the chain survives the failed write");
    let entries = store
        .decision_entries(&store_operation(), WORKSPACE, 100)
        .unwrap();
    assert!(
        entries.iter().any(|entry| {
            entry.kind == DecisionKind::Cancellation.as_str()
                && entry.payload_json.contains("\"action\":\"mute\"")
        }),
        "the mute issued before the failing write must remain logged: {entries:?}"
    );
    assert!(
        !entries
            .iter()
            .any(|entry| entry.trace_id == "engine-pipeline-ev-1"),
        "the rejected write must not appear in the log"
    );
}

/// The fixture avatar renders at 8 kHz and 25 fps: one frame spans exactly 320
/// samples. A frame total is therefore a *derived* fact, and it was declared
/// before the first frame existed.
const REHEARSAL_SAMPLE_RATE: u32 = 8000;
const REHEARSAL_FPS_NUM: u32 = 25;
const REHEARSAL_FPS_DEN: u32 = 1;

/// The declared sample count of the rehearsal speech stub is bounded by
/// `60 ms per character`, clamped to `[400 ms, 5000 ms]`. Both ends are hard
/// properties of the seam, so a run that produced audio must sit inside them.
const MIN_REHEARSAL_SAMPLES: u64 = 400 * REHEARSAL_SAMPLE_RATE as u64 / 1000;
const MAX_REHEARSAL_SAMPLES: u64 = 5000 * REHEARSAL_SAMPLE_RATE as u64 / 1000;

#[test]
fn an_approved_event_plays_through_the_lane_to_a_verified_acknowledgement() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &render_script(
            root.path(),
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            "complete",
        ),
        None,
    );
    let summary = run.pipeline.expect("pipeline summary");
    assert_eq!(summary["generated"], 1, "{summary}");
    assert_eq!(summary["played"], 1, "{summary}");
    assert_eq!(summary["chain_verified"], true, "{summary}");
    assert_eq!(summary["replay_matched"], true, "{summary}");

    // The drill declares its own boundaries instead of overclaiming.
    assert_eq!(summary["playback_ack"], true, "{summary}");
    assert_eq!(summary["playback_scenario"], "complete", "{summary}");
    assert_eq!(summary["playback_sink"], "rehearsal-sink", "{summary}");
    assert_eq!(summary["renderer"], "software-rgba8-v1", "{summary}");
    assert_eq!(
        summary["speech"], "rehearsal-speech-deterministic-v1",
        "{summary}"
    );
    assert_eq!(summary["delivery"], "memory", "{summary}");
    assert_eq!(summary["real_provider"], false, "{summary}");
    assert_eq!(summary["real_tts"], false, "{summary}");
    assert_eq!(summary["devices"], false, "{summary}");
    assert_eq!(summary["frame_slot"], false, "{summary}");
    assert_eq!(summary["keeper_consume"], false, "{summary}");
    assert_eq!(summary["output_available"], false, "{summary}");

    assert_eq!(stage_of(&summary, "ev-1"), "played", "{summary}");
    let outcome = outcome_of(&summary, "ev-1");
    assert_eq!(outcome["playback"], "completed", "{outcome}");
    assert_eq!(outcome["delivery"], "memory", "{outcome}");
    assert_eq!(outcome["playback_rejected"], 0, "{outcome}");
    let samples = outcome["audio_samples"].as_u64().unwrap();
    assert!(
        (MIN_REHEARSAL_SAMPLES..=MAX_REHEARSAL_SAMPLES).contains(&samples),
        "the stub's bounded duration must hold: {outcome}"
    );
    // The frames the renderer produced are exactly the frames the declared
    // sample total implies. This is the completion criterion's real content:
    // the total was known before the first frame, and it was met.
    assert_eq!(
        outcome["frames"].as_u64().unwrap() as u32,
        nai_media_core::playback::frame_count(
            samples,
            REHEARSAL_SAMPLE_RATE,
            REHEARSAL_FPS_NUM,
            REHEARSAL_FPS_DEN
        ),
        "the produced frame total must match the declared audio total: {outcome}"
    );
    assert_eq!(
        run.status,
        Some(0),
        "the engine halts cleanly: {}",
        run.stderr
    );

    let entries = decision_entries(root.path());
    let played = entries
        .iter()
        .find(|(trace_id, _, _)| trace_id == "engine-pipeline-ev-1")
        .expect("the pipeline entry is persisted");
    assert_eq!(played.1, DecisionKind::PlaybackAck.as_str());
    assert!(
        played.2.contains("\"playback\":\"completed\""),
        "the acknowledgement records the completion: {}",
        played.2
    );
    assert!(
        played.2.contains("\"delivery\":\"memory\""),
        "the acknowledgement records that frames stayed in memory: {}",
        played.2
    );
    assert!(
        played.2.contains("rehearsal-speech-deterministic-v1"),
        "the speech identity is recorded: {}",
        played.2
    );
    assert_eq!(
        entries
            .iter()
            .filter(|(_, kind, _)| kind == DecisionKind::PlaybackAck.as_str())
            .count(),
        1,
        "exactly one completion may be logged: {entries:?}"
    );
}

#[test]
fn a_playback_that_did_not_complete_never_logs_an_acknowledgement() {
    // Each scenario is a *normal* ending, not a fault: the lane must survive it
    // honestly and must never claim the audience heard the utterance.
    for (scenario, stage) in [
        ("partial", "playback_incomplete"),
        ("invalid", "playback_invalid"),
        ("cancelled", "playback_cancelled"),
    ] {
        let root = tempdir().unwrap();
        provision_with_engine(root.path());
        let run = run_engine(
            root.path(),
            &render_script(
                root.path(),
                vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
                scenario,
            ),
            None,
        );
        let summary = run
            .pipeline
            .unwrap_or_else(|| panic!("{scenario}: no summary, stderr={}", run.stderr));
        assert_eq!(summary["generated"], 1, "{scenario}: {summary}");
        assert_eq!(summary["played"], 0, "{scenario}: {summary}");
        assert_eq!(summary["playback_ack"], false, "{scenario}: {summary}");
        assert_eq!(summary["playback_scenario"], scenario, "{summary}");
        assert_eq!(stage_of(&summary, "ev-1"), stage, "{scenario}: {summary}");
        assert_ne!(
            outcome_of(&summary, "ev-1")["playback"],
            "completed",
            "{scenario}: an incomplete lane must never report completion: {summary}"
        );
        assert_eq!(run.status, Some(0), "{scenario}: {}", run.stderr);

        let entries = decision_entries(root.path());
        assert!(
            !entries
                .iter()
                .any(|(_, kind, _)| kind == DecisionKind::PlaybackAck.as_str()),
            "{scenario}: a playback that did not complete must not be logged as an acknowledgement: {entries:?}"
        );
        let entry = entries
            .iter()
            .find(|(trace_id, _, _)| trace_id == "engine-pipeline-ev-1")
            .expect("the pipeline entry is persisted");
        assert_eq!(entry.1, DecisionKind::Synthesis.as_str(), "{scenario}");
        assert!(
            !entry.2.contains("\"playback\":\"completed\""),
            "{scenario}: {}",
            entry.2
        );
    }
}

#[test]
fn injected_speech_failure_halts_in_order_after_the_text_stage() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &render_script(
            root.path(),
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            "complete",
        ),
        Some("speech"),
    );
    run.assert_failed();
    assert!(
        run.stderr.contains("synthesis failed"),
        "the speech-stage failure is reported: {}",
        run.stderr
    );
    let entries = decision_entries(root.path());
    assert!(
        entries.iter().any(|(_, kind, payload)| {
            kind == DecisionKind::Cancellation.as_str() && payload.contains("\"action\":\"mute\"")
        }),
        "the ordered halt must log a critical cancellation: {entries:?}"
    );
    assert!(
        !entries
            .iter()
            .any(|(trace_id, _, _)| trace_id == "engine-pipeline-ev-1"),
        "a halted run must not record the event it failed on: {entries:?}"
    );
}

#[test]
fn a_speech_injection_without_a_render_closure_is_refused() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &script(
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            true,
        ),
        Some("speech"),
    );
    run.assert_failed();
    assert!(
        run.stderr
            .contains("--inject speech requires a script with a render closure"),
        "an injection that could never fire must be refused, not ignored: {}",
        run.stderr
    );
}

#[test]
fn a_render_closure_that_cannot_be_admitted_fails_the_run_not_an_event() {
    let root = tempdir().unwrap();
    provision_with_engine(root.path());

    // An unreadable declared input is a hard failure: no asset is ever defaulted.
    let mut missing = render_script(
        root.path(),
        vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
        "complete",
    );
    missing["render"]["manifest"] = json!("absent-manifest.json");
    let run = run_engine(root.path(), &missing, None);
    run.assert_failed();
    assert!(
        run.stderr.contains("avatar manifest unreadable"),
        "a missing manifest is reported: {}",
        run.stderr
    );

    // An unlicensed avatar is refused by the renderer itself, not smoothed over.
    let unlicensed_root = tempdir().unwrap();
    provision_with_engine(unlicensed_root.path());
    let mut unlicensed = render_script(
        unlicensed_root.path(),
        vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
        "complete",
    );
    unlicensed["render"]["licenses"] = json!(["some-other-license"]);
    let run = run_engine(unlicensed_root.path(), &unlicensed, None);
    run.assert_failed();
    assert!(
        run.stderr.contains("avatar license not granted by root"),
        "an unlicensed closure is refused at admission: {}",
        run.stderr
    );
}

#[test]
fn the_render_lane_does_not_change_a_script_that_declares_none() {
    // The text chain is the archived behaviour and must stay byte-identical: a
    // script without a render closure still logs the same digest and never
    // claims audio, frames or playback.
    let root = tempdir().unwrap();
    provision_with_engine(root.path());
    let run = run_engine(
        root.path(),
        &script(
            vec![event("ev-1", "商品价格是多少", REHEARSAL_LIVE_EPOCH)],
            true,
        ),
        None,
    );
    let summary = run.pipeline.expect("pipeline summary");
    assert_eq!(summary["generated"], 1, "{summary}");
    assert_eq!(summary["played"], 0, "{summary}");
    assert_eq!(summary["playback_ack"], false, "{summary}");
    assert_eq!(summary["delivery"], "none", "{summary}");
    assert_eq!(summary["speech"], Value::Null, "{summary}");
    assert_eq!(summary["renderer"], Value::Null, "{summary}");
    assert_eq!(summary["playback_sink"], Value::Null, "{summary}");
    assert_eq!(stage_of(&summary, "ev-1"), "generated", "{summary}");
    let outcome = outcome_of(&summary, "ev-1");
    assert_eq!(outcome["audio_samples"], 0, "{outcome}");
    assert_eq!(outcome["frames"], 0, "{outcome}");
    assert_eq!(outcome["playback"], "none", "{outcome}");
    assert_eq!(outcome["delivery"], "none", "{outcome}");
    let entries = decision_entries(root.path());
    let entry = entries
        .iter()
        .find(|(trace_id, _, _)| trace_id == "engine-pipeline-ev-1")
        .expect("the pipeline entry is persisted");
    assert_eq!(entry.1, DecisionKind::Synthesis.as_str());
    assert!(
        !entries
            .iter()
            .any(|(_, kind, _)| kind == DecisionKind::PlaybackAck.as_str()),
        "a text-only chain must not log a playback acknowledgement: {entries:?}"
    );
}

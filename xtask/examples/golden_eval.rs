//! Golden-set evaluation runner: the step between the case set and the kernel's
//! judgement.
//!
//! The kernel decides (W30) and the payload gate reads (W31), but nothing drove
//! a real set through anything. This example is that driver. It reads the case
//! sets the payload gate would accept, asks the **real** text model every
//! question through the bound stack (`ProviderProfile` -> `ProviderClient` ->
//! adapter -> budget gateway), and judges each answer with the kernel's own
//! `GoldenCase::answer_verdict`.
//!
//! ## What it drives, and what it does not
//!
//! It drives the **generation side only**: question in, answer out, judged
//! against the case's expected answer. It does **not** drive the behaviour
//! side — retrieval against a served knowledge base, the guard's verdict, side
//! effects, cancellation — so it must not report a verdict on those, and it
//! does not. Printing `answer_accuracy` and then listing what stayed
//! unmeasured is the honest output; filling the other fields with guesses would
//! manufacture a report nobody could trust.
//!
//! In particular the reported accuracy is **not** a B01 pass. §655 asks for
//! "100 case sets reaching the §6 thresholds", and the §6 thresholds include
//! the retrieval and behaviour rules that this runner leaves unmeasured.
//!
//! ## Credentials
//!
//! Read from the environment, never printed, never written to any record. A
//! missing variable stops the run with `CAPABILITY_NOT_READY`: a rehearsal stub
//! that would make the run green is exactly what §30.1 forbids.
//!
//!     NAI_LLM_BASE_URL=... NAI_LLM_API_KEY=... NAI_LLM_MODEL=... \
//!     cargo run -p nai-xtask --example golden_eval -- [--dir <目录>] [--evidence <文件>]
//!
//! Exit codes: 1 nothing to run or no credential (no call is made); 2 a call
//! ran and failed.
use nai_budget_core::{Ceiling, Limits};
use nai_contracts::v1::{
    BillingState, BudgetCharge, LlmChunk, LlmRequest, RequestContext, StreamEnd, UsageLedgerEntry,
};
use nai_kernel::golden::{AnswerVerdict, GoldenSet};
use nai_provider_core::{InMemoryGateway, Protocol, ProviderProfile};
use nai_security::{Operation, Secret};
use nai_xtask::golden::{PAYLOAD_DIR, case_set_files, read_case_set};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const REQUIRED: [&str; 3] = ["NAI_LLM_BASE_URL", "NAI_LLM_API_KEY", "NAI_LLM_MODEL"];

/// Enough room for a reasoning-capable model to finish. Such a model streams its
/// thinking first and only then the visible answer; a cap that fits only the
/// reasoning ends the stream with an empty completion, which would show up here
/// as a wrong answer rather than as a truncated run.
const MAX_OUTPUT_TOKENS: u32 = 256;

/// The criteria this runner cannot evaluate, each named with the reason. They
/// are printed with every run so a partial report can never be read as a
/// complete one.
const UNDRIVEN: [(&str, &str); 3] = [
    (
        "evidence_recall_at_5",
        "no served knowledge base is attached to this runner",
    ),
    (
        "retrieval_p95",
        "no retrieval samples exist without a served knowledge base",
    ),
    (
        "behaviour_expectations",
        "clarify/escalate, no-side-effect and cancel are decided by the running \
         pipeline (guard, side-effect and cancellation channels), which this \
         runner does not drive",
    ),
];

/// An unset and a blank variable are equally absent: a half-filled `.env` must
/// not produce a run that "half works".
fn read(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn credential(name: &str) -> Secret {
    Secret::new(read(name).expect("checked before any call").into_bytes())
        .expect("a non-empty credential")
}

fn arg_value(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

fn ceiling() -> Ceiling {
    Ceiling {
        cost_minor: 10_000,
        units: [
            ("requests".into(), 10_000u64),
            ("tokens".into(), 10_000_000u64),
            ("prompt_tokens".into(), 10_000_000u64),
            ("completion_tokens".into(), 10_000_000u64),
            ("total_tokens".into(), 10_000_000u64),
        ]
        .into(),
        concurrent: [("provider_slots".into(), 1u64)].into(),
    }
}

fn entry(request_id: &str) -> UsageLedgerEntry {
    UsageLedgerEntry {
        workspace_id: "golden-eval-workspace".into(),
        session_id: "golden-eval-session".into(),
        provider_ref: "golden-eval-text".into(),
        request_id: request_id.into(),
        generation_epoch: 1,
        estimate: Some(BudgetCharge {
            currency: "CNY".into(),
            cost_minor: 0,
            units: [("requests".into(), 1u64), ("tokens".into(), 4_000)].into(),
            price_revision: "golden-eval-price".into(),
        }),
        state: BillingState::Reserved as i32,
        ..Default::default()
    }
}

fn profile(base: &str, model: &str) -> ProviderProfile {
    ProviderProfile {
        provider_ref: "golden-eval-text".into(),
        endpoint: base.into(),
        model: model.into(),
        price_revision: "golden-eval-price".into(),
        currency: "CNY".into(),
        timeout: Operation::CEILING,
        max_retries: 1,
        protocol: Protocol::OpenAiCompatible,
    }
}

fn text_of(chunks: &[LlmChunk]) -> String {
    chunks
        .iter()
        .filter(|chunk| chunk.end == StreamEnd::StreamData as i32)
        .map(|chunk| chunk.text_delta.clone())
        .collect()
}

fn verdict_label(verdict: AnswerVerdict) -> &'static str {
    match verdict {
        AnswerVerdict::Correct => "correct",
        AnswerVerdict::Incorrect => "incorrect",
        AnswerVerdict::Unknown => "unknown",
    }
}

/// The payload directory: `--dir` overrides the §30.1 location so a fixture can
/// be exercised without pretending it is the real set.
fn payload_dir(root: &Path) -> PathBuf {
    match arg_value("--dir") {
        Some(dir) => root.join(dir),
        None => root.join(PAYLOAD_DIR),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let dir = payload_dir(&root);

    // ---- 1. the payload: absent is "not ready", malformed is a defect
    let files = case_set_files(&dir)?;
    if files.is_empty() {
        println!(
            "CAPABILITY_NOT_READY: no golden case set under {}",
            dir.display()
        );
        println!("the §6 questions are a scenario-package payload; nothing here fabricates one");
        std::process::exit(1);
    }

    // Read every set before making any call: a structural defect is a reason to
    // stop, not something to discover halfway through a paid run. The reader is
    // the payload gate's own, so a set that loads here is a set the gate
    // accepts.
    let mut sets: Vec<(String, GoldenSet)> = Vec::new();
    let mut defects = 0usize;
    for (name, text) in &files {
        match read_case_set(name, text) {
            Ok(set) => sets.push((name.clone(), set)),
            Err(found) => {
                for defect in &found {
                    println!("defect: {} {} {}", defect.file, defect.token, defect.detail);
                }
                defects += found.len();
            }
        }
    }
    if defects > 0 {
        println!("the case set is structurally broken; no call was made");
        std::process::exit(1);
    }

    // ---- 2. credentials
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|name| read(name).is_none())
        .collect();
    if !missing.is_empty() {
        // Variable names only; the values are never touched.
        println!("CAPABILITY_NOT_READY: missing {}", missing.join(", "));
        println!("no call was made; a rehearsal stub cannot stand in for a live endpoint");
        std::process::exit(1);
    }

    let profile = profile(
        &read("NAI_LLM_BASE_URL").expect("checked"),
        &read("NAI_LLM_MODEL").expect("checked"),
    );
    let mut gateway = InMemoryGateway::new(
        "golden-eval-workspace",
        Limits {
            currency: "CNY".into(),
            global: ceiling(),
            sessions: [("golden-eval-session".into(), ceiling())].into(),
            providers: [("golden-eval-text".into(), ceiling())].into(),
            max_entries: 10_000,
        },
    )
    .map_err(|error| format!("budget: {error:?}"))?;
    let client = profile.client(credential("NAI_LLM_API_KEY"))?;
    let llm = profile.llm(&client)?;
    let op = Operation::new(Operation::CEILING).expect("the runner stays inside the ceiling");

    // ---- 3. drive the real generation seam, question by question
    let mut runs: Vec<Value> = Vec::new();
    let mut failed = false;
    let mut total_judged = 0u64;
    let mut total_correct = 0u64;

    for (file, set) in &sets {
        let mut judged = 0u64;
        let mut correct = 0u64;
        let mut cases: Vec<Value> = Vec::new();

        for case in set.cases() {
            let request_id = format!("golden-eval-{}", case.id);
            let request = LlmRequest {
                // The question is sent as-is: the adapter adds no prompt text of
                // its own, and a system-only request is not a user query.
                permitted_context: case.question.clone(),
                max_output_tokens: MAX_OUTPUT_TOKENS,
                context: Some(RequestContext {
                    request_id: request_id.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let answer = match llm
                .generate(&op, &request, &mut gateway, entry(&request_id))
                .await
            {
                Ok(chunks) => text_of(&chunks),
                Err(error) => {
                    println!("call failed for case {}: {error}", case.id);
                    failed = true;
                    continue;
                }
            };
            let verdict = case.answer_verdict(&answer);
            match verdict {
                AnswerVerdict::Correct => {
                    judged += 1;
                    correct += 1;
                }
                AnswerVerdict::Incorrect => judged += 1,
                AnswerVerdict::Unknown => {}
            }
            cases.push(json!({
                "case_id": case.id,
                "category": case.category.label(),
                "verdict": verdict_label(verdict),
                "answer_chars": answer.chars().count(),
            }));
        }

        println!(
            "{file} [{}] {}/{} judged, {correct} correct",
            set.scenario_version,
            judged,
            set.len()
        );
        total_judged += judged;
        total_correct += correct;
        runs.push(json!({
            "file": file,
            "scenario_version": set.scenario_version,
            "cases_total": set.len(),
            "answer_accuracy": { "correct": correct, "judged": judged },
            "cases": cases,
        }));
    }

    // `checked_div` is the same rule as "no judged answer means no ratio": a run
    // that judged nothing reports nothing rather than a zero.
    let permille = (total_correct * 1000).checked_div(total_judged);
    match permille {
        Some(value) => println!("answer_accuracy: {value} permille over {total_judged} judged"),
        None => println!("answer_accuracy: unmeasured (no answer was judged)"),
    }
    // The unmeasured half is printed, never filled in. A runner that guessed
    // the behaviour side would produce a number someone might quote.
    for (field, reason) in UNDRIVEN {
        println!("unmeasured: {field} — {reason}");
    }
    println!(
        "not a B01 pass: §655 asks for the §6 thresholds, which include the unmeasured fields above"
    );

    let evidence = json!({
        "run": "golden-eval-generate-side",
        "generated_by": "xtask/examples/golden_eval.rs",
        "boundary": [
            "driven through ProviderProfile -> ProviderClient -> adapter -> budget gateway",
            "generation side only: answers judged against each case's expected answer",
            "credentials come from the environment and are absent from this record",
            "answer_accuracy here is not a B01 pass; retrieval and behaviour are unmeasured",
        ],
        "answer_accuracy": { "correct": total_correct, "judged": total_judged, "permille": permille },
        "unmeasured": UNDRIVEN.iter().map(|(field, reason)| json!({ "field": field, "reason": reason })).collect::<Vec<_>>(),
        "sets": runs,
    });
    if let Some(path) = arg_value("--evidence") {
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&evidence)?),
        )?;
        println!("evidence: {path}");
    }

    if failed {
        println!("golden evaluation: at least one question produced no answer");
        std::process::exit(2);
    }
    Ok(())
}

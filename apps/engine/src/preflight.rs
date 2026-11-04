//! One-key local preflight for the manual demo package.
//!
//! `nai-engine --preflight <scene.json>` answers "can this package run on this
//! machine without a running session?" by re-using the *same* loaders and
//! admission the runtime would perform — the operator scene schema, the
//! provider constructor, and the 2D closure admission — so a scene that passes
//! preflight is one the engine can actually construct. It never provisions,
//! never starts a session or server, and never makes a provider call unless the
//! caller explicitly requests a bounded connectivity probe with a credential
//! present. Each axis is `passed` / `failed` / `not_ready` with a reason; any
//! non-pass refuses the whole preflight (exit code non-zero), following the
//! "unknown and insufficient refuse alike" rule that `nai-upgrade-core::preflight`
//! fixes.
use crate::operator::{Loaded, build_provider};
use crate::pipeline::{RenderSetup, render_lane, resolve_render_setup};
use nai_budget_core::{Ceiling, Limits};
use nai_contracts::v1::{BillingState, BudgetCharge, LlmRequest, RequestContext, UsageLedgerEntry};
use nai_provider_core::{InMemoryGateway, Protocol};
use nai_security::Operation;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time;

pub const REPORT_NAME: &str = "preflight.json";

/// One axis of the preflight result. `status` is a closed set
/// (`passed`/`failed`/`not_ready`); `reason` names the text a human operator
/// needs to act on (missing environment variable name, asset id, exact check).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightAxis {
    pub name: String,
    pub status: String,
    pub reason: String,
}

impl PreflightAxis {
    fn passed(name: &str) -> Self {
        Self {
            name: name.into(),
            status: "passed".into(),
            reason: String::new(),
        }
    }
    fn failed(name: &str, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "failed".into(),
            reason: reason.into(),
        }
    }
    fn not_ready(name: &str, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "not_ready".into(),
            reason: reason.into(),
        }
    }
    pub fn is_pass(&self) -> bool {
        self.status == "passed"
    }
}

/// The full preflight report: the axes, whether it all passed, and the honest
/// fact that no device output and no playback acknowledgement happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    pub checks: Vec<PreflightAxis>,
    pub all_passed: bool,
    pub generated_at_utc_ms: u64,
    pub playback_ack: bool,
}

impl PreflightReport {
    fn to_value(&self) -> Value {
        json!({
            "generated_at_utc_ms": self.generated_at_utc_ms.to_string(),
            "all_passed": self.all_passed,
            "playback_ack": self.playback_ack,
            "note": "本机预检只验证场景与素材准入；未播放、未接设备、未产生 PlaybackAck。",
            "checks": self.checks.iter().map(|c| json!({
                "name": c.name, "status": c.status, "reason": c.reason
            })).collect::<Vec<_>>(),
        })
    }
}

/// Runs the preflight against a scene. Pure read: writes nothing except the
/// report the caller chooses to persist, and makes a provider call only when
/// `connectivity` is requested *and* a credential is present.
pub async fn run_scene(scene: &Path, connectivity: bool) -> PreflightReport {
    let mut checks = Vec::new();
    let loaded = match crate::operator::load(scene) {
        Ok(loaded) => loaded,
        Err(reason) => {
            return PreflightReport {
                checks: vec![PreflightAxis::failed("scene", reason)],
                all_passed: false,
                generated_at_utc_ms: now(),
                playback_ack: false,
            };
        }
    };
    checks.push(PreflightAxis::passed("scene"));

    // knowledge: the loader already rejects duplicate or empty document ids;
    // report the count so the operator can see what will be seeded.
    let doc_count = loaded.config["knowledge"]["documents"]
        .as_array()
        .map(|docs| docs.len())
        .unwrap_or(0);
    checks.push(if doc_count > 0 {
        PreflightAxis::passed("knowledge")
    } else {
        PreflightAxis::failed("knowledge", "场景没有可导入的知识文档")
    });

    checks.push(provider_axis(&loaded));
    checks.push(budget_axis(&loaded));
    checks.push(render_axis(&loaded));

    if connectivity {
        checks.push(connectivity_probe(&loaded).await);
    } else {
        checks.push(PreflightAxis::not_ready(
            "connectivity",
            "连通探针未发：需要显式 --preflight 并携带凭据时的受预算请求",
        ));
    }

    let all_passed = checks.iter().all(PreflightAxis::is_pass);
    PreflightReport {
        checks,
        all_passed,
        generated_at_utc_ms: now(),
        playback_ack: false,
    }
}

fn provider_axis(loaded: &Loaded) -> PreflightAxis {
    if loaded.config.get("provider_routes").is_some() {
        for cap in ["llm", "tts"] {
            match crate::provider_routes::Routes::build(&loaded.config, cap) {
                Ok(routes) if routes.endpoints.iter().all(|e| e.client.is_some()) => {}
                _ => {
                    return PreflightAxis::not_ready(
                        "provider",
                        format!("{cap}部分主备凭据缺失或配置不可用；未认证"),
                    );
                }
            }
        }
    }
    match build_provider(&loaded.config) {
        Ok(_) => PreflightAxis::passed("provider"),
        Err(reason) => {
            if reason.starts_with("CAPABILITY_NOT_READY:") {
                // Credential missing or blank is not a config error: the config
                // could be correct, the operator just has not put the key in.
                PreflightAxis::not_ready("provider", reason)
            } else {
                PreflightAxis::failed("provider", reason)
            }
        }
    }
}

fn budget_axis(loaded: &Loaded) -> PreflightAxis {
    let p = loaded
        .config
        .pointer("/provider_routes/llm/providers/0")
        .unwrap_or(&loaded.config["provider"]);
    let profile = match crate::operator::profile_from(p) {
        Ok(profile) => profile,
        Err(reason) => return PreflightAxis::failed("budget", reason),
    };
    let limits = budget_limits(&loaded.config, &profile);
    match nai_budget_core::Book::new(
        loaded.config["knowledge"]["scope"].as_str().unwrap_or("x"),
        limits,
    ) {
        Ok(_) => PreflightAxis::passed("budget"),
        Err(error) => PreflightAxis::failed("budget", format!("预算不合理：{error:?}")),
    }
}

/// The same budget shape the operator lane derives (`limits()` in operator.rs);
/// duplicated here only as a *call* to a shared constructor so a cost estimate
/// the preflight accepts is one the runtime could actually reserve. The ceiling
/// values come straight from the scene, matching operator.rs.
pub(crate) fn budget_limits(
    config: &Value,
    profile: &nai_provider_core::ProviderProfile,
) -> Limits {
    let b = &config["budget"];
    let provider_ref = profile.provider_ref.as_str();
    let tokens = number(b, "token_limit");
    let ceiling = Ceiling {
        cost_minor: number(b, "cost_limit_minor"),
        units: [
            ("requests".into(), number(b, "request_limit")),
            ("tokens".into(), tokens),
            ("prompt_tokens".into(), tokens),
            ("completion_tokens".into(), tokens),
            ("total_tokens".into(), tokens),
            ("characters".into(), number(b, "character_limit")),
        ]
        .into(),
        concurrent: [("provider_slots".into(), 1)].into(),
    };
    Limits {
        currency: profile.currency.clone(),
        global: ceiling.clone(),
        sessions: [("preflight".into(), ceiling.clone())].into(),
        providers: [(provider_ref.into(), ceiling)].into(),
        max_entries: 100,
    }
}

fn number(v: &Value, key: &str) -> u64 {
    v[key].as_u64().unwrap_or(0)
}

fn render_axis(loaded: &Loaded) -> PreflightAxis {
    let Some(render_value) = loaded.config.get("render").filter(|value| !value.is_null()) else {
        return PreflightAxis::not_ready(
            "render",
            "场景没有 render 闭包：演示包需要 2D 素材才可演练语音化输出",
        );
    };
    let mut setup: RenderSetup = match serde_json::from_value(render_value.clone()) {
        Ok(setup) => setup,
        Err(_) => {
            return PreflightAxis::failed("render", "render 段不是一个合法的 RenderSetup");
        }
    };
    if let Err(reason) = resolve_render_setup(&mut setup, &loaded.base_dir) {
        return PreflightAxis::failed("render", reason);
    }
    // The admission IS the runtime admission: sha256, licence grant, exact
    // three-asset closure, layer/size geometry, binding. A scene that passes
    // here is one the engine can build its rehearsal lane from.
    match render_lane(Some(&setup)) {
        Ok(Some(_)) => PreflightAxis::passed("render"),
        Ok(None) => PreflightAxis::not_ready("render", "render 闭包为空"),
        Err(reason) => PreflightAxis::failed("render", reason),
    }
}

/// A bounded, live, credentialed probe through the *same* provider stack the
/// operator lane would use. The result is a reachability fact only: it is
/// returned in the report and never recorded as a playback acknowledgement or
/// as source eligibility.
async fn connectivity_probe(loaded: &Loaded) -> PreflightAxis {
    let (profile, client) = match build_provider(&loaded.config) {
        Ok(pair) => pair,
        Err(reason) => return PreflightAxis::not_ready("connectivity", reason),
    };
    let ok = matches!(profile.protocol, Protocol::OpenAiCompatible);
    if !ok {
        return PreflightAxis::not_ready(
            "connectivity",
            "连通探针只支持 OpenAI-compatible 协议，当前协议不发起调用",
        );
    }
    let mut gateway = match InMemoryGateway::new(
        loaded.config["knowledge"]["scope"]
            .as_str()
            .unwrap_or("preflight"),
        budget_limits(&loaded.config, &profile),
    ) {
        Ok(gateway) => gateway,
        Err(error) => {
            return PreflightAxis::failed("connectivity", format!("探针预算不合理：{error:?}"));
        }
    };
    let request_id = format!("preflight-{}", now());
    let entry = UsageLedgerEntry {
        workspace_id: loaded.config["knowledge"]["scope"]
            .as_str()
            .unwrap_or("preflight")
            .into(),
        session_id: "preflight".into(),
        provider_ref: profile.provider_ref.clone(),
        request_id: request_id.clone(),
        generation_epoch: 1,
        state: BillingState::Reserved as i32,
        estimate: Some(BudgetCharge {
            currency: profile.currency.clone(),
            cost_minor: number(&loaded.config["budget"], "request_estimate_minor"),
            price_revision: profile.price_revision.clone(),
            units: [("requests".into(), 1u64), ("characters".into(), 1u64)].into(),
        }),
        ..Default::default()
    };
    let operation = match Operation::new(profile.timeout) {
        Ok(operation) => operation,
        Err(error) => {
            return PreflightAxis::failed("connectivity", format!("预算操作无效：{error}"));
        }
    };
    let llm = match profile.llm(&client) {
        Ok(llm) => llm,
        Err(error) => return PreflightAxis::failed("connectivity", error.message),
    };
    let request = LlmRequest {
        context: Some(RequestContext {
            session_id: "preflight".into(),
            workspace_id: loaded.config["knowledge"]["scope"]
                .as_str()
                .unwrap_or("preflight")
                .into(),
            generation_epoch: Some(1),
            ..Default::default()
        }),
        provider_profile: profile.provider_ref.clone(),
        prompt_version: "preflight-probe".into(),
        permitted_context: "连通性探针：请回复“探针已接收”。".into(),
        max_output_tokens: 16,
        ..Default::default()
    };
    let cancel = AtomicBool::new(false);
    let result = time::timeout(profile.timeout, async {
        let call = llm.generate(&operation, &request, &mut gateway, entry);
        tokio::pin!(call);
        loop {
            tokio::select! {
                result = &mut call => break result,
                _ = time::sleep(Duration::from_millis(20)) => {
                    if cancel.load(Ordering::Acquire) {
                        operation.cancel();
                        break Err(nai_provider_core::ProviderError::cancelled());
                    }
                }
            }
        }
    })
    .await;
    match result {
        Ok(Ok(chunks)) => {
            let text: String = chunks
                .iter()
                .filter(|c| c.end == nai_contracts::v1::StreamEnd::StreamData as i32)
                .map(|c| c.text_delta.as_str())
                .collect();
            if text.trim().is_empty() {
                PreflightAxis::failed("connectivity", "探针返回成功但无文本")
            } else {
                if loaded.config.get("provider_routes").is_some() {
                    PreflightAxis::not_ready(
                        "connectivity",
                        "当前LLM探针成功；TTS与备用连通尚未实测",
                    )
                } else {
                    PreflightAxis::passed("connectivity")
                }
            }
        }
        Ok(Err(error)) => {
            PreflightAxis::failed("connectivity", format!("探针失败：{}", error.message))
        }
        Err(_) => {
            operation.cancel();
            PreflightAxis::failed("connectivity", "连通探针超时")
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Persists a report atomically under `<data-root>/operator-artifacts/`.
pub fn write_report(data_root: &Path, report: &PreflightReport) -> Result<(), String> {
    let dir = data_root.join("operator-artifacts");
    fs::create_dir_all(&dir).map_err(|error| format!("preflight dir unavailable: {error}"))?;
    let bytes = serde_json::to_vec_pretty(&report.to_value())
        .map_err(|error| format!("preflight report serialization failed: {error}"))?;
    let tmp = dir.join(format!(".{REPORT_NAME}.tmp"));
    fs::write(&tmp, &bytes).map_err(|error| format!("preflight report write failed: {error}"))?;
    fs::rename(&tmp, dir.join(REPORT_NAME))
        .map_err(|error| format!("preflight report finalize failed: {error}"))?;
    Ok(())
}

/// Reads the persisted report with a strict bound. Missing or unparseable is
/// `Ok(None)`; the caller decides how to read it (e.g. control route reports
/// `CAPABILITY_NOT_READY`).
pub fn read_report(path: &Path) -> Result<Option<Value>, String> {
    let Ok(bytes) = fs::read(path) else {
        return Ok(None);
    };
    if bytes.len() > 256 * 1024 {
        return Err("preflight report exceeds bound".into());
    }
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Err("preflight report is not valid JSON".into()),
    }
}

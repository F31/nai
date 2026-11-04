//! Explicit two-call diagnostic authorization. No credential or audio is returned to this process.
#[cfg(debug_assertions)]
mod diagnostic {
    use nai_contracts::workbench::*;
    use nai_workbench_host::foundation::{Owner, random_id};
    use prost::Message;
    use serde_json::json;
    use std::{
        io::Write,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };
    type R<T> = Result<T, Box<dyn std::error::Error>>;
    fn call(owner: &mut Owner, operation: foundation_command::Operation) -> R<FoundationResult> {
        let command = FoundationCommand {
            idempotency_key: random_id()?,
            operation: Some(operation),
        }
        .encode_to_vec();
        let raw = owner.call(&command)?;
        match FoundationResponse::decode(raw.as_slice())?.outcome {
            Some(foundation_response::Outcome::Result(r)) => Ok(r),
            Some(foundation_response::Outcome::Problem(p)) => Err(format!(
                "root rejected {}",
                WorkbenchError::try_from(p.code)?.as_str_name()
            )
            .into()),
            _ => Err("missing root result".into()),
        }
    }
    fn write(path: &Path, value: serde_json::Value) -> R<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(serde_json::to_string_pretty(&value)?.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }
    pub fn run() -> R<()> {
        use foundation_command::Operation as C;
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() != 4 || args[0] != "--authorized-two-tts-calls-once" {
            return Err("explicit authorization, engine, app-root, evidence-root required".into());
        }
        let engine = PathBuf::from(&args[1]);
        let app = PathBuf::from(&args[2]);
        let out = PathBuf::from(&args[3]);
        if !engine.is_absolute() || !app.is_absolute() || !out.is_absolute() {
            return Err("absolute paths required".into());
        }
        let ledger = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.nai-diagnostics/tts-route-authorization-2026-10-25.json");
        if ledger.exists() {
            return Err("authorization already allocated; no automatic retry".into());
        }
        std::fs::create_dir_all(&out)?;
        let root = app.join("foundation-v2");
        let identity = app.join("operator-v43.json");
        let mut owner = Owner::open_at(&engine, &identity, &root)?;
        let ws = random_id()?;
        call(
            &mut owner,
            C::CreateWorkspaceSettings(WorkspaceSettingsCommand {
                workspace_id: ws.clone(),
                settings: Some(WorkspaceSettingsDraft {
                    name: "语音路径诊断（演练，不用于直播）".into(),
                    budget_mode: WorkspaceBudgetMode::Disabled as i32,
                    currency: "CNY".into(),
                    ..Default::default()
                }),
            }),
        )?;
        let settings = call(
            &mut owner,
            C::ReadLlmSettings(FoundationLlmRead {
                workspace_id: ws.clone(),
            }),
        )?;
        if !settings.shared_catalog_enabled {
            return Err("shared catalog not enabled; no credential migration or API calls".into());
        }
        let models: Vec<_> = settings
            .tts_models
            .iter()
            .filter(|m| {
                m.enabled
                    && m.credential_saved
                    && m.model == "FunAudioLLM/CosyVoice2-0.5B"
                    && m.voice == "FunAudioLLM/CosyVoice2-0.5B:alex"
                    && [
                        "https://api.siliconflow.cn",
                        "https://api.siliconflow.cn/v1",
                    ]
                    .contains(&m.base_url.trim_end_matches('/'))
            })
            .collect();
        if models.len() != 1 {
            return Err("authorized model is missing or ambiguous; no API calls".into());
        }
        let model = models[0].clone();
        write(
            &out.join("setup.json"),
            json!({"workspace_id":ws,"model_id":model.id,"model_revision":model.revision,"model":model.model,"voice":model.voice,"base_url":model.base_url,"text":"你好，这是语音合成测试。","max_calls":2,"automatic_retry":false,"key_copied":false}),
        )?;
        drop(owner);
        write(
            &ledger,
            json!({"authorization":"user approved at most two calls: direct and system proxy once each","evidence_directory":out,"workspace_id":ws,"status":"allocated_no_repeat","automatic_retry":false}),
        )?;
        for direct in [true, false] {
            let route = if direct { "direct" } else { "system_proxy" };
            let task_id = random_id()?;
            let mut owner = Owner::open_diagnostic_route_at(&engine, &identity, &root, direct)?;
            // Recheck the exact selected model revision before consuming this route's one dispatch.
            let current = call(
                &mut owner,
                C::ReadLlmSettings(FoundationLlmRead {
                    workspace_id: ws.clone(),
                }),
            )?;
            if !current.tts_models.iter().any(|m| {
                m.id == model.id
                    && m.revision == model.revision
                    && m.base_url == model.base_url
                    && m.model == model.model
                    && m.voice == model.voice
                    && m.enabled
                    && m.credential_saved
            }) {
                return Err("model changed; no dispatch on this route".into());
            }
            write(
                &out.join(format!("{route}-dispatch.json")),
                json!({"route":route,"task_id":task_id,"trace_id":task_id,"status":"dispatch_may_have_started_do_not_retry"}),
            )?;
            let started = Instant::now();
            println!("DISPATCH {route} task={task_id}");
            let operation = C::TestTts(FoundationTtsTest {
                workspace_id: ws.clone(),
                task_id: task_id.clone(),
                model_id: model.id.clone(),
                model_revision: model.revision,
            });
            let result = (|| -> R<FoundationResult> {
                let mut r = call(&mut owner, operation)?;
                let until = Instant::now() + Duration::from_secs(52);
                while matches!(r.task_state, 3 | 4) {
                    if Instant::now() >= until {
                        return Err("root task outcome unknown; no repeat".into());
                    }
                    std::thread::sleep(Duration::from_millis(250));
                    r = call(
                        &mut owner,
                        C::ReadTtsTask(FoundationLlmTaskRead {
                            workspace_id: ws.clone(),
                            task_id: task_id.clone(),
                        }),
                    )?;
                }
                Ok(r)
            })();
            match result {
                Ok(r) => {
                    let body: serde_json::Value = serde_json::from_str(&r.body).unwrap_or_default();
                    let summary = json!({"route":route,"task_id":task_id,"trace_id":task_id,"elapsed_ms":started.elapsed().as_millis(),"task_state":r.task_state,"reason":r.task_reason,"charge_status":r.charge_status,"sample_count":body["sample_count"],"audio_sha256":body["audio_sha256"],"played":false,"automatic_retry":false});
                    write(&out.join(format!("{route}-result.json")), summary.clone())?;
                    println!("{summary}");
                }
                Err(_) => {
                    write(
                        &out.join(format!("{route}-result.json")),
                        json!({"route":route,"task_id":task_id,"elapsed_ms":started.elapsed().as_millis(),"outcome":"unknown","automatic_retry":false}),
                    )?;
                    println!("OUTCOME_UNKNOWN {route}; no retry");
                }
            }
            drop(owner);
        }
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    {
        diagnostic::run()
    }
    #[cfg(not(debug_assertions))]
    {
        Err("diagnostic unavailable in release".into())
    }
}

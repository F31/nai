//! Register the user-requested local API through the authenticated owner, then test once.
use nai_contracts::workbench::*;
use nai_workbench_host::foundation::{Owner, random_id};
use prost::Message;
use serde_json::json;
use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};
type R<T> = Result<T, Box<dyn std::error::Error>>;
fn call(owner: &mut Owner, operation: foundation_command::Operation) -> R<FoundationResult> {
    let raw = owner.call(
        &FoundationCommand {
            idempotency_key: random_id()?,
            operation: Some(operation),
        }
        .encode_to_vec(),
    )?;
    match FoundationResponse::decode(raw.as_slice())?.outcome {
        Some(foundation_response::Outcome::Result(r)) => Ok(r),
        Some(foundation_response::Outcome::Problem(p)) => {
            Err(format!("root rejected {}", p.code).into())
        }
        _ => Err("missing result".into()),
    }
}
fn main() -> R<()> {
    use foundation_command::Operation as C;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 || args[0] != "--register-user-local-qwen" {
        return Err("explicit registration, engine, app-root, evidence-root required".into());
    }
    let engine = PathBuf::from(&args[1]);
    let app = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    if !engine.is_absolute() || !app.is_absolute() || !out.is_absolute() {
        return Err("absolute paths required".into());
    }
    std::fs::create_dir_all(&out)?;
    let mut owner = Owner::open_at(
        &engine,
        &app.join("operator-v43.json"),
        &app.join("foundation-v2"),
    )?;
    let mut ws = call(
        &mut owner,
        C::WorkspaceContext(FoundationLlmRead::default()),
    )?
    .resource_id;
    if ws.is_empty() {
        ws = random_id()?;
        call(
            &mut owner,
            C::CreateWorkspaceSettings(WorkspaceSettingsCommand {
                workspace_id: ws.clone(),
                settings: Some(WorkspaceSettingsDraft {
                    name: "本地 Qwen 注册".into(),
                    budget_mode: WorkspaceBudgetMode::Limited as i32,
                    budget_limit_minor: 10,
                    currency: "CNY".into(),
                    expected_revision: 0,
                }),
            }),
        )?;
    }
    let mut settings = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: ws.clone(),
        }),
    )?;
    if !settings.shared_catalog_enabled {
        let preview = call(
            &mut owner,
            C::PreviewSharedModels(FoundationLlmRead::default()),
        )?
        .shared_model_preview
        .ok_or("shared preview missing")?;
        if !preview.rows.is_empty() {
            return Err(
                "shared catalog disabled with existing entries; driver will not migrate credentials"
                    .into(),
            );
        }
        let migration = SharedModelMigration {
            migration_id: random_id()?,
            plan_sha256: preview.plan_sha256,
            entries: vec![],
        };
        let mut committed = false;
        for _ in 0..8 {
            if call(&mut owner, C::MigrateSharedModels(migration.clone()))?.shared_catalog_enabled {
                committed = true;
                break;
            }
        }
        if !committed {
            return Err("empty shared catalog did not commit".into());
        }
        settings = call(
            &mut owner,
            C::ReadLlmSettings(FoundationLlmRead {
                workspace_id: ws.clone(),
            }),
        )?;
    }
    let found: Vec<_> = settings
        .tts_models
        .iter()
        .filter(|m| {
            m.protocol == "qwen_local"
                && m.base_url.trim_end_matches('/') == "http://127.0.0.1:8000/v1"
                && m.model == "qwen3-tts-1.7b-customvoice"
        })
        .collect();
    let id = match found.as_slice() {
        [] => {
            let id = random_id()?;
            call(
                &mut owner,
                C::SaveTtsModel(FoundationTtsModel {
                    workspace_id: ws.clone(),
                    model_id: id.clone(),
                    name: "本地 Qwen3-TTS".into(),
                    base_url: "http://127.0.0.1:8000/v1".into(),
                    model: "qwen3-tts-1.7b-customvoice".into(),
                    voice: "serena".into(),
                    protocol: "qwen_local".into(),
                    language: "chinese".into(),
                    enabled: true,
                    ..Default::default()
                }),
            )?;
            id
        }
        [m] => m.id.clone(),
        _ => return Err("ambiguous existing local models; no overwrite".into()),
    };
    let settings = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: ws.clone(),
        }),
    )?;
    let model = settings
        .tts_models
        .iter()
        .find(|m| m.id == id)
        .ok_or("saved model absent")?;
    if !model.enabled || model.credential_saved || model.protocol != "qwen_local" {
        return Err("invalid local configuration".into());
    }
    let task = random_id()?;
    let mut evidence = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join("local-test.json"))?;
    let started = Instant::now();
    let mut r = call(
        &mut owner,
        C::TestTts(FoundationTtsTest {
            workspace_id: ws.clone(),
            task_id: task.clone(),
            model_id: id.clone(),
            model_revision: model.revision,
        }),
    )?;
    while matches!(r.task_state, 3 | 4) {
        if started.elapsed() > Duration::from_secs(52) {
            return Err("task outcome unknown; no retry".into());
        }
        std::thread::sleep(Duration::from_millis(150));
        r = call(
            &mut owner,
            C::ReadTtsTask(FoundationLlmTaskRead {
                workspace_id: ws.clone(),
                task_id: task.clone(),
            }),
        )?;
    }
    let audio = if r.task_state == AgentTaskState::Completed as i32 {
        call(
            &mut owner,
            C::AuthorizeTtsAudio(FoundationTtsAudio {
                workspace_id: ws,
                task_id: task,
                task_sha256: r.sha256.clone(),
            }),
        )?
        .tts_audio
    } else {
        None
    };
    let ok = audio
        .as_ref()
        .is_some_and(|a| a.sample_count > 0 && a.sample_rate == 24000);
    let result = json!({"model_id":id,"model":model.model,"voice":model.voice,"language":model.language,"base_url":model.base_url,"shared_catalog":true,"tts_model_count":settings.tts_models.len(),"state":r.task_state,"reason":r.task_reason,"elapsed_ms":started.elapsed().as_millis(),"audio":audio.map(|a|json!({"sample_count":a.sample_count,"sample_rate":a.sample_rate,"sha256":a.pcm_sha256})),"cache_authorized":ok,"played":false,"paid_api":false,"automatic_retry":false});
    evidence.write_all(serde_json::to_string_pretty(&result)?.as_bytes())?;
    evidence.sync_all()?;
    println!("{result}");
    if !ok {
        return Err("local test failed; model retained; no retry".into());
    }
    Ok(())
}

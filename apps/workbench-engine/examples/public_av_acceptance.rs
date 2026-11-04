//! Explicit, operator-authorized local AV acceptance. No secrets are read or copied by this driver.
use nai_contracts::workbench::*;
use nai_workbench_host::{
    foundation::{Owner, random_id},
    public_output::OutputOwner,
};
use prost::Message;
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
type R<T> = Result<T, Box<dyn std::error::Error>>;
fn call(
    owner: &mut Owner,
    op: foundation_command::Operation,
    window: Option<&mut OutputOwner>,
    root: &Path,
) -> R<FoundationResult> {
    let cmd = FoundationCommand {
        idempotency_key: random_id()?,
        operation: Some(op),
    }
    .encode_to_vec();
    let raw = if let Some(window) = window {
        owner.call_with_public_output(&cmd, Some(window), root)?
    } else {
        owner.call(&cmd)?
    };
    match FoundationResponse::decode(raw.as_slice())?.outcome {
        Some(foundation_response::Outcome::Result(r)) => Ok(r),
        Some(foundation_response::Outcome::Problem(p)) => Err(format!(
            "root rejected: {}",
            WorkbenchError::try_from(p.code)?.as_str_name()
        )
        .into()),
        _ => Err("missing result".into()),
    }
}
fn save(owner: &mut Owner, root: &Path, ws: &str, id: &str, doc: Value) -> R<FoundationResult> {
    call(
        owner,
        foundation_command::Operation::PutPreparation(FoundationObject {
            workspace_id: ws.into(),
            object_id: id.into(),
            expected_revision: 0,
            body: doc.to_string(),
        }),
        None,
        root,
    )
}
fn evidence(out: &Path, name: &str, v: Value) -> R<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join(name))?;
    f.write_all(serde_json::to_string_pretty(&v)?.as_bytes())?;
    f.sync_all()?;
    Ok(())
}
fn main() -> R<()> {
    use foundation_command::Operation as C;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 5
        || (args[0] != "--authorized-real-tts-once" && args[0] != "--local-qwen-av-once")
    {
        return Err(
            "explicit authorization + engine, keeper, app-root, evidence-root required".into(),
        );
    }
    let local = args[0] == "--local-qwen-av-once";
    let engine = PathBuf::from(&args[1]);
    let keeper = PathBuf::from(&args[2]);
    let app = PathBuf::from(&args[3]);
    let out = PathBuf::from(&args[4]);
    let root = app.join("foundation-v2");
    if !out.is_absolute() || !engine.is_absolute() || !keeper.is_absolute() || !app.is_absolute() {
        return Err("absolute paths required".into());
    }
    std::fs::create_dir_all(&out)?;
    let authorization_root = if local {
        out.as_path()
    } else {
        out.parent().ok_or("evidence parent required")?
    };
    let authorization_name = if local {
        "local-qwen-av-once.json"
    } else {
        "caption-authorization-once.json"
    };
    if authorization_root.join(authorization_name).exists()
        || out.join("dispatch-once.json").exists()
    {
        return Err(
            "the single authorized attempt is already recorded; refusing another dispatch".into(),
        );
    }
    let mut owner = Owner::open_at(&engine, &app.join("operator-v43.json"), &root)?;
    let ws = random_id()?;
    call(
        &mut owner,
        C::CreateWorkspaceSettings(WorkspaceSettingsCommand {
            workspace_id: ws.clone(),
            settings: Some(WorkspaceSettingsDraft {
                name: "字幕音画验收（演练，不用于实际直播）".into(),
                budget_mode: WorkspaceBudgetMode::Limited as i32,
                budget_limit_minor: 10,
                currency: "CNY".into(),
                expected_revision: 0,
            }),
        }),
        None,
        &root,
    )?;
    let mut settings = call(
        &mut owner,
        C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: ws.clone(),
        }),
        None,
        &root,
    )?;
    if !settings.shared_catalog_enabled {
        enable_empty_shared_catalog(&mut owner, &root)?;
        settings = call(
            &mut owner,
            C::ReadLlmSettings(FoundationLlmRead {
                workspace_id: ws.clone(),
            }),
            None,
            &root,
        )?;
        if !settings.shared_catalog_enabled {
            return Err("empty shared catalog did not commit".into());
        }
    }
    let matches: Vec<_> = settings
        .tts_models
        .iter()
        .filter(|m| model_matches(m, local))
        .collect();
    let model = match matches.as_slice() {
        [m] => (*m).clone(),
        [] if local => {
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
                None,
                &root,
            )?;
            let saved = call(
                &mut owner,
                C::ReadLlmSettings(FoundationLlmRead {
                    workspace_id: ws.clone(),
                }),
                None,
                &root,
            )?;
            let found: Vec<_> = saved
                .tts_models
                .iter()
                .filter(|m| model_matches(m, true))
                .collect();
            match found.as_slice() {
                [m] => (*m).clone(),
                _ => return Err("authorized model absent or ambiguous; no dispatch".into()),
            }
        }
        _ => return Err("authorized model absent or ambiguous; no dispatch".into()),
    };
    let mut product: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/contracts/preparation.json"
    ))?;
    product["name"] = "音画验收样例（非直播商品）".into();
    product["facts"][0]["conditions"] = "".into();
    product["facts"][0]["source"] = "仅本机验收的固定演练数据，不作为实际商品事实".into();
    product["faq"][0]["text"] = "容量为500毫升。".into();
    product["talk_tracks"][0]["text"] = "容量为500毫升。".into();
    let product_id = random_id()?;
    let host_id = random_id()?;
    let series_id = random_id()?;
    let show_id = random_id()?;
    let report_id = random_id()?;
    let snapshot_id = random_id()?;
    let p = save(&mut owner, &root, &ws, &product_id, product)?;
    let h = save(
        &mut owner,
        &root,
        &ws,
        &host_id,
        json!({"schema_version":1,"kind":"host_draft","name":"音画验收","style":"professional","preferences":""}),
    )?;
    let series = save(
        &mut owner,
        &root,
        &ws,
        &series_id,
        json!({"schema_version":1,"kind":"series_draft","name":"本地音画验收","goal":"隔离公共窗口录制，不推流"}),
    )?;
    let ptr = |id: &str, r: &FoundationResult| json!({"object_id":id,"revision":r.revision,"sha256":r.sha256});
    save(
        &mut owner,
        &root,
        &ws,
        &show_id,
        json!({"schema_version":1,"kind":"show_draft","name":"本地公共字幕验收","product":ptr(&product_id,&p),"host":ptr(&host_id,&h),"series":ptr(&series_id,&series),"cues":[{"id":"capacity","title":"容量原文","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":false}]}),
    )?;
    let report = call(
        &mut owner,
        C::PublicationImpact(FoundationImpact {
            workspace_id: ws.clone(),
            report_id: report_id.clone(),
            show_id: show_id.clone(),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }),
        None,
        &root,
    )?;
    let snapshot = call(
        &mut owner,
        C::Publish(FoundationPublish {
            workspace_id: ws.clone(),
            snapshot_id: snapshot_id.clone(),
            report_id,
            report_sha256: report.sha256,
            acknowledge_unknown: true,
        }),
        None,
        &root,
    )?;
    let mut window = OutputOwner::open_at(&keeper, &root, false)?;
    evidence(
        &out,
        "setup.json",
        json!({"workspace_id":ws,"output_id":window.current().output_id,"output_pid":window.process_id(),"model_id":model.id,"model":model.model,"voice":model.voice,"text":"容量为500毫升。","budget_minor":10,"currency":"CNY","delivery":"local_public_window_only","provider_dispatched":false}),
    )?;
    println!(
        "ACCEPTANCE_WINDOW_READY pid={} workspace={ws}; no API sent. Send RUN only after isolated OBS recording is ready.",
        window.process_id()
    );
    std::io::stdout().flush()?;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut input = std::io::BufReader::new(std::io::stdin());
        loop {
            let mut line = String::new();
            let Ok(n) = input.by_ref().take(65).read_line(&mut line) else {
                break;
            };
            if n == 0 {
                break;
            }
            if line.len() > 64 {
                break;
            }
            if tx.send(line.trim().to_owned()).is_err() {
                break;
            }
        }
    });
    let idle = Instant::now() + Duration::from_secs(600);
    loop {
        window.poll()?;
        if out.join("stop.request").exists() {
            return Ok(());
        }
        if out.join("run.request").exists() {
            break;
        }
        match rx.try_recv() {
            Ok(s) if s == "RUN" => break,
            Ok(s) if s == "STOP" => return Ok(()),
            Ok(_) => return Err("unsupported command; no dispatch".into()),
            Err(mpsc::TryRecvError::Disconnected) => (),
            _ => (),
        }
        if Instant::now() >= idle {
            return Err("idle deadline; no dispatch".into());
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    let preview_id = random_id()?;
    let session_id = random_id()?;
    let task_id = random_id()?;
    let (card, done) = standby_during(&mut window, || -> R<_> {
        let card = call(
            &mut owner,
            C::Preview(FoundationPreview {
                workspace_id: ws.clone(),
                preview_id: preview_id.clone(),
                snapshot_id,
                snapshot_sha256: snapshot.sha256,
                cue_id: "capacity".into(),
                session_id: session_id.clone(),
                expected_epoch: 0,
            }),
            None,
            &root,
        )?;
        let plan = FoundationSpeechPlan {
            workspace_id: ws.clone(),
            preview_id: preview_id.clone(),
            preview_sha256: card.sha256.clone(),
            model_id: model.id.clone(),
            model_revision: model.revision,
            max_cost_minor: 10,
            task_id: task_id.clone(),
        };
        let checked = call(&mut owner, C::CheckPreviewSpeech(plan.clone()), None, &root)?
            .speech_plan
            .ok_or("missing speech plan")?;
        if checked.text != "容量为500毫升。"
            || checked.voice != model.voice
            || checked.reserve_minor > 10
        {
            return Err("authorized phrase/voice/budget mismatch; no dispatch".into());
        }
        evidence(
            &out,
            "dispatch-once.json",
            json!({"authorization":if local { "user requested local public AV acceptance" } else { "user approved one request, CNY 0.10" },"paid_api":!local,"workspace_id":ws,"task_id":task_id,"model":model.model,"voice":model.voice,"text":checked.text,"max_cost_minor":10,"automatic_retry":false,"status":"dispatch_may_have_started_do_not_repeat"}),
        )?;
        evidence(
            authorization_root,
            authorization_name,
            json!({"authorization":if local { "local Qwen acceptance, one dispatch, no paid API" } else { "single user-approved TTS attempt, CNY 0.10" },"task_id":task_id,"workspace_id":ws,"evidence_directory":out,"status":"consumed_before_dispatch_no_automatic_retry"}),
        )?;
        call(
            &mut owner,
            C::SynthesizePreviewSpeech(FoundationSpeechStart {
                plan: Some(plan),
                task_id: task_id.clone(),
                approval_sha256: checked.approval_sha256,
            }),
            None,
            &root,
        )?;
        let until = Instant::now() + Duration::from_secs(40);
        let done = loop {
            let d = call(
                &mut owner,
                C::ReadTtsTask(FoundationLlmTaskRead {
                    workspace_id: ws.clone(),
                    task_id: task_id.clone(),
                }),
                None,
                &root,
            )?;
            if !matches!(d.task_state,x if x==AgentTaskState::Queued as i32||x==AgentTaskState::Running as i32)
            {
                break d;
            }
            if Instant::now() >= until {
                return Err("provider outcome unknown; no retry".into());
            }
            std::thread::sleep(Duration::from_millis(80));
        };
        evidence(
            &out,
            "tts-result.json",
            json!({"task_id":task_id,"state":done.task_state,"reason":done.task_reason,"charge_status":done.charge_status,"task_sha256":done.sha256}),
        )?;
        if done.task_state != AgentTaskState::Completed as i32 || done.body.is_empty() {
            return Err(
                "real TTS did not complete; single authorization consumed, no retry".into(),
            );
        }
        Ok((card, done))
    })?;
    let candidate = FoundationPublicCandidate {
        workspace_id: ws.clone(),
        preview_id,
        preview_sha256: card.sha256,
        task_id: task_id.clone(),
        task_sha256: done.sha256.clone(),
        output_observation: None,
    };
    let until = Instant::now() + Duration::from_secs(4);
    let mut checks = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out.join("candidate-checks.jsonl"))?;
    let draft = loop {
        let c = call(
            &mut owner,
            C::CheckPublicCandidate(candidate.clone()),
            Some(&mut window),
            &root,
        )?
        .public_candidate_check
        .ok_or("missing candidate check")?;
        writeln!(
            checks,
            "{}",
            json!({"axes":c.axes.iter().map(|a|json!({"axis":a.axis,"state":a.state,"reason":a.reason})).collect::<Vec<_>>(),"guard":c.guard_review.as_ref().map(|g|json!({"reason":g.reason,"policy_passed":g.policy_passed})),"output":c.output_instance.as_ref().map(|o|json!({"open":o.window_open,"terminal":o.terminal,"reason":o.reason})),"approval_available":c.approval_available})
        )?;
        checks.flush()?;
        if let Some(d) = c.approval_draft {
            break d;
        }
        if Instant::now() >= until {
            return Err("media/guard not ready; no new TTS will be sent".into());
        }
        std::thread::sleep(Duration::from_millis(80));
    };
    let bundle = draft.bundle.ok_or("missing bundle")?;
    call(
        &mut owner,
        C::ConfirmPublicApproval(FoundationPublicApproval {
            candidate: Some(candidate.clone()),
            bundle_sha256: bundle.approved_sha256.clone(),
            acknowledge_public_rights: true,
        }),
        Some(&mut window),
        &root,
    )?;
    let execution = call(
        &mut owner,
        C::StartPublicExecution(FoundationPublicWindowPrepare {
            candidate: Some(candidate),
            bundle_sha256: bundle.approved_sha256.clone(),
        }),
        Some(&mut window),
        &root,
    )?;
    evidence(
        &out,
        "execution-start.json",
        json!({"prepared":execution.public_window_prepared.as_ref().map(|p|json!({"device_ready":p.device_ready,"reason":p.reason,"device_name":p.device_name,"playing":p.playing,"stopped":p.execution_stopped,"caption_state":p.caption_state,"caption_font":p.caption_font,"device_clock":clock_json(p.device_clock.as_ref())})),"task_reason":execution.task_reason}),
    )?;
    if let Some(p) = &execution.public_window_prepared
        && !p.device_ready
    {
        return Err(format!("public device not ready: {}", p.reason).into());
    }
    let start = Instant::now();
    let mut timeline = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out.join("timeline.jsonl"))?;
    let mut caption_seen = false;
    let mut clock_seen = false;
    let mut max_played_samples = 0u64;
    let mut max_frame_sample_start = 0u64;
    let mut source_sample_within_played = true;
    let count = bundle.audio.as_ref().ok_or("missing audio")?.sample_count;
    loop {
        let next = Instant::now() + Duration::from_millis(100);
        let s = owner.renew_public_output(&mut window, &root)?;
        let p = s
            .window_prepared
            .as_ref()
            .ok_or("prepared media lost during playback")?;
        let v = s.presentation.as_ref();
        caption_seen |= v.is_some_and(|v| v.caption_visible);
        let frame_start = p.frame.as_ref().map(|f| f.sample_start);
        if let Some(c) = p.device_clock.as_ref() {
            if c.stream_generation >= 1 && c.device_frequency > 0 {
                clock_seen = true;
                max_played_samples = max_played_samples.max(c.played_samples);
                if let Some(sample) = frame_start {
                    max_frame_sample_start = max_frame_sample_start.max(sample);
                    if sample > c.played_samples {
                        source_sample_within_played = false;
                    }
                }
            }
        }
        writeln!(
            timeline,
            "{}",
            json!({"elapsed_ms":start.elapsed().as_millis(),"submitted_samples":p.submitted_samples,"playing":p.playing,"submission_complete":p.submission_complete,"execution_stopped":p.execution_stopped,"caption_visible":v.is_some_and(|v|v.caption_visible),"window_frame":v.map(|v|v.frame_index),"composited_sha256":v.map(|v|&v.composited_sha256),"openness_q16":p.frame.as_ref().map(|f|f.openness_q16),"device_clock":clock_json(p.device_clock.as_ref()),"frame_sample_start":frame_start,"presentation_sample_start":v.map(|v|v.sample_start)})
        )?;
        if p.submission_complete {
            if p.submitted_samples != count || !caption_seen || s.presentation.is_some() {
                return Err("submission/caption/clear evidence mismatch".into());
            }
            if !clock_seen {
                return Err("device clock never observed on the public WASAPI path".into());
            }
            if !source_sample_within_played {
                return Err("frame source sample exceeded played samples".into());
            }
            break;
        }
        if p.execution_stopped || s.terminal {
            return Err(
                "execution stopped before completion; no automatic re-synthesis or replay".into(),
            );
        }
        if start.elapsed() > Duration::from_secs(25) {
            return Err("execution deadline; no retry".into());
        }
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    timeline.sync_all()?;
    evidence(
        &out,
        "local-result.json",
        json!({"source":if local { "user_local_qwen_current_run" } else { "real_provider_current_run" },"paid_api":!local,"submitted_samples":count,"caption_observed":caption_seen,"hardware_playback_completed":"unknown","obs_recording":"external_evidence_required","device_clock_observed":clock_seen,"max_played_samples":max_played_samples,"max_frame_sample_start":max_frame_sample_start,"source_sample_within_played":source_sample_within_played,"workspace_id":ws,"task_id":task_id,"bundle_sha256":bundle.approved_sha256,"caption_sha256":bundle.caption_sha256}),
    )?;
    println!(
        "LOCAL_SUBMISSION_COMPLETE samples={count}; stop OBS recording, then send STOP. Hardware completion/OBS not inferred."
    );
    std::io::stdout().flush()?;
    let until = Instant::now() + Duration::from_secs(180);
    while Instant::now() < until {
        window.poll()?;
        if out.join("stop.request").exists() {
            break;
        }
        if let Ok(s) = rx.try_recv() {
            if s == "STOP" {
                break;
            }
            return Err("no replay accepted".into());
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    let _ = call(
        &mut owner,
        C::StopPreview(FoundationPreviewStop {
            workspace_id: ws,
            session_id,
            expected_epoch: 1,
        }),
        None,
        &root,
    );
    Ok(())
}

fn clock_json(clock: Option<&PublicDeviceClockObservation>) -> Value {
    clock.map(|c| json!({"stream_generation":c.stream_generation,"device_frequency":c.device_frequency,"device_position":c.device_position,"sample_rate":c.sample_rate,"played_samples":c.played_samples})).unwrap_or(Value::Null)
}
fn enable_empty_shared_catalog(owner: &mut Owner, root: &Path) -> R<()> {
    let preview = call(
        owner,
        foundation_command::Operation::PreviewSharedModels(FoundationLlmRead::default()),
        None,
        root,
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
    for _ in 0..8 {
        if call(
            owner,
            foundation_command::Operation::MigrateSharedModels(migration.clone()),
            None,
            root,
        )?
        .shared_catalog_enabled
        {
            return Ok(());
        }
    }
    Err("empty shared catalog did not commit".into())
}
fn model_matches(m: &TtsModelView, local: bool) -> bool {
    if local {
        m.enabled
            && !m.credential_saved
            && m.protocol == "qwen_local"
            && m.base_url.trim_end_matches('/') == "http://127.0.0.1:8000/v1"
            && m.model == "qwen3-tts-1.7b-customvoice"
            && m.voice == "serena"
            && m.language == "chinese"
    } else {
        m.enabled
            && m.credential_saved
            && (m.protocol.is_empty() || m.protocol == "openai_speech")
            && m.model == "FunAudioLLM/CosyVoice2-0.5B"
            && m.voice == "FunAudioLLM/CosyVoice2-0.5B:alex"
            && [
                "https://api.siliconflow.cn",
                "https://api.siliconflow.cn/v1",
            ]
            .contains(&m.base_url.trim_end_matches('/'))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    #[ignore = "requires actual Windows public standby window; no voice or API"]
    fn slow_source_work_does_not_starve_standby_health() {
        let root = tempfile::tempdir().unwrap();
        let keeper = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("nai-public-output-keeper.exe");
        let mut window = OutputOwner::open_at(&keeper, root.path(), true).unwrap();
        standby_during(&mut window, || {
            std::thread::sleep(Duration::from_millis(1100));
            Ok(())
        })
        .unwrap();
        assert!(!window.poll().unwrap().terminal);
    }
    #[test]
    fn local_acceptance_never_selects_paid_or_credentialed_models() {
        let m = TtsModelView {
            enabled: true,
            protocol: "qwen_local".into(),
            base_url: "http://127.0.0.1:8000/v1".into(),
            model: "qwen3-tts-1.7b-customvoice".into(),
            voice: "serena".into(),
            language: "chinese".into(),
            ..Default::default()
        };
        assert!(model_matches(&m, true));
        assert!(!model_matches(&m, false));
        let mut bad = m.clone();
        bad.credential_saved = true;
        assert!(!model_matches(&bad, true));
        bad = m.clone();
        bad.base_url = "https://api.siliconflow.cn/v1".into();
        assert!(!model_matches(&bad, true));
        bad = m.clone();
        bad.protocol = "openai_speech".into();
        assert!(!model_matches(&bad, true));
        bad = m.clone();
        bad.language = "auto".into();
        assert!(!model_matches(&bad, true));
        bad = m.clone();
        bad.enabled = false;
        assert!(!model_matches(&bad, true));
    }
}

fn standby_during<T>(window: &mut OutputOwner, action: impl FnOnce() -> R<T>) -> R<T> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let flag = &stop;
        let heartbeat = scope.spawn(move || -> Result<(), String> {
            while !flag.load(Ordering::Acquire) {
                if window.poll()?.terminal {
                    return Err("standby lost during source preparation".into());
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Ok(())
        });
        let result = action();
        stop.store(true, Ordering::Release);
        heartbeat
            .join()
            .map_err(|_| "standby heartbeat panicked")??;
        result
    })
}

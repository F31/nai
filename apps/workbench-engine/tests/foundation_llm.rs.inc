//! Windows-only real credential vault, real child engine, synthetic local HTTP provider.
use super::*;
use nai_foundation_sqlite::Error;
use serde_json::{Value, json};
use std::time::Instant;
#[test]
fn real_native_settings_tests_no_budget_and_actionable_provider_errors() {
    for speech in [false, true] {
        for (status, body, want) in [
            (402, "synthetic-secret", "BALANCE_INSUFFICIENT"),
            (401, "synthetic-secret", "HTTP_401"),
            (403, "synthetic-secret", "HTTP_403"),
            (
                403,
                r#"{"message":"余额不足 synthetic-secret"}"#,
                "BALANCE_INSUFFICIENT",
            ),
            (429, "synthetic-secret", "HTTP_429"),
            (503, "synthetic-secret", "HTTP_503"),
        ] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let url = format!("http://{}/v1", server.server_addr());
            let root = tempdir().unwrap();
            let _keys = Keys {
                root: root.path().into(),
            };
            let mut c = Client::open(root.path(), true, 1);
            c.send(C::CreateWorkspace(FoundationWorkspace {
                workspace_id: id(10),
                name: "synthetic no budget".into(),
                budget_limit_minor: 0,
                currency: "CNY".into(),
            }))
            .unwrap();
            if speech {
                c.send(C::SaveTtsModel(FoundationTtsModel {
                    protocol: "openai_speech".into(),
                    language: String::new(),
                    workspace_id: id(10),
                    model_id: id(100),
                    name: "synthetic".into(),
                    base_url: url,
                    model: "test".into(),
                    voice: "test:voice".into(),
                    enabled: true,
                    api_key: b"synthetic-secret".to_vec(),
                    ..Default::default()
                }))
                .unwrap();
            } else {
                c.send(C::SaveLlmModel(FoundationLlmModel {
                    workspace_id: id(10),
                    model_id: id(100),
                    name: "synthetic".into(),
                    base_url: url,
                    model: "test".into(),
                    enabled: true,
                    api_key: b"synthetic-secret".to_vec(),
                    ..Default::default()
                }))
                .unwrap();
            }
            let worker = std::thread::spawn(move || {
                let r = server
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap()
                    .unwrap();
                r.respond(tiny_http::Response::from_string(body).with_status_code(status))
                    .unwrap();
                assert!(
                    server
                        .recv_timeout(Duration::from_millis(100))
                        .unwrap()
                        .is_none()
                );
            });
            c.send(if speech {
                C::TestTts(FoundationTtsTest {
                    workspace_id: id(10),
                    task_id: id(121),
                    model_id: id(100),
                    model_revision: 1,
                })
            } else {
                C::TestLlm(FoundationModelTest {
                    workspace_id: id(10),
                    task_id: id(121),
                    model_id: id(100),
                    model_revision: 1,
                })
            })
            .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let done = loop {
                let q = FoundationLlmTaskRead {
                    workspace_id: id(10),
                    task_id: id(121),
                };
                let r = c
                    .send(if speech {
                        C::ReadTtsTask(q)
                    } else {
                        C::ReadLlmTask(q)
                    })
                    .unwrap();
                if !matches!(r.task_state, 3 | 4) {
                    break r;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(20));
            };
            worker.join().unwrap();
            assert_eq!(done.task_state, AgentTaskState::Failed as i32);
            assert_eq!(done.task_reason, want);
            assert!(done.body.is_empty());
            assert_eq!(done.reserved_minor, 0);
            assert_eq!(done.charge_status, "unknown");
            let b = c
                .send(C::ReadLlmSettings(FoundationLlmRead {
                    workspace_id: id(10),
                }))
                .unwrap()
                .llm_budget
                .unwrap();
            assert_eq!(b.used_minor, 0);
            assert_eq!(b.reserved_minor, 0);
        }
    }
}
struct Keys {
    root: std::path::PathBuf,
}
impl Drop for Keys {
    fn drop(&mut self) {
        // Only entries referenced by encrypted artifacts in this randomly created test root.
        let scope = format!("{}:{}", id(1), self.root.canonicalize().unwrap().display());
        for path in std::fs::read_dir(self.root.join("objects"))
            .unwrap()
            .flatten()
        {
            if let Ok(bytes) = std::fs::read(path.path())
                && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
                && matches!(
                    v["kind"].as_str(),
                    Some("publication_llm_config" | "publication_tts_config")
                )
                && let Some(r) = v["secret"]["reference"].as_str()
            {
                let entry = keyring::Entry::new(
                    if v["kind"] == "publication_tts_config" {
                        "nai.workbench.v43.tts"
                    } else {
                        "nai.workbench.v43.llm"
                    },
                    &format!("{scope}:{}:{r}", id(10)),
                )
                .unwrap();
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => (),
                    Err(e) => panic!("synthetic test credential cleanup: {e}"),
                }
            }
        }
    }
}
fn setup(c: &mut Client, endpoint: String) -> FoundationResult {
    c.send(workspace()).unwrap();
    let mut refs = Vec::new();
    for (n, doc) in [
        (
            80,
            serde_json::from_str::<Value>(include_str!(
                "../../../tests/fixtures/contracts/preparation.json"
            ))
            .unwrap(),
        ),
        (
            81,
            json!({"schema_version":1,"kind":"host_draft","name":"test host","style":"professional","preferences":""}),
        ),
        (
            82,
            json!({"schema_version":1,"kind":"series_draft","name":"test series","goal":"test"}),
        ),
    ] {
        let r = c
            .send(C::PutPreparation(FoundationObject {
                workspace_id: id(10),
                object_id: id(n),
                expected_revision: 0,
                body: doc.to_string(),
            }))
            .unwrap();
        refs.push(json!({"object_id":id(n),"revision":r.revision,"sha256":r.sha256}));
    }
    c.send(C::PutPreparation(FoundationObject{workspace_id:id(10),object_id:id(83),expected_revision:0,body:json!({"schema_version":1,"kind":"show_draft","name":"test show","product":refs[0],"host":refs[1],"series":refs[2],"cues":[{"id":"intro","title":"test","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}).to_string()})).unwrap();
    let report = c
        .send(C::PublicationImpact(FoundationImpact {
            workspace_id: id(10),
            report_id: id(85),
            show_id: id(83),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }))
        .unwrap();
    let snap = c
        .send(C::Publish(FoundationPublish {
            workspace_id: id(10),
            snapshot_id: id(90),
            report_id: id(85),
            report_sha256: report.sha256,
            acknowledge_unknown: true,
        }))
        .unwrap();
    c.send(C::SaveLlmModel(FoundationLlmModel {
        workspace_id: id(10),
        model_id: id(100),
        expected_revision: 0,
        name: "synthetic model".into(),
        base_url: endpoint,
        model: "test-model".into(),
        input_minor_per_million: 100,
        output_minor_per_million: 200,
        enabled: true,
        api_key: b"synthetic-key-never-production".to_vec(),
        prices_confirmed: true,
    }))
    .unwrap();
    let settings = c
        .send(C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(10),
        }))
        .unwrap();
    assert!(settings.llm_models[0].credential_saved);
    c.send(C::SetLlmBudget(FoundationLlmBudget {
        workspace_id: id(10),
        expected_revision: settings.llm_budget.unwrap().revision,
        limit_minor: 100,
    }))
    .unwrap();
    snap
}
fn start(c: &mut Client, snap: &FoundationResult, n: u64) -> FoundationResult {
    c.send(C::StartLlm(FoundationLlmStart {
        workspace_id: id(10),
        task_id: id(n),
        snapshot_id: id(90),
        snapshot_sha256: snap.sha256.clone(),
        cue_id: "intro".into(),
        model_id: id(100),
        model_revision: 1,
        max_cost_minor: 5,
    }))
    .unwrap()
}
fn local_check(c: &mut Client, snap: &FoundationResult) -> LlmLocalCheck {
    c.send(C::CheckLlm(FoundationLlmPlan {
        workspace_id: id(10),
        snapshot_id: id(90),
        snapshot_sha256: snap.sha256.clone(),
        cue_id: "intro".into(),
        model_id: id(100),
        model_revision: 1,
        max_cost_minor: 5,
    }))
    .unwrap()
    .llm_check
    .unwrap()
}
#[test]
fn real_native_local_check_verifies_binding_without_provider_or_budget_writes() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open(root.path(), true, 1);
    let snap = setup(&mut c, format!("http://{}/v1", server.server_addr()));
    for _ in 0..3 {
        let checked = local_check(&mut c, &snap);
        assert_eq!(checked.reason, LlmLocalReason::ReadyLocal as i32);
        assert!(checked.credential_checked && checked.estimate_known);
    }
    assert!(
        server
            .recv_timeout(Duration::from_millis(150))
            .unwrap()
            .is_none(),
        "local check contacted the provider"
    );
    let scope = format!(
        "{}:{}",
        id(1),
        root.path().canonicalize().unwrap().display()
    );
    let mut removed = 0;
    for file in std::fs::read_dir(root.path().join("objects"))
        .unwrap()
        .flatten()
    {
        let bytes = std::fs::read(file.path()).unwrap();
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes)
            && v["kind"] == "publication_llm_config"
        {
            let r = v["secret"]["reference"].as_str().unwrap();
            keyring::Entry::new("nai.workbench.v43.llm", &format!("{scope}:{}:{r}", id(10)))
                .unwrap()
                .delete_credential()
                .unwrap();
            removed += 1;
        }
    }
    assert_eq!(removed, 1);
    let checked = local_check(&mut c, &snap);
    assert_eq!(checked.reason, LlmLocalReason::CredentialUnavailable as i32);
    assert!(!checked.credential_checked);
    let b = c
        .send(C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(10),
        }))
        .unwrap()
        .llm_budget
        .unwrap();
    assert_eq!(b.used_minor, 0);
    assert_eq!(b.reserved_minor, 0);
    assert_eq!(b.unknown_minor, 0);
    assert!(
        server
            .recv_timeout(Duration::from_millis(150))
            .unwrap()
            .is_none()
    );
}
fn read(c: &mut Client, n: u64) -> FoundationResult {
    c.send(C::ReadLlmTask(FoundationLlmTaskRead {
        workspace_id: id(10),
        task_id: id(n),
    }))
    .unwrap()
}
fn wait(c: &mut Client, n: u64) -> FoundationResult {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let r = read(c, n);
        if r.task_state != AgentTaskState::Running as i32
            && r.task_state != AgentTaskState::Queued as i32
        {
            return r;
        }
        assert!(Instant::now() < deadline, "bounded LLM completion");
        std::thread::sleep(Duration::from_millis(30));
    }
}
#[test]
fn real_native_llm_completion_is_private_and_adoption_is_only_a_draft() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open(root.path(), true, 1);
    let snap = setup(&mut c, format!("http://{}/v1", server.server_addr()));
    let worker = std::thread::spawn(move || {
        let mut request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("real HTTP request");
        assert_eq!(request.url(), "/v1/chat/completions");
        let mut text = String::new();
        request.as_reader().read_to_string(&mut text).unwrap();
        let body: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["max_tokens"], 256);
        assert_eq!(body["stream"], false);
        assert!(text.contains("标准款"));
        request.respond(tiny_http::Response::from_string(json!({"choices":[{"finish_reason":"stop","message":{"content":json!({"text":"标准款容量为500毫升。","fact_ids":["capacity"]}).to_string()}}],"usage":{"prompt_tokens":100,"completion_tokens":20}}).to_string())).unwrap();
        assert!(
            server
                .recv_timeout(Duration::from_millis(150))
                .unwrap()
                .is_none()
        );
    });
    let r = start(&mut c, &snap, 110);
    assert_eq!(r.task_state, AgentTaskState::Running as i32);
    let source_context = r.agent_context.clone().expect("business LLM context");
    let pool = c
        .send(C::ReadQuestionPool(QuestionPoolRead {
            workspace_id: id(10),
            snapshot_id: id(90),
            snapshot_sha256: snap.sha256.clone(),
        }))
        .unwrap();
    assert_eq!(
        Some(source_context.clone()),
        pool.question_pool.unwrap().context
    );
    let done = wait(&mut c, 110);
    assert_eq!(
        done.task_state,
        AgentTaskState::Completed as i32,
        "{}",
        done.task_reason
    );
    assert_eq!(done.agent_context, Some(source_context));
    assert_eq!(done.charge_status, "known");
    assert_eq!(done.actual_minor, 1);
    assert!(!done.body.contains("\"input\":"));
    assert!(done.body.contains("标准款容量为500毫升"));
    let adopt = c
        .send(C::ResolveLlm(FoundationLlmResolve {
            workspace_id: id(10),
            task_id: id(110),
            task_sha256: done.sha256,
            decision: FaqDecision::Accept as i32,
            edited_text: String::new(),
            acknowledge_edit: false,
        }))
        .unwrap();
    assert_eq!(adopt.adopted_revision, 2);
    assert_eq!(adopt.suggestion_state, SuggestionState::Accepted as i32);
    assert!(adopt.body.is_empty());
    let product = c
        .send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(80),
            revision: 0,
        }))
        .unwrap();
    let p: Value = serde_json::from_str(&product.body).unwrap();
    assert_eq!(p["talk_tracks"].as_array().unwrap().len(), 2);
    // Explicit operator steps: read latest show, select adopted track, CAS save, review, publish.
    let old_snapshot = c
        .send(C::ReadPublication(FoundationPublicationRead {
            workspace_id: id(10),
            snapshot_id: id(90),
        }))
        .unwrap();
    let show = c
        .send(C::ReadObject(FoundationReadObject {
            workspace_id: id(10),
            object_id: id(83),
            revision: 0,
        }))
        .unwrap();
    let mut show_doc: Value = serde_json::from_str(&show.body).unwrap();
    assert_eq!(show_doc["product"]["revision"], 1);
    assert_eq!(show_doc["cues"][0]["talk_track_id"], "opening");
    show_doc["product"] =
        json!({"object_id":id(80),"revision":product.revision,"sha256":product.sha256});
    show_doc["cues"][0]["talk_track_id"] = p["talk_tracks"][1]["id"].clone();
    show_doc["cues"][0]["mandatory_fact_ids"] = p["talk_tracks"][1]["fact_ids"].clone();
    let saved = c
        .send(C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(83),
            expected_revision: show.revision,
            body: show_doc.to_string(),
        }))
        .unwrap();
    let impact = c
        .send(C::PublicationImpact(FoundationImpact {
            workspace_id: id(10),
            report_id: id(150),
            show_id: id(83),
            show_revision: saved.revision,
            previous_snapshot_id: id(90),
        }))
        .unwrap();
    let published = c
        .send(C::Publish(FoundationPublish {
            workspace_id: id(10),
            snapshot_id: id(151),
            report_id: id(150),
            report_sha256: impact.sha256,
            acknowledge_unknown: true,
        }))
        .unwrap();
    let still_old = c
        .send(C::ReadPublication(FoundationPublicationRead {
            workspace_id: id(10),
            snapshot_id: id(90),
        }))
        .unwrap();
    assert_eq!(old_snapshot.body, still_old.body);
    assert_eq!(old_snapshot.sha256, still_old.sha256);
    let old_preview = c
        .send(C::Preview(FoundationPreview {
            workspace_id: id(10),
            preview_id: id(152),
            snapshot_id: id(90),
            snapshot_sha256: snap.sha256.clone(),
            cue_id: "intro".into(),
            session_id: id(153),
            expected_epoch: 0,
        }))
        .unwrap();
    c.send(C::StopPreview(FoundationPreviewStop {
        workspace_id: id(10),
        session_id: id(153),
        expected_epoch: old_preview.epoch,
    }))
    .unwrap();
    let new_preview = c
        .send(C::Preview(FoundationPreview {
            workspace_id: id(10),
            preview_id: id(154),
            snapshot_id: id(151),
            snapshot_sha256: published.sha256,
            cue_id: "intro".into(),
            session_id: id(155),
            expected_epoch: 0,
        }))
        .unwrap();
    assert!(old_preview.body.contains("这是一款便携水杯"));
    assert!(new_preview.body.contains("标准款容量为500毫升。"));
    assert!(!new_preview.body.contains("这是一款便携水杯"));
    assert_ne!(old_preview.body, new_preview.body);
    c.send(C::StopPreview(FoundationPreviewStop {
        workspace_id: id(10),
        session_id: id(155),
        expected_epoch: new_preview.epoch,
    }))
    .unwrap();

    worker.join().unwrap();
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    assert!(read(&mut c, 110).body.is_empty());
}
#[test]
fn real_native_cancel_and_killed_owner_keep_unknown_cost_and_never_resend() {
    for cancel in [true, false] {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let root = tempdir().unwrap();
        let _keys = Keys {
            root: root.path().into(),
        };
        let mut c = Client::open(root.path(), true, 1);
        let snap = setup(&mut c, format!("http://{}/v1", server.server_addr()));
        start(&mut c, &snap, 120);
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("HTTP reached the synthetic provider");
        let clock = Instant::now();
        c.send(C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(10),
        }))
        .unwrap();
        assert!(
            clock.elapsed() < Duration::from_secs(1),
            "network blocked normal control"
        );
        if cancel {
            let clock = Instant::now();
            c.send(C::CancelLlm(FoundationLlmTaskRead {
                workspace_id: id(10),
                task_id: id(120),
            }))
            .unwrap();
            assert!(clock.elapsed() < Duration::from_secs(1));
            let r = read(&mut c, 120);
            assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
            assert!(r.body.is_empty());
            assert_eq!(r.charge_status, "unknown");
        }
        drop(c);
        let _ = request.respond(tiny_http::Response::from_string(
            "late private candidate must be hidden",
        ));
        let mut recovered = Client::open(root.path(), false, 1);
        let r = read(&mut recovered, 120);
        assert!(r.body.is_empty());
        assert_eq!(r.charge_status, "unknown");
        assert!(r.llm_budget.unwrap().unknown_minor > 0);
        assert!(
            server
                .recv_timeout(Duration::from_millis(250))
                .unwrap()
                .is_none(),
            "restart resent a billable request"
        );
    }
}
#[test]
fn real_native_model_test_without_product_returns_plain_text_and_forbids_adoption() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    c.send(C::SaveLlmModel(FoundationLlmModel {
        workspace_id: id(10),
        model_id: id(100),
        expected_revision: 0,
        name: "synthetic probe".into(),
        base_url: format!("http://{}/v1", server.server_addr()),
        model: "test-model".into(),
        input_minor_per_million: 100,
        output_minor_per_million: 200,
        enabled: true,
        api_key: b"synthetic-probe-key".to_vec(),
        prices_confirmed: true,
    }))
    .unwrap();
    let worker = std::thread::spawn(move || {
        let mut req = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        let mut text = String::new();
        req.as_reader().read_to_string(&mut text).unwrap();
        let body: Value = serde_json::from_str(&text).unwrap();
        assert!(body.get("response_format").is_none());
        assert_eq!(
            body["messages"][1]["content"],
            nai_runtime_core::llm::TEST_INPUT
        );
        assert_eq!(body["max_tokens"], 256);
        req.respond(tiny_http::Response::from_string(json!({"choices":[{"finish_reason":"stop","message":{"content":"NAI_OK"}}],"usage":{"prompt_tokens":10,"completion_tokens":3}}).to_string())).unwrap();
        assert!(
            server
                .recv_timeout(Duration::from_millis(100))
                .unwrap()
                .is_none()
        );
    });
    c.send(C::TestLlm(FoundationModelTest {
        workspace_id: id(10),
        task_id: id(91),
        model_id: id(100),
        model_revision: 1,
    }))
    .unwrap();
    let r = wait(&mut c, 91);
    worker.join().unwrap();
    assert_eq!(r.task_state, AgentTaskState::Completed as i32);
    assert_eq!(r.task_reason, "API_RESPONSE_OK");
    assert_eq!(r.suggestion_state, 0);
    assert!(r.body.contains("NAI_OK"));
    assert_eq!(r.actual_minor, 0);
    assert_eq!(
        c.send(C::ResolveLlm(FoundationLlmResolve {
            workspace_id: id(10),
            task_id: id(91),
            task_sha256: r.sha256,
            decision: FaqDecision::Accept as i32,
            ..Default::default()
        })),
        Err(WorkbenchError::InvalidArgument)
    );
}
#[test]
fn real_native_tts_config_persists_and_removed_models_keep_sealed_history() {
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    let model = FoundationTtsModel {
        protocol: "openai_speech".into(),
        language: String::new(),
        workspace_id: id(10),
        model_id: id(110),
        expected_revision: 0,
        name: "synthetic speech model".into(),
        base_url: "https://example.invalid/v1".into(),
        model: "speech-test".into(),
        voice: "voice-test".into(),
        enabled: true,
        api_key: b"synthetic-speech-key".to_vec(),
        price_known: false,
        character_minor_per_million: 0,
    };
    let saved = c.send(C::SaveTtsModel(model.clone())).unwrap();
    assert_eq!(saved.tts_models[0].voice, "voice-test");
    assert!(saved.tts_models[0].credential_saved);
    assert!(!format!("{saved:?}").contains("synthetic-speech-key"));
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let read = c
        .send(C::ReadLlmSettings(FoundationLlmRead {
            workspace_id: id(10),
        }))
        .unwrap();
    assert_eq!(read.tts_models.len(), 1);
    assert!(!read.tts_models[0].price_known);
    let mut update = model;
    update.expected_revision = 1;
    update.api_key.clear();
    update.voice = "voice-updated".into();
    let edited = c.send(C::SaveTtsModel(update)).unwrap();
    assert_eq!(edited.revision, 2);
    assert!(edited.tts_models[0].credential_saved);
    let removed = c
        .send(C::DeleteTtsModel(FoundationDelete {
            workspace_id: id(10),
            object_id: id(110),
            expected_revision: 2,
        }))
        .unwrap();
    assert!(removed.tts_models.is_empty());
    assert_eq!(removed.revision, 3);
    assert!(
        std::fs::read_dir(root.path().join("objects"))
            .unwrap()
            .flatten()
            .any(|p| std::fs::read(p.path())
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_some_and(|v| v["kind"] == "publication_tts_config" && !v["secret"].is_null()))
    );
}
#[test]
fn real_native_tts_test_settles_unknown_and_returns_metadata_without_pcm() {
    native_tts_audio_case(false);
}
#[test]
#[ignore = "requires actual Windows default audio device; silent PCM only"]
fn real_native_tts_audition_device_submission() {
    native_tts_audio_case(true);
}
fn native_tts_audio_case(device: bool) {
    native_tts_audio_with_delay(device, Duration::ZERO);
}
#[test]
fn real_native_tts_model_test_waits_beyond_old_fifteen_seconds() {
    native_tts_audio_with_delay(false, Duration::from_secs(16));
}
fn native_tts_audio_with_delay(device: bool, delay: Duration) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", server.server_addr());
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open_mode(root.path(), true, 1, !delay.is_zero());
    c.send(workspace()).unwrap();
    c.send(C::SaveTtsModel(FoundationTtsModel {
        protocol: "openai_speech".into(),
        language: String::new(),
        workspace_id: id(10),
        model_id: id(120),
        name: "synthetic speech".into(),
        base_url: endpoint,
        model: "test".into(),
        voice: "test:voice".into(),
        enabled: true,
        api_key: b"synthetic-speech-key".to_vec(),
        price_known: true,
        character_minor_per_million: 100,
        ..Default::default()
    }))
    .unwrap();
    let samples = if device { 24000u32 } else { 4u32 };
    let worker = std::thread::spawn(move || {
        let mut r = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(r.url(), "/v1/audio/speech");
        let mut text = String::new();
        r.as_reader().read_to_string(&mut text).unwrap();
        let body: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["input"], nai_foundation_sqlite::TTS_TEST_TEXT);
        assert_eq!(body["sample_rate"], 24000);
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + samples * 2).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(24000u32.to_le_bytes());
        wav.extend(48000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((samples * 2).to_le_bytes());
        wav.resize(44 + samples as usize * 2, 0);
        std::thread::sleep(delay);
        r.respond(tiny_http::Response::from_data(wav)).unwrap();
        assert!(
            server
                .recv_timeout(Duration::from_millis(200))
                .unwrap()
                .is_none()
        );
    });
    let cmd = C::TestTts(FoundationTtsTest {
        workspace_id: id(10),
        task_id: id(121),
        model_id: id(120),
        model_revision: 1,
    });
    c.send(cmd).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5) + delay;
    let done = loop {
        let r = c
            .send(C::ReadTtsTask(FoundationLlmTaskRead {
                workspace_id: id(10),
                task_id: id(121),
            }))
            .unwrap();
        if r.task_state != AgentTaskState::Queued as i32
            && r.task_state != AgentTaskState::Running as i32
        {
            break r;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(30));
    };
    assert_eq!(
        done.task_state,
        AgentTaskState::Completed as i32,
        "{}",
        done.task_reason
    );
    assert_eq!(done.charge_status, "unknown");
    let value: Value = serde_json::from_str(&done.body).unwrap();
    assert_eq!(value["sample_count"], samples);
    let audio = FoundationTtsAudio {
        workspace_id: id(10),
        task_id: id(121),
        task_sha256: done.sha256.clone(),
    };
    let grant = c.send(C::AuthorizeTtsAudio(audio.clone())).unwrap();
    let lease = grant.tts_audio.unwrap();
    assert_eq!(lease.artifact_id, id(121));
    assert_eq!(lease.sample_count, u64::from(samples));
    assert_eq!(lease.sample_rate, 24000);
    assert!(lease.ttl_ms > 0 && lease.ttl_ms <= 500);
    let mut stale = audio.clone();
    stale.task_sha256 = "aa".repeat(32);
    assert_eq!(
        c.send(C::AuthorizeTtsAudio(stale)).unwrap_err(),
        Error::RevisionConflict
    );
    let renewed = c
        .send(C::AuthorizeTtsAudio(audio.clone()))
        .unwrap()
        .tts_audio
        .unwrap();
    assert_ne!(renewed.lease_epoch, lease.lease_epoch);
    // A queued old start must never survive a priority stop.
    std::fs::write(root.path().join("audition.stop"), "new-stop").unwrap();
    assert_eq!(
        c.send(C::StartTtsAudition(FoundationAuditionStart {
            audio: Some(audio.clone()),
            expected_stop_nonce: String::new()
        }))
        .unwrap_err(),
        Error::Cancelled
    );
    if device {
        let start = c
            .send(C::StartTtsAudition(FoundationAuditionStart {
                audio: Some(audio.clone()),
                expected_stop_nonce: "new-stop".into(),
            }))
            .unwrap();
        assert_eq!(
            start.audition.unwrap().state,
            AuditionState::Starting as i32
        );
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            std::thread::sleep(Duration::from_millis(80));
            let v = c
                .send(C::RenewTtsAudition(FoundationLlmTaskRead {
                    workspace_id: id(10),
                    task_id: id(121),
                }))
                .unwrap()
                .audition
                .unwrap();
            eprintln!(
                "native audition state={} count={} reason={}",
                v.state, v.submitted_samples, v.reason
            );
            if v.state == AuditionState::Choosing as i32 {
                select_test_device(&mut c, &v);
                continue;
            }
            assert_ne!(v.state, AuditionState::Unavailable as i32, "{}", v.reason);
            assert_ne!(v.state, AuditionState::Stopped as i32, "{}", v.reason);
            if v.state == AuditionState::Submitted as i32 {
                assert!(!v.selected_device_name.is_empty());
                assert_eq!(v.submitted_samples, u64::from(samples));
                assert_eq!(v.reason, "SUBMITTED_NOT_DEVICE_ACK");
                break;
            }
            assert!(Instant::now() < end, "device submission timed out");
        }
        c.send(C::StartTtsAudition(FoundationAuditionStart {
            audio: Some(audio.clone()),
            expected_stop_nonce: "new-stop".into(),
        }))
        .unwrap();
        // GUI renewal cannot revive the old player after its own deadline.
        std::thread::sleep(Duration::from_millis(750));
        let lost = c
            .send(C::RenewTtsAudition(FoundationLlmTaskRead {
                workspace_id: id(10),
                task_id: id(121),
            }))
            .unwrap()
            .audition
            .unwrap();
        assert_eq!(lost.state, AuditionState::Unavailable as i32);
        assert_eq!(lost.reason, "GUI_OR_INIT_DEADLINE");
        let stopped = c
            .send(C::StopTtsAudition(FoundationLlmTaskRead {
                workspace_id: id(10),
                task_id: id(121),
            }))
            .unwrap()
            .audition
            .unwrap();
        assert_eq!(stopped.state, AuditionState::Stopped as i32);
    }
    c.send(C::RevokeTtsAudio(audio.clone())).unwrap();
    assert_eq!(
        c.send(C::AuthorizeTtsAudio(audio)).unwrap_err(),
        Error::CapabilityUnavailable
    );

    assert_eq!(value["audio_sha256"].as_str().unwrap().len(), 64);
    assert!(value.get("pcm").is_none());
    assert!(!done.body.contains("synthetic-speech-key"));
    worker.join().unwrap();
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let done = c
        .send(C::ReadTtsTask(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(121),
        }))
        .unwrap();
    assert_eq!(done.charge_status, "unknown");
    assert_eq!(
        c.send(C::AuthorizeTtsAudio(FoundationTtsAudio {
            workspace_id: id(10),
            task_id: id(121),
            task_sha256: done.sha256
        }))
        .unwrap_err(),
        Error::CapabilityUnavailable
    );
}
#[test]
fn real_native_tts_cancel_discards_late_audio_and_keeps_unknown_after_restart() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", server.server_addr());
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    c.send(C::SaveTtsModel(FoundationTtsModel {
        protocol: "openai_speech".into(),
        language: String::new(),
        workspace_id: id(10),
        model_id: id(120),
        name: "synthetic speech".into(),
        base_url: url,
        model: "test".into(),
        voice: "test:voice".into(),
        enabled: true,
        api_key: b"synthetic-speech-key".to_vec(),
        price_known: true,
        character_minor_per_million: 100,
        ..Default::default()
    }))
    .unwrap();
    let (tx, rx) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let r = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        tx.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(5)).unwrap();
        let _ = r.respond(tiny_http::Response::from_data(
            b"late invalid audio".to_vec(),
        ));
        assert!(
            server
                .recv_timeout(Duration::from_millis(200))
                .unwrap()
                .is_none()
        );
    });
    c.send(C::TestTts(FoundationTtsTest {
        workspace_id: id(10),
        task_id: id(121),
        model_id: id(120),
        model_revision: 1,
    }))
    .unwrap();
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let now = Instant::now();
    let r = c
        .send(C::CancelTts(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(121),
        }))
        .unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
    assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
    assert_eq!(r.charge_status, "unknown");
    assert!(r.body.is_empty());
    release.send(()).unwrap();
    worker.join().unwrap();
    let r = c
        .send(C::ReadTtsTask(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(121),
        }))
        .unwrap();
    assert_eq!(r.task_state, AgentTaskState::Cancelled as i32);
    assert!(r.body.is_empty());
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let r = c
        .send(C::ReadTtsTask(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(121),
        }))
        .unwrap();
    assert_eq!(r.charge_status, "unknown");
    assert!(r.body.is_empty());
}

#[test]
fn real_native_published_speech_http_and_revocation() {
    published_speech_case(false, false, true, false, false, 0);
}
#[test]
fn real_native_published_speech_late_response_after_session_stop() {
    published_speech_case(false, true, false, false, false, 0);
}
#[test]
#[ignore = "real Windows default sound output, silent fixture"]
fn real_native_published_speech_device_stop_and_revocation() {
    for revoke in [false, true] {
        published_speech_case(true, false, revoke, false, false, 0);
    }
}
#[test]
fn real_native_public_guard_blocks_rewrite() {
    published_speech_case(false, false, true, true, false, 0);
}
#[test]
#[ignore = "real Windows default device initialization, permanently muted, explicit native verification"]
fn real_native_public_window_muted_preload() {
    published_speech_case(false, false, true, false, true, 0);
}
#[test]
#[ignore = "real Windows default device, silent PCM fixture, explicit execution verification"]
fn real_native_public_execution() {
    published_speech_case(false, false, true, false, true, 1);
}
#[test]
#[ignore = "real Windows output with silent PCM; explicit stop/revoke/root loss/P0/health-only matrix"]
fn real_native_public_execution_faults() {
    for mode in 2..=6 {
        published_speech_case(false, false, true, false, true, mode);
    }
}
fn published_speech_case(
    device: bool,
    late: bool,
    revoke: bool,
    rewrite: bool,
    preload: bool,
    playback: u8,
) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", server.server_addr());
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open_mode(root.path(), true, 1, preload || device);
    c.send(workspace()).unwrap();
    let mut save = |n, body: String| {
        c.send(C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(n),
            expected_revision: 0,
            body,
        }))
        .unwrap()
    };
    let mut product_doc: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/contracts/preparation.json"
    ))
    .unwrap();
    if rewrite {
        product_doc["talk_tracks"][0]["text"] = "这款水杯适合出行，标准款容量为500毫升。".into();
    }
    let product = save(80, product_doc.to_string());
    let host=save(81,json!({"schema_version":1,"kind":"host_draft","name":"host","style":"professional","preferences":""}).to_string());
    let series = save(
        82,
        json!({"schema_version":1,"kind":"series_draft","name":"series","goal":"说明"}).to_string(),
    );
    let ptr = |n, r: &FoundationResult| json!({"object_id":id(n),"revision":r.revision,"sha256":r.sha256});
    save(83,json!({"schema_version":1,"kind":"show_draft","name":"show","product":ptr(80,&product),"host":ptr(81,&host),"series":ptr(82,&series),"cues":[{"id":"intro","title":"intro","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}).to_string());
    let report = c
        .send(C::PublicationImpact(FoundationImpact {
            workspace_id: id(10),
            report_id: id(85),
            show_id: id(83),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }))
        .unwrap();
    let snap = c
        .send(C::Publish(FoundationPublish {
            workspace_id: id(10),
            snapshot_id: id(90),
            report_id: id(85),
            report_sha256: report.sha256,
            acknowledge_unknown: true,
        }))
        .unwrap();
    if playback == 1 {
        c.send(C::CreateLiveRun(LiveRunCreate {
            scope: Some(QuestionPoolRead {
                workspace_id: id(10),
                snapshot_id: id(90),
                snapshot_sha256: snap.sha256.clone(),
            }),
            run_id: id(92),
        }))
        .unwrap();
    }
    let card = c
        .send(C::Preview(FoundationPreview {
            workspace_id: id(10),
            preview_id: id(91),
            snapshot_id: id(90),
            snapshot_sha256: snap.sha256.clone(),
            cue_id: "intro".into(),
            session_id: id(92),
            expected_epoch: if playback == 1 { 1 } else { 0 },
        }))
        .unwrap();
    c.send(C::SaveTtsModel(FoundationTtsModel {
        protocol: "openai_speech".into(),
        language: String::new(),
        workspace_id: id(10),
        model_id: id(120),
        name: "synthetic speech".into(),
        base_url: endpoint,
        model: "test".into(),
        voice: "test:voice".into(),
        enabled: true,
        api_key: b"synthetic-speech-key".to_vec(),
        price_known: true,
        character_minor_per_million: 100,
        ..Default::default()
    }))
    .unwrap();
    let plan = FoundationSpeechPlan {
        workspace_id: id(10),
        preview_id: id(91),
        preview_sha256: card.sha256,
        model_id: id(120),
        model_revision: 1,
        max_cost_minor: 5,
        task_id: id(121),
    };
    let q = c
        .send(C::CheckPreviewSpeech(plan.clone()))
        .unwrap()
        .speech_plan
        .unwrap();
    let source_context = q.agent_context.clone().expect("speech context");
    let pool = c
        .send(C::ReadQuestionPool(QuestionPoolRead {
            workspace_id: id(10),
            snapshot_id: id(90),
            snapshot_sha256: snap.sha256.clone(),
        }))
        .unwrap();
    assert_eq!(Some(source_context), pool.question_pool.unwrap().context);
    let expected = product_doc;
    assert_eq!(q.text, expected["talk_tracks"][0]["text"]);
    assert!(
        server
            .recv_timeout(Duration::from_millis(30))
            .unwrap()
            .is_none(),
        "read-only plan sent HTTP"
    );
    let (sent, received) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let wanted = q.text.clone();
    let worker = std::thread::spawn(move || {
        let mut r = server
            .recv_timeout(Duration::from_secs(8))
            .unwrap()
            .unwrap();
        let mut text = String::new();
        r.as_reader().read_to_string(&mut text).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["input"], wanted);
        assert_eq!(v["voice"], "test:voice");
        sent.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(8)).unwrap();
        let count = 72000u32;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + count * 2).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(24000u32.to_le_bytes());
        wav.extend(48000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((count * 2).to_le_bytes());
        wav.resize(44 + count as usize * 2, 0);
        let _ = r.respond(tiny_http::Response::from_data(wav));
        assert!(
            server
                .recv_timeout(Duration::from_millis(200))
                .unwrap()
                .is_none(),
            "automatic retry"
        );
    });
    let mut start = FoundationSpeechStart {
        plan: Some(plan.clone()),
        task_id: id(121),
        approval_sha256: "0".repeat(64),
    };
    assert_eq!(
        c.send(C::SynthesizePreviewSpeech(start.clone()))
            .unwrap_err(),
        Error::RevisionConflict
    );
    start.approval_sha256 = q.approval_sha256;
    c.send(C::SynthesizePreviewSpeech(start)).unwrap();
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    let stop = |c: &mut Client| {
        if revoke {
            c.send(C::RevokePublication(FoundationPublicationRevoke {
                workspace_id: id(10),
                revocation_id: id(130),
                snapshot_id: id(90),
                snapshot_sha256: snap.sha256.clone(),
                reason: "wrong facts".into(),
            }))
            .unwrap();
        } else {
            c.send(C::StopPreview(FoundationPreviewStop {
                workspace_id: id(10),
                session_id: id(92),
                expected_epoch: 1,
            }))
            .unwrap();
        }
    };
    if late {
        stop(&mut c);
    }
    release.send(()).unwrap();
    let end = Instant::now() + Duration::from_secs(8);
    let done = loop {
        let r = c
            .send(C::ReadTtsTask(FoundationLlmTaskRead {
                workspace_id: id(10),
                task_id: id(121),
            }))
            .unwrap();
        if !matches!(r.task_state,x if x==AgentTaskState::Queued as i32||x==AgentTaskState::Running as i32)
        {
            break r;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(30));
    };
    worker.join().unwrap();
    assert_eq!(done.charge_status, "unknown");
    let audio = FoundationTtsAudio {
        workspace_id: id(10),
        task_id: id(121),
        task_sha256: done.sha256,
    };
    if late {
        assert!(done.body.is_empty());
        assert!(c.send(C::AuthorizeTtsAudio(audio)).is_err());
        return;
    }
    assert_eq!(done.task_state, AgentTaskState::Completed as i32);
    assert_eq!(
        serde_json::from_str::<Value>(&done.body).unwrap()["kind"],
        "publication_tts_preview"
    );
    let mut public_request = FoundationPublicCandidate {
        workspace_id: id(10),
        preview_id: id(91),
        preview_sha256: plan.preview_sha256.clone(),
        task_id: id(121),
        task_sha256: audio.task_sha256.clone(),
        output_observation: None,
    };
    if !device {
        let prepared_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let unbound = loop {
            let r = c
                .send(C::CheckPublicCandidate(public_request.clone()))
                .unwrap()
                .public_candidate_check
                .unwrap();
            if r.prepared_media.is_some() {
                break r;
            }
            assert!(
                std::time::Instant::now() < prepared_deadline,
                "media preparation timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let receipt = unbound
            .prepared_media_receipt
            .as_ref()
            .expect("actual child receipt");
        let prepared = unbound.prepared_media.as_ref().unwrap();
        assert_eq!(
            receipt.descriptor_sha256,
            nai_contracts::prepared_media_hash(prepared)
        );
        assert_eq!(receipt.sample_count, 72000);
        assert_eq!(receipt.tail_frame.as_ref().unwrap().sample_start, 71200);
        assert_eq!(receipt.tail_frame.as_ref().unwrap().sample_count, 800);
        assert_ne!(receipt.consumer_instance_id, prepared.task_id);
        assert_eq!(prepared.expected_frames, 90);
        assert_eq!(prepared.renderer_id, "rms-sample-v1");
        assert_eq!(prepared.pcm_format, "s16le-mono-24000");
        assert_eq!(prepared.audio.as_ref().unwrap().sample_count, 72000);
        assert!(unbound.output_instance.is_none());
        assert_eq!(
            unbound.guard_review.unwrap().reason,
            PublicGuardReason::OutputNotBound as i32
        );
        let keeper = std::env::var_os("NAI_PUBLIC_OUTPUT_KEEPER")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_BIN_EXE_nai-foundation-engine"))
                    .with_file_name("nai-public-output-keeper.exe")
            });
        let mut window =
            nai_workbench_host::public_output::OutputOwner::open_at(&keeper, root.path(), true)
                .unwrap();
        let challenge = c
            .send(C::PreparePublicOutputBinding(true))
            .unwrap()
            .public_output_challenge
            .unwrap();
        let window_status = window.poll().unwrap();
        let output_id = window_status.output_id.clone();
        public_request.output_observation = Some(PublicOutputObservation {
            challenge: challenge.challenge,
            status: Some(window_status),
            stop_nonce: String::new(),
        });
        let public_check = c
            .send(C::CheckPublicCandidate(public_request.clone()))
            .unwrap()
            .public_candidate_check
            .unwrap();
        assert_eq!(public_check.text, q.text);
        assert_eq!(
            public_check.output_instance.as_ref().unwrap().output_id,
            output_id
        );
        assert_eq!(
            public_check
                .guard_review
                .as_ref()
                .unwrap()
                .object
                .as_ref()
                .unwrap()
                .output_target_id,
            output_id
        );
        assert_eq!(public_check.audio.as_ref().unwrap().sample_count, 72000);
        assert_eq!(
            public_check
                .axes
                .iter()
                .filter(|a| a.state == PublicCandidateAxisState::Passed as i32)
                .count(),
            if rewrite { 4 } else { 5 }
        );
        let guard = public_check.guard_review.as_ref().unwrap();
        assert_eq!(guard.policy_passed, !rewrite);
        assert_eq!(
            guard.object_sha256,
            nai_contracts::public_guard_hash(guard.object.as_ref().unwrap())
        );
        assert_eq!(
            guard.reason,
            if rewrite {
                PublicGuardReason::UnsupportedSpeech
            } else {
                PublicGuardReason::ExactSpecPassed
            } as i32
        );
        assert_eq!(public_check.visual.as_ref().unwrap().width, 512);
        assert_eq!(public_check.visual_license, "MIT");
        assert_eq!(public_check.approval_available, !rewrite);
        let bind = |c: &mut Client, w: &mut nai_workbench_host::public_output::OutputOwner| {
            let ch = c
                .send(C::PreparePublicOutputBinding(true))
                .unwrap()
                .public_output_challenge
                .unwrap();
            let mut r = public_request.clone();
            r.output_observation = Some(PublicOutputObservation {
                challenge: ch.challenge,
                status: Some(w.poll().unwrap()),
                stop_nonce: String::new(),
            });
            r
        };
        if !rewrite {
            for failure in 0..3 {
                let r = bind(&mut c, &mut window);
                let checked = c
                    .send(C::CheckPublicCandidate(r))
                    .unwrap()
                    .public_candidate_check
                    .unwrap();
                let b = checked.approval_draft.unwrap().bundle.unwrap();
                let mut req = FoundationPublicApproval {
                    candidate: Some(bind(&mut c, &mut window)),
                    bundle_sha256: b.approved_sha256.clone(),
                    acknowledge_public_rights: true,
                };
                match failure {
                    0 => req.acknowledge_public_rights = false,
                    1 => req.bundle_sha256 = "f".repeat(64),
                    _ => req.candidate.as_mut().unwrap().preview_sha256 = "f".repeat(64),
                }
                assert!(
                    c.send(C::ConfirmPublicApproval(req)).is_err(),
                    "invalid approval {failure}"
                );
                let repeat = FoundationPublicApproval {
                    candidate: Some(bind(&mut c, &mut window)),
                    bundle_sha256: b.approved_sha256,
                    acknowledge_public_rights: true,
                };
                assert!(
                    c.send(C::ConfirmPublicApproval(repeat)).is_err(),
                    "failed confirmation must consume draft"
                );
            }
            let r = bind(&mut c, &mut window);
            let checked = c
                .send(C::CheckPublicCandidate(r))
                .unwrap()
                .public_candidate_check
                .unwrap();
            let b = checked.approval_draft.unwrap().bundle.unwrap();
            let req = FoundationPublicApproval {
                candidate: Some(bind(&mut c, &mut window)),
                bundle_sha256: b.approved_sha256.clone(),
                acknowledge_public_rights: true,
            };
            let approved = c
                .send(C::ConfirmPublicApproval(req.clone()))
                .unwrap()
                .public_approval
                .unwrap();
            assert!(approved.approved);
            assert!(!approved.playback_available);
            assert_eq!(approved.bundle.as_ref(), Some(&b));
            let candidate = bind(&mut c, &mut window);
            let transfer = c
                .send(C::PreparePublicWindow(FoundationPublicWindowPrepare {
                    candidate: Some(candidate),
                    bundle_sha256: b.approved_sha256.clone(),
                }))
                .unwrap()
                .private_window_preparation
                .unwrap();
            let exported = std::path::PathBuf::from(&transfer.data_path);
            assert!(exported.is_file());
            assert_eq!(exported.parent(), Some(root.path()));
            assert_eq!(transfer.output_id, output_id);
            if preload {
                let preparation = (*transfer).clone();
                let ready = window
                    .prepare(*transfer)
                    .expect("actual window preparation");
                assert!(
                    ready.device_ready,
                    "{} caption={}",
                    ready.reason, ready.caption_state
                );
                assert!(ready.caption_ready);
                assert_eq!(ready.caption_sha256, b.caption_sha256);
                assert_eq!(ready.caption_state, PublicCaptionState::Ready as i32);
                assert!(!ready.device_name.is_empty());
                assert_eq!(ready.submitted_samples, 0);
                let receipt = ready.receipt.unwrap();
                assert_eq!(receipt.consumer_instance_id, output_id);
                assert_eq!(receipt.descriptor_sha256, b.prepared_media_sha256);
                assert_eq!(receipt.sample_count, 72000);
                if playback > 0 {
                    let mut first = true;
                    let mut last = 0;
                    let mut frames = 0;
                    let mut caption_seen = false;
                    let until = Instant::now() + Duration::from_secs(5);
                    loop {
                        let cycle = Instant::now();
                        let actual = window
                            .fresh_execution()
                            .expect("fresh local execution challenge");
                        if actual
                            .window_prepared
                            .as_ref()
                            .is_some_and(|p| p.submission_complete)
                        {
                            last = actual.window_prepared.as_ref().unwrap().submitted_samples;
                            break;
                        }
                        let candidate = bind(&mut c, &mut window);
                        let req = FoundationPublicWindowPrepare {
                            candidate: Some(candidate),
                            bundle_sha256: b.approved_sha256.clone(),
                        };
                        let result=c.send(if first{C::StartPublicExecution(req)}else{C::RenewPublicExecution(req)}).unwrap_or_else(|e|{let _=c.child.kill();let _=c.child.wait();let mut diagnostic=String::new();if let Some(mut stderr)=c.child.stderr.take(){let _=stderr.read_to_string(&mut diagnostic);}panic!("root grant failed first={first} last={last} elapsed={:?} error={e:?} diagnostic={diagnostic}",until.elapsed());});
                        let lease = result
                            .private_execution_lease
                            .expect("private execution grant");
                        eprintln!(
                            "EXECUTION_CYCLE seq={} root_ms={} ttl={}",
                            lease.seq,
                            cycle.elapsed().as_millis(),
                            lease.lease_ms
                        );
                        assert_eq!(lease.preparation_id, ready.preparation_id);
                        assert!(lease.lease_ms <= 500);
                        let state=window.execute(*lease).unwrap_or_else(|e|{let _=c.child.kill();let _=c.child.wait();let mut diagnostic=String::new();if let Some(mut stderr)=c.child.stderr.take(){let _=stderr.read_to_string(&mut diagnostic);}let p=window.current().window_prepared;panic!("consumer first={first} last={last} error={e} view={:?} diagnostic={diagnostic}",p.map(|p|(p.reason,p.execution_seq,p.submitted_samples,p.execution_challenge)));});
                        assert!(state.submitted_samples >= last);
                        last = state.submitted_samples;
                        if let Some(f) = state.frame {
                            frames = frames.max(f.frame_index);
                        }
                        if let Some(p) = window.current().presentation.as_ref()
                            && p.caption_visible
                        {
                            assert_eq!(p.caption_sha256, b.caption_sha256);
                            assert_eq!(p.preparation_id, ready.preparation_id);
                            assert!(p.submission_clock);
                            caption_seen = true;
                        }
                        if playback > 1 && last >= 2400 {
                            assert!(last < 72000);
                            match playback {
                                2 => {
                                    window.clear_prepared().unwrap();
                                    c.send(C::StopPublicExecution(PublicApprovalRevoke {
                                        workspace_id: id(10),
                                        approval_id: b.approval_id.clone(),
                                    }))
                                    .unwrap();
                                    let stopped = window.poll().unwrap();
                                    assert!(stopped.window_prepared.is_none());
                                    assert!(stopped.presentation.is_none());
                                    assert!(!exported.exists());
                                    return;
                                }
                                3 => {
                                    c.send(C::RevokePublicApproval(PublicApprovalRevoke {
                                        workspace_id: id(10),
                                        approval_id: b.approval_id.clone(),
                                    }))
                                    .unwrap();
                                    let candidate = bind(&mut c, &mut window);
                                    assert!(
                                        c.send(C::RenewPublicExecution(
                                            FoundationPublicWindowPrepare {
                                                candidate: Some(candidate),
                                                bundle_sha256: b.approved_sha256.clone()
                                            }
                                        ))
                                        .is_err()
                                    );
                                    assert!(!exported.exists());
                                }
                                4 => {
                                    c.child.kill().unwrap();
                                    c.child.wait().unwrap();
                                }
                                5 => {
                                    nai_workbench_host::public_output::stop(root.path()).unwrap();
                                }
                                6 => (),
                                _ => unreachable!(),
                            }
                            let until = Instant::now() + Duration::from_millis(750);
                            loop {
                                let status = window.poll().unwrap();
                                if playback == 5 && status.terminal {
                                    assert!(status.window_prepared.is_none());
                                    assert!(status.presentation.is_none());
                                    assert!(!status.audio_active);
                                    return;
                                }
                                if let Some(p) = status.window_prepared.as_ref()
                                    && p.execution_stopped
                                {
                                    assert!(!status.terminal, "health remains alive");
                                    assert!(!status.audio_active && !p.playing);
                                    assert!(status.presentation.is_none());
                                    assert!(p.submitted_samples < 72000);
                                    std::thread::sleep(Duration::from_millis(80));
                                    let stopped = window
                                        .poll()
                                        .unwrap()
                                        .window_prepared
                                        .unwrap()
                                        .submitted_samples;
                                    std::thread::sleep(Duration::from_millis(80));
                                    assert_eq!(
                                        window
                                            .poll()
                                            .unwrap()
                                            .window_prepared
                                            .unwrap()
                                            .submitted_samples,
                                        stopped
                                    );
                                    if playback == 4 {
                                        assert!(!exported.exists());
                                    }
                                    return;
                                }
                                assert!(
                                    Instant::now() < until,
                                    "mode {playback} did not independently stop"
                                );
                                std::thread::sleep(Duration::from_millis(15));
                            }
                        }
                        if state.submission_complete {
                            assert_eq!(last, 72000);
                            break;
                        }
                        first = false;
                        assert!(Instant::now() < until, "source samples did not finish");
                        std::thread::sleep(Duration::from_millis(25));
                        let s = window.poll().unwrap();
                        if s.window_prepared
                            .as_ref()
                            .is_some_and(|p| p.submission_complete)
                        {
                            last = s.window_prepared.as_ref().unwrap().submitted_samples;
                            break;
                        }
                    }
                    assert_eq!(last, 72000);
                    assert!(caption_seen, "actual window caption was never submitted");
                    assert!(window.current().presentation.is_none());
                    assert!(frames > 1, "actual sample driven frames must advance");
                    let candidate = bind(&mut c, &mut window);
                    let completed = c
                        .send(C::RenewPublicExecution(FoundationPublicWindowPrepare {
                            candidate: Some(candidate),
                            bundle_sha256: b.approved_sha256.clone(),
                        }))
                        .expect("root submission observation");
                    assert!(completed.private_execution_lease.is_none());
                    assert!(
                        completed
                            .public_window_prepared
                            .unwrap()
                            .submission_complete
                    );
                    if playback == 1 {
                        let progress = c
                            .send(C::ReadLiveRun(LiveRunRead {
                                workspace_id: id(10),
                                run_id: id(92),
                            }))
                            .unwrap()
                            .live_context
                            .unwrap();
                        assert_eq!(progress.cursor, 1);
                        assert_eq!(progress.submitted_count, 1);
                        assert_eq!(progress.state, LiveRunState::Completed as i32);
                    }
                    assert!(!exported.exists());
                    let candidate = bind(&mut c, &mut window);
                    assert!(
                        c.send(C::StartPublicExecution(FoundationPublicWindowPrepare {
                            candidate: Some(candidate),
                            bundle_sha256: b.approved_sha256.clone()
                        }))
                        .is_err(),
                        "start cannot replay active registration"
                    );
                    window.clear_prepared().unwrap();
                    c.send(C::StopPublicExecution(PublicApprovalRevoke {
                        workspace_id: id(10),
                        approval_id: b.approval_id.clone(),
                    }))
                    .unwrap();
                    return;
                }
                let end = Instant::now() + Duration::from_secs(7);
                loop {
                    std::thread::sleep(Duration::from_millis(80));
                    c.send(C::ReadTtsTask(FoundationLlmTaskRead {
                        workspace_id: id(10),
                        task_id: id(121),
                    }))
                    .unwrap();
                    let status = window.poll().unwrap();
                    assert!(!status.audio_active && !status.live_ready);
                    if status.window_prepared.is_none() {
                        break;
                    }
                    assert_eq!(status.window_prepared.unwrap().submitted_samples, 0);
                    assert!(Instant::now() < end, "muted preload did not retire");
                }
                assert!(window.prepare(preparation.clone()).unwrap().device_ready);
                window.clear_prepared().unwrap();
                assert!(window.poll().unwrap().window_prepared.is_none());
                assert!(window.prepare(preparation).unwrap().device_ready);
                nai_workbench_host::public_output::stop(root.path()).unwrap();
                let stopped = window.poll().unwrap();
                assert!(stopped.terminal);
                assert!(stopped.window_prepared.is_none());
                assert!(!stopped.audio_active);
                assert!(exported.exists());
                c.child.kill().unwrap();
                c.child.wait().unwrap();
                let until = Instant::now() + Duration::from_secs(2);
                while exported.exists() {
                    assert!(
                        Instant::now() < until,
                        "OS did not remove crashed producer's PCM export"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                return;
            }

            assert!(
                c.send(C::ConfirmPublicApproval(req)).is_err(),
                "successful draft cannot replay"
            );
            let revoked = c
                .send(C::RevokePublicApproval(PublicApprovalRevoke {
                    workspace_id: id(10),
                    approval_id: b.approval_id.clone(),
                }))
                .unwrap()
                .public_approval
                .unwrap();
            assert!(revoked.revoked);
            assert!(
                !exported.exists(),
                "revocation must retire private data file"
            );
            assert!(!revoked.approved);
            assert!(!window.poll().unwrap().audio_active);
            // This case specifically verifies long-lived muted preparation and retirement.
            // Rebind/guard/private-audition cases below have independent named tests.
            if preload {
                return;
            }
        } else {
            assert!(public_check.approval_draft.is_none());
        }

        assert!(
            c.send(C::CheckPublicCandidate(public_request.clone()))
                .is_err(),
            "consumed challenge replay"
        );
        drop(window);
        let mut replacement =
            nai_workbench_host::public_output::OutputOwner::open_at(&keeper, root.path(), true)
                .unwrap();
        let challenge = c
            .send(C::PreparePublicOutputBinding(true))
            .unwrap()
            .public_output_challenge
            .unwrap();
        let status = replacement.poll().unwrap();
        assert_ne!(status.output_id, output_id);
        let mut rebound = public_request.clone();
        rebound.output_observation = Some(PublicOutputObservation {
            challenge: challenge.challenge,
            status: Some(status),
            stop_nonce: String::new(),
        });
        let second = c
            .send(C::CheckPublicCandidate(rebound))
            .unwrap()
            .public_candidate_check
            .unwrap();
        assert_ne!(
            second.guard_review.unwrap().object_sha256,
            guard.object_sha256
        );
        drop(replacement);
        let mut missing = public_request.clone();
        missing.output_observation = None;
        assert!(
            c.send(C::CheckPublicCandidate(missing))
                .unwrap()
                .public_candidate_check
                .unwrap()
                .output_instance
                .is_none()
        );
        let mut stale_public = public_request.clone();
        stale_public.output_observation = None;
        stale_public.preview_sha256 = "f".repeat(64);
        assert!(c.send(C::CheckPublicCandidate(stale_public)).is_err());
    }
    c.send(C::AuthorizeTtsAudio(audio.clone())).unwrap();
    if device {
        c.send(C::StartTtsAudition(FoundationAuditionStart {
            audio: Some(audio.clone()),
            expected_stop_nonce: String::new(),
        }))
        .unwrap();
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            std::thread::sleep(Duration::from_millis(80));
            let v = c
                .send(C::RenewTtsAudition(FoundationLlmTaskRead {
                    workspace_id: id(10),
                    task_id: id(121),
                }))
                .unwrap()
                .audition
                .unwrap();
            assert_ne!(v.state, AuditionState::Unavailable as i32, "{}", v.reason);
            if v.state == AuditionState::Choosing as i32 {
                select_test_device(&mut c, &v);
                continue;
            }
            if v.state == AuditionState::Playing as i32 && v.submitted_samples > 0 {
                break;
            }
            assert!(Instant::now() < end);
        }
    }
    stop(&mut c);
    public_request.output_observation = None;
    assert!(c.send(C::CheckPublicCandidate(public_request)).is_err());
    if device {
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            std::thread::sleep(Duration::from_millis(80));
            let v = c
                .send(C::RenewTtsAudition(FoundationLlmTaskRead {
                    workspace_id: id(10),
                    task_id: id(121),
                }))
                .unwrap()
                .audition
                .unwrap();
            if matches!(v.state,x if x==AuditionState::Unavailable as i32||x==AuditionState::Stopped as i32)
            {
                assert!(v.submitted_samples < 72000);
                break;
            }
            assert!(Instant::now() < end, "revoked audio kept playing");
        }
    }
    assert!(c.send(C::AuthorizeTtsAudio(audio)).is_err());
    let r = c
        .send(C::ReadTtsTask(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(121),
        }))
        .unwrap();
    assert!(r.body.is_empty());
    assert_eq!(r.charge_status, "unknown");
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    let r = c
        .send(C::ReadTtsTask(FoundationLlmTaskRead {
            workspace_id: id(10),
            task_id: id(121),
        }))
        .unwrap();
    assert!(r.body.is_empty());
    assert_eq!(r.charge_status, "unknown");
}

fn select_test_device(c: &mut Client, v: &AuditionView) {
    assert_eq!(v.submitted_samples, 0);
    assert!(!v.devices.is_empty());
    assert!(v.devices.len() <= 16);
    let device = v
        .devices
        .iter()
        .find(|d| d.is_default)
        .unwrap_or(&v.devices[0]);
    let mut s = FoundationOutputDeviceSelect {
        workspace_id: id(10),
        task_id: id(121),
        selection: Some(OutputDeviceSelection {
            output_session_id: "0".repeat(64),
            device_id: device.device_id,
        }),
    };
    assert_eq!(
        c.send(C::SelectOutputDevice(s.clone())).unwrap_err(),
        Error::RevisionConflict
    );
    s.selection.as_mut().unwrap().output_session_id = v.output_session_id.clone();
    s.selection.as_mut().unwrap().device_id = 17;
    assert_eq!(
        c.send(C::SelectOutputDevice(s.clone())).unwrap_err(),
        Error::InvalidArgument
    );
    s.selection.as_mut().unwrap().device_id = device.device_id;
    let chosen = c.send(C::SelectOutputDevice(s)).unwrap().audition.unwrap();
    assert_eq!(chosen.state, AuditionState::Starting as i32);
}
#[test]
#[ignore = "requires explicitly authorized local Qwen API at 127.0.0.1:8000; two real syntheses, no playback"]
fn real_local_qwen_live_sentence_identity_and_text_batch() {
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let mut c = Client::open_mode(root.path(), true, 1, true);
    c.send(workspace()).unwrap();
    let mut save = |n, body: String| {
        c.send(C::PutPreparation(FoundationObject {
            workspace_id: id(10),
            object_id: id(n),
            expected_revision: 0,
            body,
        }))
        .unwrap()
    };
    let mut product: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/contracts/preparation.json"
    ))
    .unwrap();
    let mut weight = product["facts"][0].clone();
    weight["id"] = "weight".into();
    weight["label"] = "重量".into();
    weight["value"] = "100".into();
    weight["unit"] = "克".into();
    product["facts"].as_array_mut().unwrap().push(weight);
    product["talk_tracks"][0]["fact_ids"] = json!(["capacity", "weight"]);
    product["talk_tracks"][0]["text"] = "标准款容量为500毫升。标准款重量为100克。".into();
    let p = save(80, product.to_string());
    let h=save(81,json!({"schema_version":1,"kind":"host_draft","name":"local acceptance","style":"professional","preferences":""}).to_string());
    let series=save(82,json!({"schema_version":1,"kind":"series_draft","name":"local acceptance","goal":"verify sentences"}).to_string());
    let ptr = |n, r: &FoundationResult| json!({"object_id":id(n),"revision":r.revision,"sha256":r.sha256});
    save(83,json!({"schema_version":1,"kind":"show_draft","name":"local Qwen sentence acceptance","product":ptr(80,&p),"host":ptr(81,&h),"series":ptr(82,&series),"cues":[{"id":"intro","title":"规格","talk_track_id":"opening","mandatory_fact_ids":["capacity","weight"],"allow_questions":true}]}).to_string());
    let report = c
        .send(C::PublicationImpact(FoundationImpact {
            workspace_id: id(10),
            report_id: id(85),
            show_id: id(83),
            show_revision: 1,
            previous_snapshot_id: String::new(),
        }))
        .unwrap();
    let snap = c
        .send(C::Publish(FoundationPublish {
            workspace_id: id(10),
            snapshot_id: id(90),
            report_id: id(85),
            report_sha256: report.sha256,
            acknowledge_unknown: true,
        }))
        .unwrap();
    let live = c
        .send(C::CreateLiveRun(LiveRunCreate {
            scope: Some(QuestionPoolRead {
                workspace_id: id(10),
                snapshot_id: id(90),
                snapshot_sha256: snap.sha256,
            }),
            run_id: id(92),
        }))
        .unwrap()
        .live_context
        .unwrap();
    assert_eq!(live.sentence_total, 2);
    let scope = LiveRunRead {
        workspace_id: id(10),
        run_id: id(92),
    };
    let plan = c
        .send(C::ReadLiveSpeechPlan(scope.clone()))
        .unwrap()
        .live_speech_plan
        .unwrap();
    c.send(C::ControlLiveBatch(LiveBatchControl {
        run: Some(scope.clone()),
        expected_revision: live.revision,
        expected_epoch: live.epoch,
        approval_id: id(93),
        plan_sha256: plan.plan_sha256.clone(),
        acknowledge_rights: true,
        revoke: false,
    }))
    .unwrap();
    let approved = c
        .send(C::ReadLiveSpeechPlan(scope.clone()))
        .unwrap()
        .live_speech_plan
        .unwrap();
    assert!(approved.batch_approval.unwrap().approved);
    c.send(C::SaveTtsModel(FoundationTtsModel {
        workspace_id: id(10),
        model_id: id(120),
        name: "local Qwen acceptance (isolated)".into(),
        base_url: "http://127.0.0.1:8000/v1".into(),
        model: "qwen3-tts".into(),
        voice: "serena".into(),
        language: "chinese".into(),
        protocol: "qwen_local".into(),
        enabled: true,
        ..Default::default()
    }))
    .unwrap();
    let mut digests = Vec::new();
    for index in 0..2u32 {
        let card = c
            .send(C::PreviewLiveSentence(LiveSentencePreview {
                run: Some(scope.clone()),
                preview_id: id(200 + u64::from(index)),
                sentence_cursor: index,
            }))
            .unwrap();
        let doc: Value = serde_json::from_str(&card.body).unwrap();
        assert_eq!(doc["text"], plan.sentences[index as usize].text);
        assert_eq!(doc["live_sentence"]["plan_sha256"], plan.plan_sha256);
        assert_eq!(doc["live_sentence"]["sentence_cursor"], index);
        let request = FoundationSpeechPlan {
            workspace_id: id(10),
            preview_id: card.resource_id,
            preview_sha256: card.sha256,
            model_id: id(120),
            model_revision: 1,
            max_cost_minor: 5,
            task_id: id(220 + u64::from(index)),
        };
        let checked = c
            .send(C::CheckPreviewSpeech(request.clone()))
            .unwrap()
            .speech_plan
            .unwrap();
        assert_eq!(checked.text, plan.sentences[index as usize].text);
        let started = c
            .send(C::SynthesizePreviewSpeech(FoundationSpeechStart {
                plan: Some(request.clone()),
                task_id: request.task_id.clone(),
                approval_sha256: checked.approval_sha256,
            }))
            .unwrap();
        let mut result = started;
        let until = Instant::now() + Duration::from_secs(20);
        while matches!(
            AgentTaskState::try_from(result.task_state).unwrap(),
            AgentTaskState::Queued | AgentTaskState::Running
        ) {
            assert!(Instant::now() < until, "local synthesis deadline");
            std::thread::sleep(Duration::from_millis(100));
            result = c
                .send(C::ReadTtsTask(FoundationLlmTaskRead {
                    workspace_id: id(10),
                    task_id: request.task_id.clone(),
                }))
                .unwrap();
        }
        assert_eq!(
            result.task_state,
            AgentTaskState::Completed as i32,
            "local synthesis failed: {}",
            result.task_reason
        );
        let a = c
            .send(C::AuthorizeTtsAudio(FoundationTtsAudio {
                workspace_id: id(10),
                task_id: request.task_id,
                task_sha256: result.sha256,
            }))
            .unwrap()
            .tts_audio
            .unwrap();
        assert!(a.sample_count > 0);
        digests.push(a.pcm_sha256);
    }
    assert_ne!(digests[0], digests[1]);
    let unchanged = c.send(C::ReadLiveRun(scope)).unwrap().live_context.unwrap();
    assert_eq!(unchanged.sentence_cursor, 0);
    assert_eq!(unchanged.submitted_sentences, 0);
}

// The root scope is unique to this test; no existing user credential is touched.
struct SharedTestKeys {
    root: std::path::PathBuf,
}
impl Drop for SharedTestKeys {
    fn drop(&mut self) {
        let scope = format!("{}:{}", id(1), self.root.canonicalize().unwrap().display());
        let db = rusqlite::Connection::open(self.root.join("foundation.sqlite")).unwrap();
        for table in ["shared_model_staging", "shared_model_versions"] {
            let mut q = db
                .prepare(&format!("SELECT body FROM {table} WHERE body IS NOT NULL"))
                .unwrap();
            let rows = q.query_map([], |r| r.get::<_, String>(0)).unwrap();
            for body in rows.flatten() {
                if let Ok(doc) = serde_json::from_str::<Value>(&body)
                    && let Some(reference) = doc["secret"]["reference"].as_str()
                {
                    let name = format!("{scope}:00000000-0000-4000-8000-000000004802:{reference}");
                    let service = if doc["kind"] == "publication_tts_config" {
                        "nai.workbench.v43.tts"
                    } else {
                        "nai.workbench.v43.llm"
                    };
                    let entry = keyring::Entry::new(service, &name).unwrap();
                    match entry.delete_credential() {
                        Ok(()) | Err(keyring::Error::NoEntry) => (),
                        Err(e) => panic!("synthetic shared credential cleanup: {e}"),
                    }
                }
            }
        }
    }
}
#[test]
fn real_native_avatar_migration_preserves_synthetic_credential_after_restart() {
    use sha2::{Digest, Sha256};
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", server.server_addr());
    let root = tempdir().unwrap();
    let _keys = Keys {
        root: root.path().into(),
    };
    let _shared = SharedTestKeys {
        root: root.path().into(),
    };
    let mut c = Client::open(root.path(), true, 1);
    c.send(workspace()).unwrap();
    c.send(C::SaveLlmModel(FoundationLlmModel {
        workspace_id: id(10),
        model_id: id(100),
        name: "synthetic".into(),
        base_url: endpoint,
        model: "test".into(),
        enabled: true,
        api_key: b"synthetic-avatar-migration-key".to_vec(),
        ..Default::default()
    }))
    .unwrap();
    c.send(C::SaveTtsModel(FoundationTtsModel {
        protocol: "openai_speech".into(),
        workspace_id: id(10),
        model_id: id(101),
        name: "synthetic speech".into(),
        base_url: format!("http://{}/v1", server.server_addr()),
        model: "speech".into(),
        voice: "test:voice".into(),
        enabled: true,
        api_key: b"synthetic-avatar-tts-key".to_vec(),
        ..Default::default()
    }))
    .unwrap();
    let ids = ["base", "closed", "open"];
    let assets: [&[u8]; 3] = [b"base", b"closed", b"open"];
    let mut h = Sha256::new();
    h.update(b"manifest");
    h.update(b"config");
    h.update(b"license");
    for (name, bytes) in ids.iter().zip(assets) {
        h.update(name.as_bytes());
        h.update(bytes);
    }
    let package = h
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let rights = Sha256::digest(b"license")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let dir = root.path().join("avatar-packages").join(&package);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in [
        ("manifest.json", b"manifest".as_slice()),
        ("config.json", b"config".as_slice()),
        ("LICENSE.txt", b"license".as_slice()),
        ("preview.png", b"preview".as_slice()),
    ] {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    std::fs::write(
        dir.join("asset-ids.json"),
        serde_json::to_vec(&ids).unwrap(),
    )
    .unwrap();
    for (i, bytes) in assets.iter().enumerate() {
        std::fs::write(dir.join(format!("asset-{i}.rgba")), bytes).unwrap();
    }
    let avatar=json!({"schema_version":1,"kind":"avatar_profile_draft","name":"avatar","avatar_id":"avatar","package_version":"1","package_sha256":package,"license_sha256":rights,"width":2,"height":1,"selected":false,"revoked":false}).to_string();
    c.send(C::PutPreparation(FoundationObject {
        workspace_id: id(10),
        object_id: id(70),
        expected_revision: 0,
        body: avatar,
    }))
    .unwrap();
    let p = c
        .send(C::PreviewSharedModels(FoundationLlmRead::default()))
        .unwrap()
        .shared_model_preview
        .unwrap();
    assert_eq!(p.rows.len(), 2);
    let entries = p
        .rows
        .iter()
        .map(|row| {
            let mut e = row.source.clone().unwrap();
            e.catalog_model_id = if e.capability == "llm" {
                id(910)
            } else {
                id(911)
            };
            e
        })
        .collect();
    let request = SharedModelMigration {
        migration_id: id(900),
        plan_sha256: p.plan_sha256,
        entries,
    };
    c.send(C::MigrateSharedModels(request.clone())).unwrap();
    c.send(C::MigrateSharedModels(request.clone())).unwrap();
    c.send(C::MigrateSharedModels(request.clone())).unwrap();
    c.send(C::MigrateSharedModels(request.clone())).unwrap();
    drop(c);
    let mut c = Client::open(root.path(), false, 1);
    assert!(
        c.send(C::MigrateSharedModels(request))
            .unwrap()
            .shared_catalog_enabled
    );
    let worker = std::thread::spawn(move || {
        let req = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(req.headers().iter().any(|h| h.field.equiv("Authorization")
            && h.value.as_str() == "Bearer synthetic-avatar-migration-key"));
        req.respond(tiny_http::Response::from_string(json!({"choices":[{"finish_reason":"stop","message":{"content":"NAI_OK"}}],"usage":{"prompt_tokens":10,"completion_tokens":3}}).to_string())).unwrap();
        let req = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(req.url(), "/v1/audio/speech");
        assert!(req.headers().iter().any(|h| h.field.equiv("Authorization")
            && h.value.as_str() == "Bearer synthetic-avatar-tts-key"));
        let samples = 4u32;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + samples * 2).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(24000u32.to_le_bytes());
        wav.extend(48000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((samples * 2).to_le_bytes());
        wav.resize(44 + samples as usize * 2, 0);
        req.respond(tiny_http::Response::from_data(wav)).unwrap();
    });
    c.send(C::TestLlm(FoundationModelTest {
        workspace_id: id(10),
        task_id: id(91),
        model_id: id(910),
        model_revision: 1,
    }))
    .unwrap();
    let result = wait(&mut c, 91);
    assert_eq!(result.task_state, AgentTaskState::Completed as i32);
    assert!(result.body.contains("NAI_OK"));
    c.send(C::TestTts(FoundationTtsTest {
        workspace_id: id(10),
        task_id: id(92),
        model_id: id(911),
        model_revision: 1,
    }))
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let tts = loop {
        let r = c
            .send(C::ReadTtsTask(FoundationLlmTaskRead {
                workspace_id: id(10),
                task_id: id(92),
            }))
            .unwrap();
        if r.task_state != AgentTaskState::Queued as i32
            && r.task_state != AgentTaskState::Running as i32
        {
            break r;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(30));
    };
    worker.join().unwrap();
    assert_eq!(
        tts.task_state,
        AgentTaskState::Completed as i32,
        "{}",
        tts.task_reason
    );
    assert_eq!(std::fs::read(dir.join("asset-1.rgba")).unwrap(), b"closed");
}

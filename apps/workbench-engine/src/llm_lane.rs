//! One cancellable private network task; workers have no database access.
use nai_contracts::workbench::*;
use nai_foundation_sqlite::{Error, Store};
use nai_runtime_core::foundation::Operation;
#[cfg(test)]
use std::sync::Arc;
use std::sync::mpsc;
use zeroize::Zeroize;
fn llm_target(command: &FoundationCommand) -> Option<(&str, &str)> {
    match command.operation.as_ref()? {
        foundation_command::Operation::StartLlm(x) => Some((&x.workspace_id, &x.task_id)),
        foundation_command::Operation::TestLlm(x) => Some((&x.workspace_id, &x.task_id)),
        _ => None,
    }
}
pub struct Lane {
    active: Option<Active>,
    allow_loopback: bool,
}
struct Active {
    workspace: String,
    id: String,
    cancel: Operation,
    reply: mpsc::Receiver<LlmWorkerResult>,
    deadline: std::time::Instant,
    settled: bool,
}
impl Lane {
    pub fn new(allow_loopback: bool) -> Self {
        Self {
            active: None,
            allow_loopback,
        }
    }
    pub fn validate_command(&self, command: &FoundationCommand) -> Result<(), Error> {
        if let Some(foundation_command::Operation::SaveLlmModel(x)) = &command.operation {
            nai_llm_openai::endpoint(&x.base_url, self.allow_loopback)?;
        }
        if let Some(foundation_command::Operation::SaveTtsModel(x)) = &command.operation {
            nai_llm_openai::tts::profile_endpoint(
                &x.base_url,
                &x.protocol,
                &x.language,
                self.allow_loopback,
            )?;
        }
        Ok(())
    }
    pub fn before(
        &mut self,
        store: &mut Store,
        actor: &str,
        command: &FoundationCommand,
    ) -> Result<(), Error> {
        self.validate_command(command)?;
        if matches!(
            command.operation,
            Some(
                foundation_command::Operation::TestTts(_)
                    | foundation_command::Operation::SynthesizePreviewSpeech(_)
                    | foundation_command::Operation::PrepareLiveAuto(_)
            )
        ) && self.active.is_some()
        {
            return Err(Error::Busy);
        }
        if let Some((workspace, task)) = llm_target(command)
            && self
                .active
                .as_ref()
                .is_some_and(|a| a.id != task || a.workspace != workspace)
        {
            return Err(Error::Busy);
        }
        if let Some(foundation_command::Operation::CancelLlm(x)) = &command.operation {
            store.llm_current(&Operation::new(4000)?, actor, &x.workspace_id, &x.task_id)?;
            if let Some(active) = &self.active
                && active.id == x.task_id
                && active.workspace == x.workspace_id
            {
                active.cancel.cancel();
            }
        }
        Ok(())
    }
    pub fn after(
        &mut self,
        store: &mut Store,
        actor: &str,
        command: &FoundationCommand,
        result: &mut FoundationResult,
    ) -> Result<(), Error> {
        if let Some(foundation_command::Operation::WorkspaceLifecycle(x)) = &command.operation
            && x.action == WorkspaceAction::Recycle as i32
            && let Some(a) = &self.active
            && a.workspace == x.workspace_id
        {
            a.cancel.cancel();
        }
        if matches!(
            command.operation,
            Some(foundation_command::Operation::CheckLlm(_))
        ) && self.active.is_some()
            && let Some(check) = result.llm_check.as_mut()
        {
            check.reason = LlmLocalReason::Busy as i32;
        }
        if let Some((workspace, task)) = llm_target(command)
            && result.task_state == AgentTaskState::Queued as i32
        {
            let op = Operation::new(4000)?;
            let prepared =
                store
                    .llm_request(&op, actor, workspace, task)
                    .and_then(|mut request| {
                        let scopes =
                            Operation::model_task(request.deadline_ms).and_then(|parent| {
                                let child = parent.model_child(request.deadline_ms)?;
                                Ok((parent, child))
                            });
                        let (cancel, flag) = match scopes {
                            Ok(scopes) => scopes,
                            Err(error) => {
                                request.api_key.zeroize();
                                return Err(error);
                            }
                        };
                        if let Err(error) = store.claim_llm(&op, actor, workspace, task) {
                            request.api_key.zeroize();
                            return Err(error);
                        }
                        Ok((request, cancel, flag))
                    });
            match prepared {
                Ok((request, cancel, flag)) => {
                    let deadline = std::time::Instant::now()
                        + std::time::Duration::from_millis(u64::from(request.deadline_ms) + 250);

                    let (tx, rx) = mpsc::sync_channel(1);
                    let allow = self.allow_loopback;
                    let id = task.to_owned();
                    let fallback_id = id.clone();
                    let spawn = std::thread::Builder::new()
                        .name("nai-private-llm".into())
                        .spawn(move || {
                            let reply =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    nai_llm_openai::run(request, flag, allow)
                                }))
                                .unwrap_or_else(|_| {
                                    LlmWorkerResult {
                                        task_id: fallback_id,
                                        request_started: true,
                                        error: "WORKER_LOST".into(),
                                        ..Default::default()
                                    }
                                });
                            let _ = tx.send(reply);
                        });
                    if spawn.is_ok() {
                        self.active = Some(Active {
                            workspace: workspace.to_owned(),
                            id,
                            cancel,
                            reply: rx,
                            deadline,
                            settled: false,
                        });
                    } else {
                        store.finish_llm(
                            &Operation::new(4000)?,
                            actor,
                            workspace,
                            &LlmWorkerResult {
                                task_id: task.to_owned(),
                                error: "WORKER_UNAVAILABLE".into(),
                                request_started: false,
                                ..Default::default()
                            },
                        )?;
                    }
                }
                Err(error) => {
                    store.finish_llm(
                        &Operation::new(4000)?,
                        actor,
                        workspace,
                        &LlmWorkerResult {
                            task_id: task.to_owned(),
                            error: error.as_str_name().into(),
                            request_started: false,
                            ..Default::default()
                        },
                    )?;
                }
            }
            *result = store.llm_current(&Operation::new(4000)?, actor, workspace, task)?;
        }
        Ok(())
    }
    pub fn poll(&mut self, store: &mut Store, actor: &str) -> Result<(), Error> {
        let Some(active) = &mut self.active else {
            return Ok(());
        };
        match store.llm_current(&Operation::new(4000)?, actor, &active.workspace, &active.id) {
            Ok(r)
                if r.task_state == AgentTaskState::Cancelled as i32
                    || r.suggestion_state == SuggestionState::Expired as i32 =>
            {
                active.cancel.cancel()
            }
            Err(_) => active.cancel.cancel(),
            _ => (),
        }
        match active.reply.try_recv() {
            Ok(reply) => {
                if !active.settled {
                    store.finish_llm(&Operation::new(4000)?, actor, &active.workspace, &reply)?;
                }
                self.active.take();
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                store.finish_llm(
                    &Operation::new(4000)?,
                    actor,
                    &active.workspace,
                    &LlmWorkerResult {
                        task_id: active.id.clone(),
                        error: "WORKER_LOST".into(),
                        request_started: true,
                        ..Default::default()
                    },
                )?;
                self.active.take();
            }
            Err(mpsc::TryRecvError::Empty) => {
                if !active.settled && std::time::Instant::now() >= active.deadline {
                    active.cancel.cancel();
                    store.finish_llm(
                        &Operation::new(4000)?,
                        actor,
                        &active.workspace,
                        &LlmWorkerResult {
                            task_id: active.id.clone(),
                            error: "WORKER_STALLED".into(),
                            request_started: true,
                            ..Default::default()
                        },
                    )?;
                    // Keep the single worker slot occupied until the thread exits. A stuck OS resolver
                    // must not accumulate unbounded abandoned threads as users start new tasks.
                    active.settled = true;
                }
            }
        };
        Ok(())
    }
    pub fn stop(&mut self, store: &mut Store, actor: &str) {
        if let Some(active) = self.active.take() {
            active.cancel.cancel();
            let reply = active.reply.try_recv().unwrap_or_else(|_| LlmWorkerResult {
                task_id: active.id,
                error: "ENGINE_STOPPED".into(),
                request_started: true,
                ..Default::default()
            });
            let _ = store.finish_llm(
                &Operation::new(4000).expect("constant deadline"),
                actor,
                &active.workspace,
                &reply,
            );
        }
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        if let Some(active) = &self.active {
            active.cancel.cancel();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use nai_runtime_core::llm::CredentialVault;
    use serde_json::{Value, json};
    struct TestVault;
    impl CredentialVault for TestVault {
        fn seal(&self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Value, Error> {
            Ok(json!({"reference":id(200),"nonce":"a".repeat(24),"ciphertext":"b".repeat(34)}))
        }
        fn open(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &Value,
        ) -> Result<zeroize::Zeroizing<Vec<u8>>, Error> {
            Ok(zeroize::Zeroizing::new(b"test-only-key".to_vec()))
        }
    }
    fn id(n: u64) -> String {
        format!("00000000-0000-0000-0000-{n:012x}")
    }
    #[test]
    fn stalled_worker_settles_unknown_and_holds_one_slot_until_exit() {
        use foundation_command::Operation as C;
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(root.path(), &id(1), true, Arc::default()).unwrap();
        store.attach_llm_vault(Arc::new(TestVault));
        let mut sequence = 500;
        let mut call = |s: &mut Store, action: C| {
            sequence += 1;
            s.execute(
                &Operation::new(4000).unwrap(),
                &id(1),
                &FoundationCommand {
                    idempotency_key: id(sequence),
                    operation: Some(action),
                },
            )
            .unwrap()
        };
        call(
            &mut store,
            C::CreateWorkspace(FoundationWorkspace {
                workspace_id: id(10),
                name: "isolated worker test".into(),
                budget_limit_minor: 100,
                currency: "CNY".into(),
            }),
        );
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
                json!({"schema_version":1,"kind":"host_draft","name":"test","style":"professional","preferences":""}),
            ),
            (
                82,
                json!({"schema_version":1,"kind":"series_draft","name":"test","goal":"test"}),
            ),
        ] {
            let r = call(
                &mut store,
                C::PutPreparation(FoundationObject {
                    workspace_id: id(10),
                    object_id: id(n),
                    expected_revision: 0,
                    body: doc.to_string(),
                }),
            );
            refs.push(json!({"object_id":id(n),"revision":r.revision,"sha256":r.sha256}));
        }
        call(&mut store,C::PutPreparation(FoundationObject{workspace_id:id(10),object_id:id(83),expected_revision:0,body:json!({"schema_version":1,"kind":"show_draft","name":"test","product":refs[0],"host":refs[1],"series":refs[2],"cues":[{"id":"intro","title":"test","talk_track_id":"opening","mandatory_fact_ids":["capacity"],"allow_questions":true}]}).to_string()}));
        let report = call(
            &mut store,
            C::PublicationImpact(FoundationImpact {
                workspace_id: id(10),
                report_id: id(85),
                show_id: id(83),
                show_revision: 1,
                previous_snapshot_id: String::new(),
            }),
        );
        let snapshot = call(
            &mut store,
            C::Publish(FoundationPublish {
                workspace_id: id(10),
                snapshot_id: id(90),
                report_id: id(85),
                report_sha256: report.sha256,
                acknowledge_unknown: true,
            }),
        );
        call(
            &mut store,
            C::SaveLlmModel(FoundationLlmModel {
                workspace_id: id(10),
                model_id: id(100),
                expected_revision: 0,
                name: "test-only".into(),
                base_url: "https://example.invalid/v1".into(),
                model: "test-only".into(),
                input_minor_per_million: 100,
                output_minor_per_million: 100,
                enabled: true,
                api_key: b"test-only-key".to_vec(),
                prices_confirmed: true,
            }),
        );
        call(
            &mut store,
            C::StartLlm(FoundationLlmStart {
                workspace_id: id(10),
                task_id: id(110),
                snapshot_id: id(90),
                snapshot_sha256: snapshot.sha256.clone(),
                cue_id: "intro".into(),
                model_id: id(100),
                model_revision: 1,
                max_cost_minor: 5,
            }),
        );
        store
            .claim_llm(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        let flag = Operation::model_task(15000).unwrap();
        let mut lane = Lane {
            allow_loopback: false,
            active: Some(Active {
                workspace: id(10),
                id: id(110),
                cancel: flag.clone(),
                reply: receiver,
                deadline: std::time::Instant::now(),
                settled: false,
            }),
        };
        lane.poll(&mut store, &id(1)).unwrap();
        assert!(matches!(flag.check(), Err(Error::Cancelled)));
        assert!(lane.active.as_ref().unwrap().settled);
        let result = store
            .llm_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .unwrap();
        assert_eq!(result.task_reason, "WORKER_STALLED");
        assert_eq!(result.charge_status, "unknown");
        assert!(result.body.is_empty());
        assert!(result.llm_budget.unwrap().unknown_minor > 0);
        let next = FoundationCommand {
            operation: Some(C::StartLlm(FoundationLlmStart {
                workspace_id: id(10),
                task_id: id(111),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(lane.before(&mut store, &id(1), &next), Err(Error::Busy));
        let action = C::CheckLlm(FoundationLlmPlan {
            workspace_id: id(10),
            snapshot_id: id(90),
            snapshot_sha256: snapshot.sha256,
            cue_id: "intro".into(),
            model_id: id(100),
            model_revision: 1,
            max_cost_minor: 5,
        });
        let mut checked = call(&mut store, action.clone());
        assert_eq!(
            checked.llm_check.as_ref().unwrap().reason,
            LlmLocalReason::ReadyLocal as i32
        );
        lane.after(
            &mut store,
            &id(1),
            &FoundationCommand {
                operation: Some(action),
                ..Default::default()
            },
            &mut checked,
        )
        .unwrap();
        assert_eq!(
            checked.llm_check.unwrap().reason,
            LlmLocalReason::Busy as i32,
            "a logically settled task must not hide an occupied native worker slot"
        );
        sender
            .send(LlmWorkerResult {
                task_id: id(110),
                text: "late private data".into(),
                request_started: true,
                ..Default::default()
            })
            .unwrap();
        lane.poll(&mut store, &id(1)).unwrap();
        assert!(lane.active.is_none());
        let result = store
            .llm_current(&Operation::new(4000).unwrap(), &id(1), &id(10), &id(110))
            .unwrap();
        assert!(result.body.is_empty());
        assert_eq!(result.charge_status, "unknown");
    }
}

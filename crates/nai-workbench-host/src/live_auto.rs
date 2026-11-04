//! Trusted native orchestration. One bounded action per tick, never driven by page timers.
use super::{Owner, random_id};
use crate::public_output::OutputOwner;
use nai_contracts::workbench::*;
use prost::Message;
use std::{
    path::Path,
    time::{Duration, Instant},
};
pub(super) fn result(raw: &[u8]) -> Result<FoundationResult, String> {
    match FoundationResponse::decode(raw)
        .map_err(|_| "WB_PROTOCOL_ERROR")?
        .outcome
    {
        Some(foundation_response::Outcome::Result(r)) => Ok(r),
        Some(foundation_response::Outcome::Problem(p)) => Err(format!(
            "WB_{}",
            WorkbenchError::try_from(p.code)
                .map_err(|_| "WB_PROTOCOL_ERROR")?
                .as_str_name()
                .trim_start_matches("WORKBENCH_ERROR_")
        )),
        _ => Err("WB_PROTOCOL_ERROR".into()),
    }
}
#[derive(Clone)]
enum Phase {
    Poll,
    Prepare(u32),
    Inspect(LiveAutoTask, Instant),
    Confirm(LiveAutoTask, String),
    Start(LiveAutoTask, String),
}
pub(super) struct Coordinator {
    pub scope: LiveRunRead,
    pub grant_id: String,
    output_id: String,
    phase: Phase,
    primed: bool,
}
impl Coordinator {
    pub fn new(scope: LiveRunRead, v: LiveAutoView) -> Self {
        Self {
            scope,
            grant_id: v.grant_id,
            output_id: v.output_id,
            phase: Phase::Poll,
            primed: false,
        }
    }
    fn candidate(&self, t: &LiveAutoTask) -> FoundationPublicCandidate {
        FoundationPublicCandidate {
            workspace_id: self.scope.workspace_id.clone(),
            preview_id: t.preview_id.clone(),
            preview_sha256: t.preview_sha256.clone(),
            task_id: t.task_id.clone(),
            task_sha256: t.task_sha256.clone(),
            output_observation: None,
        }
    }
    fn send(
        owner: &mut Owner,
        operation: foundation_command::Operation,
        output: &mut OutputOwner,
    ) -> Result<FoundationResult, String> {
        result(&owner.send_pumping(
            FoundationCommand {
                idempotency_key: random_id()?,
                operation: Some(operation),
            },
            Some(output),
        )?)
    }
    pub fn phase_name(&self) -> &'static str {
        match self.phase {
            Phase::Poll => "poll",
            Phase::Prepare(_) => "prepare",
            Phase::Inspect(..) => "inspect",
            Phase::Confirm(..) => "confirm",
            Phase::Start(..) => "start",
        }
    }
    pub fn tick(
        &mut self,
        owner: &mut Owner,
        output: &mut OutputOwner,
        root: &Path,
    ) -> Result<bool, String> {
        use foundation_command::Operation as C;
        if output.current().terminal || output.current().output_id != self.output_id {
            return Err("WB_OUTPUT_CHANGED".into());
        }
        match self.phase.clone() {
            Phase::Poll => {
                let v = Self::send(owner, C::ReadLiveAuto(self.scope.clone()), output)?
                    .live_auto
                    .ok_or("WB_PROTOCOL_ERROR")?;
                if v.grant_id != self.grant_id || v.output_id != self.output_id {
                    return Err("WB_PERMISSION_DENIED".into());
                }
                if ["stopped", "failed", "blocked"].contains(&v.state.as_str()) {
                    return Err("WB_AUTHORITY_UNAVAILABLE".into());
                }
                if v.state == "question_pausing" {
                    return Ok(true);
                }
                if v.state == "question_waiting" {
                    self.primed = true;
                    self.phase = Phase::Prepare(v.sentence_cursor);
                    return Ok(true);
                }
                if v.state == "completed" {
                    Self::send(
                        owner,
                        C::StopLiveAuto(LiveAutoStop {
                            run: Some(self.scope.clone()),
                            grant_id: self.grant_id.clone(),
                            reason: "COMPLETED".into(),
                        }),
                        output,
                    )?;
                    return Ok(false);
                }
                if v.tasks.iter().any(|t| {
                    !matches!(
                        AgentTaskState::try_from(t.task_state),
                        Ok(AgentTaskState::Queued
                            | AgentTaskState::Running
                            | AgentTaskState::Completed)
                    )
                }) {
                    return Err("WB_TTS_FAILED".into());
                }
                if v.state == "paused" || v.state == "pausing" {
                    return Ok(true);
                }
                let current = v
                    .tasks
                    .iter()
                    .find(|t| t.sentence_cursor == v.sentence_cursor);
                if v.state == "playing" {
                    if v.sentence_cursor + 1 < v.sentence_total
                        && v.reserved_minor < v.max_cost_minor
                        && !v
                            .tasks
                            .iter()
                            .any(|t| t.sentence_cursor == v.sentence_cursor + 1)
                    {
                        self.phase = Phase::Prepare(v.sentence_cursor + 1);
                    }
                } else if let Some(t) = current {
                    if t.task_state == AgentTaskState::Completed as i32 {
                        if !self.primed
                            && v.tasks.iter().any(|next| {
                                next.sentence_cursor == v.sentence_cursor + 1
                                    && next.task_state != AgentTaskState::Completed as i32
                            })
                        {
                            return Ok(true);
                        }

                        if v.sentence_cursor + 1 < v.sentence_total
                            && v.reserved_minor < v.max_cost_minor
                            && !v
                                .tasks
                                .iter()
                                .any(|t| t.sentence_cursor == v.sentence_cursor + 1)
                        {
                            self.phase = Phase::Prepare(v.sentence_cursor + 1);
                        } else {
                            self.phase =
                                Phase::Inspect(t.clone(), Instant::now() + Duration::from_secs(5));
                        }
                    }
                } else {
                    self.phase = Phase::Prepare(v.sentence_cursor);
                }
            }
            Phase::Prepare(index) => {
                self.phase = Phase::Poll;
                let r = Self::send(
                    owner,
                    C::PrepareLiveAuto(LiveAutoPrepare {
                        run: Some(self.scope.clone()),
                        grant_id: self.grant_id.clone(),
                        sentence_cursor: index,
                        preview_id: random_id()?,
                        task_id: random_id()?,
                    }),
                    output,
                );
                match r {
                    Ok(_) => (),
                    Err(e) if e == "WB_BUSY" || e == "WB_CANCELLED" => (),
                    Err(e) => return Err(e),
                }
            }
            Phase::Inspect(t, until) => {
                if Instant::now() >= until {
                    return Err("WB_MEDIA_NOT_READY".into());
                }
                let r = result(&owner.public_command(
                    FoundationCommand {
                        idempotency_key: random_id()?,
                        operation: Some(C::CheckPublicCandidate(self.candidate(&t))),
                    },
                    Some(output),
                    root,
                )?)?;
                let c = r.public_candidate_check.ok_or("WB_PROTOCOL_ERROR")?;
                if let Some(d) = c.approval_draft {
                    let b = d.bundle.ok_or("WB_PROTOCOL_ERROR")?;
                    self.phase = Phase::Confirm(t, b.approved_sha256);
                } else if c
                    .axes
                    .iter()
                    .any(|a| a.state == PublicCandidateAxisState::Blocked as i32)
                {
                    return Err("WB_POLICY_BLOCKED".into());
                }
            }
            Phase::Confirm(t, sha) => {
                self.phase = Phase::Poll;
                let response = owner.public_command(
                    FoundationCommand {
                        idempotency_key: random_id()?,
                        operation: Some(C::ConfirmLiveAuto(LiveAutoConfirm {
                            run: Some(self.scope.clone()),
                            grant_id: self.grant_id.clone(),
                            candidate: Some(self.candidate(&t)),
                            bundle_sha256: sha.clone(),
                        })),
                    },
                    Some(output),
                    root,
                )?;
                match result(&response) {
                    Ok(r)
                        if r.public_approval
                            .as_ref()
                            .is_some_and(|a| a.approved && !a.revoked) =>
                    {
                        self.phase = Phase::Start(t, sha)
                    }
                    Err(e) if e == "WB_CANCELLED" || e == "WB_REVISION_CONFLICT" => (),
                    Err(e) => return Err(e),
                    _ => return Err("WB_PROTOCOL_ERROR".into()),
                }
            }
            Phase::Start(t, sha) => {
                self.phase = Phase::Poll;
                let response = owner.public_command(
                    FoundationCommand {
                        idempotency_key: random_id()?,
                        operation: Some(C::StartPublicExecution(FoundationPublicWindowPrepare {
                            candidate: Some(self.candidate(&t)),
                            bundle_sha256: sha,
                        })),
                    },
                    Some(output),
                    root,
                )?;
                match result(&response) {
                    Ok(r)
                        if r.public_window_prepared
                            .as_ref()
                            .is_some_and(|p| p.playing || p.submission_complete) =>
                    {
                        self.primed = true;
                    }
                    Err(e) if e == "WB_CANCELLED" || e == "WB_REVISION_CONFLICT" => (),
                    Err(e) => return Err(e),
                    _ => return Err("WB_PROTOCOL_ERROR".into()),
                }
            }
        }
        Ok(true)
    }
}

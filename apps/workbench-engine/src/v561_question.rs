//! Root-owned single question slot. No UI, network, audio or model dependencies.
//! This coordinator does not itself grant playback or perform automatic recovery.
use nai_contracts::{
    v561_live::{QuestionResult, QuestionTask, ResumeIntent},
    v561_live_validation,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Generating,
    Speaking,
    Resuming,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Busy,
    Stale,
    Deadline,
    Revoked,
    WrongPhase,
}

/// Trusted root observation, re-created before each transition; never from UI.
pub struct Authority<'a> {
    pub generation: u64,
    pub worker_alive: bool,
    pub content_valid: bool,
    pub content_generation: u64,
    pub plan_sha256: &'a [u8],
    pub sentence_count: u32,
}

/// Progress advances only on complete, matching device consumption receipts.
pub struct Cursor {
    intent: ResumeIntent,
}
impl Cursor {
    pub fn position(&self) -> u32 {
        self.intent.sentence_index
    }
    pub fn matches(&self, identity: &[u8; 32], count: u32) -> bool {
        self.intent.plan_sha256 == identity && self.intent.sentence_count == count
    }
    pub fn new(
        plan_sha256: Vec<u8>,
        content_generation: u64,
        sentence_count: u32,
    ) -> Result<Self, Error> {
        let intent = ResumeIntent {
            plan_sha256,
            content_generation,
            sentence_count,
            sentence_index: 0,
        };
        if !v561_live_validation::resume_intent(&intent) {
            return Err(Error::Invalid);
        }
        Ok(Self { intent })
    }
    pub fn complete(
        &mut self,
        index: u32,
        expected_samples: u64,
        played_samples: u64,
        completed: bool,
    ) -> Result<(), Error> {
        if !completed
            || expected_samples == 0
            || expected_samples > 144000
            || played_samples != expected_samples
            || index != self.intent.sentence_index
            || index >= self.intent.sentence_count
        {
            return Err(Error::Invalid);
        }
        self.intent.sentence_index += 1;
        Ok(())
    }
    pub fn resume(&self) -> Option<ResumeIntent> {
        (self.intent.sentence_index < self.intent.sentence_count).then(|| self.intent.clone())
    }
    #[cfg(windows)]
    pub(crate) fn restart_completed(&mut self) -> Result<(), Error> {
        if self.intent.sentence_index != self.intent.sentence_count {
            return Err(Error::Invalid);
        }
        self.intent.sentence_index = 0;
        Ok(())
    }
}

struct Pending {
    task: QuestionTask,
    deadline: Instant,
    phase: Phase,
}
#[derive(Default)]
pub struct Slot {
    active: Option<Pending>,
    last_task: u64,
}
impl Slot {
    pub fn phase(&self) -> Option<Phase> {
        self.active.as_ref().map(|p| p.phase)
    }
    pub fn cancel(&mut self) {
        self.active = None;
    }
    /// Call from the root's idle loop as well as before model/playback transitions.
    pub fn observe(&mut self, a: &Authority<'_>, now: Instant) {
        if self
            .active
            .as_ref()
            .is_some_and(|p| now >= p.deadline || !Self::authority(&p.task, a))
        {
            self.cancel();
        }
    }
    fn authority(task: &QuestionTask, a: &Authority<'_>) -> bool {
        task.generation == a.generation
            && a.worker_alive
            && task.resume.as_ref().is_none_or(|r| {
                a.content_valid
                    && r.content_generation == a.content_generation
                    && r.plan_sha256 == a.plan_sha256
                    && r.sentence_count == a.sentence_count
            })
    }
    pub fn queue(
        &mut self,
        task: QuestionTask,
        a: &Authority<'_>,
        now: Instant,
    ) -> Result<(), Error> {
        if !v561_live_validation::question_task(&task) {
            return Err(Error::Invalid);
        }
        if task.task_id <= self.last_task {
            return Err(Error::Stale);
        }
        if self.active.is_some() {
            return Err(Error::Busy);
        }
        if !Self::authority(&task, a) {
            return Err(Error::Revoked);
        }
        let deadline = now
            .checked_add(Duration::from_millis(u64::from(task.budget_ms)))
            .ok_or(Error::Invalid)?;
        self.last_task = task.task_id;
        self.active = Some(Pending {
            task,
            deadline,
            phase: Phase::Queued,
        });
        Ok(())
    }
    fn checked(
        &mut self,
        task_id: u64,
        generation: u64,
        a: &Authority<'_>,
        now: Instant,
    ) -> Result<&mut Pending, Error> {
        let p = self.active.as_ref().ok_or(Error::Stale)?;
        // A late result cannot clear a newer active question.
        if p.task.task_id != task_id || p.task.generation != generation {
            return Err(Error::Stale);
        }
        if !Self::authority(&p.task, a) {
            self.cancel();
            return Err(Error::Revoked);
        }
        if now >= p.deadline {
            self.cancel();
            return Err(Error::Deadline);
        }
        self.active.as_mut().ok_or(Error::Stale)
    }
    pub fn generate(
        &mut self,
        task_id: u64,
        generation: u64,
        a: &Authority<'_>,
        now: Instant,
    ) -> Result<QuestionTask, Error> {
        let p = self.checked(task_id, generation, a, now)?;
        if p.phase != Phase::Queued {
            return Err(Error::WrongPhase);
        }
        p.phase = Phase::Generating;
        let mut task = p.task.clone();
        // Remaining time, never a new full budget for a model child.
        task.budget_ms = p
            .deadline
            .saturating_duration_since(now)
            .as_millis()
            .min(u128::from(task.budget_ms)) as u32;
        if task.budget_ms == 0 {
            self.cancel();
            return Err(Error::Deadline);
        }
        Ok(task)
    }
    pub fn answer(
        &mut self,
        result: QuestionResult,
        a: &Authority<'_>,
        now: Instant,
    ) -> Result<String, Error> {
        let p = self.checked(result.task_id, result.generation, a, now)?;
        if p.phase != Phase::Generating {
            return Err(Error::WrongPhase);
        }
        if !v561_live_validation::question_result(&result) {
            self.cancel();
            return Err(Error::Invalid);
        }
        p.phase = Phase::Speaking;
        Ok(result.answer)
    }
    pub fn answer_completed(
        &mut self,
        task_id: u64,
        generation: u64,
        a: &Authority<'_>,
        now: Instant,
    ) -> Result<Option<ResumeIntent>, Error> {
        let p = self.checked(task_id, generation, a, now)?;
        if p.phase != Phase::Speaking {
            return Err(Error::WrongPhase);
        }
        let resume = p.task.resume.clone();
        if resume.is_some() {
            p.phase = Phase::Resuming;
        } else {
            self.cancel();
        }
        Ok(resume)
    }
    pub fn resumed(
        &mut self,
        task_id: u64,
        generation: u64,
        a: &Authority<'_>,
        now: Instant,
    ) -> Result<(), Error> {
        let p = self.checked(task_id, generation, a, now)?;
        if p.phase != Phase::Resuming {
            return Err(Error::WrongPhase);
        }
        self.cancel();
        Ok(())
    }
}

#[cfg(test)]
mod tests;

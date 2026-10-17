//! B07: the selling loop's **executor** (V3.11 §53 / §5.7), the layer the kernel judge does not
//! provide.
//!
//! W68 delivered a judge that answers one question about one *plan*: 「这份编排计划合规吗」. That
//! is a verdict on a snapshot, and §53's four rows are 确定性脚本与规则 — but a verdict on a
//! snapshot is not a running loop. A loop advances **one entry at a time**, and at each step it
//! has to answer 「现在能进这一段吗」 against the history that has actually happened. This module
//! is that: it holds the session's plan, its timeline and its checkpoint, and it refuses by name
//! at the exact step where a rule breaks.
//!
//! Three things are deliberately **not** claimed here, because claiming them would be a false
//! statement about what runs:
//!
//! * 段落内容与话术 — content authoring;
//! * TTS 时长与播放采样 — the render lane's;
//! * 「恢复前重新校验产品、价格、活动、素材与授权」 — §5.7 assigns it to the guard and knowledge
//!   layers. This executor **requires** that re-check as input ([`ResumeRecheck`]) and refuses
//!   the resume when it is not fresh; it never asserts the re-check itself. `Unavailable` is a
//!   refusal, not a pass — the producer does not exist in this repository yet, and writing
//!   "assume fresh" here would turn an unmeasured fact into an authorisation.
//!
//! The rules are not re-implemented: cooldown, switching and monotonicity come from
//! [`evaluate_transition`], the same function the batch judge replays its timeline through, so the
//! live loop and the audit cannot disagree about which rule refused a step.

use crate::{RuntimeError, err};
use nai_contracts::v1::ErrorCode;
use nai_kernel::evaluate::selling::{
    ActionCue, Checkpoint, Entrance, FactRef, PitchHistory, PitchPlan, PitchPolicy, PitchRejection,
    SEGMENT_ORDER, SegmentKind, evaluate_pitch, evaluate_transition,
};

// ---------------------------------------------------------------------------
// §5.7: the five things a resume has to re-check
// ---------------------------------------------------------------------------

/// §5.7 「恢复前重新校验**产品、价格、活动、素材与授权**」— five faces, closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecheckFace {
    Product,
    Price,
    Campaign,
    Material,
    Authorisation,
}

/// All five faces, in the baseline's order. A test transcribes it so that quietly dropping one is
/// not survivable: a resume that stopped re-checking 授权 would still look like "all faces fresh".
pub const RECHECK_FACES: [RecheckFace; 5] = [
    RecheckFace::Product,
    RecheckFace::Price,
    RecheckFace::Campaign,
    RecheckFace::Material,
    RecheckFace::Authorisation,
];

impl RecheckFace {
    /// Stable label, also the name a refusal carries.
    pub fn label(self) -> &'static str {
        match self {
            RecheckFace::Product => "product",
            RecheckFace::Price => "price",
            RecheckFace::Campaign => "campaign",
            RecheckFace::Material => "material",
            RecheckFace::Authorisation => "authorisation",
        }
    }
}

/// What the re-check said about one face. Three values, not two: 「没查到」 and 「查到了但不成立」
/// are different facts and §5.7's gate must not fold them into one "no".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecheckOutcome {
    /// The face was re-checked and still holds.
    Fresh,
    /// The face was re-checked and no longer holds.
    Stale,
    /// Nobody could re-check it. **Not** a pass: an unmeasured fact is not an authorisation.
    Unavailable,
}

/// The guard/knowledge layers' answer for one resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRecheck {
    outcomes: [RecheckOutcome; 5],
}

impl ResumeRecheck {
    /// Every face fresh — the only value that permits a resume.
    pub fn fresh() -> Self {
        Self {
            outcomes: [RecheckOutcome::Fresh; 5],
        }
    }

    /// Every face unavailable: what a caller with no producer can honestly say.
    ///
    /// This exists so "we do not have the re-check" has a *representable* value. Without it a
    /// caller would have to choose between lying (`fresh()`) and not calling at all.
    pub fn unavailable() -> Self {
        Self {
            outcomes: [RecheckOutcome::Unavailable; 5],
        }
    }

    pub fn with(mut self, face: RecheckFace, outcome: RecheckOutcome) -> Self {
        self.outcomes[face as usize] = outcome;
        self
    }

    pub fn outcome(&self, face: RecheckFace) -> RecheckOutcome {
        self.outcomes[face as usize]
    }

    /// The faces that are not fresh, by name — in the baseline's order.
    fn not_fresh(&self, wanted: RecheckOutcome) -> Vec<&'static str> {
        RECHECK_FACES
            .iter()
            .filter(|face| self.outcome(**face) == wanted)
            .map(|face| face.label())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

/// Why a step of the selling loop did not happen.
///
/// Each variant carries the kernel's own rejection names ([`PitchRejection::label`]) or the face
/// names, so a decision log records *which rule* refused a step instead of "the step failed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PitchRefusal {
    /// §53 rows 1/2/4: the plan was refused **before** the loop started.
    Plan(Vec<PitchRejection>),
    /// §53 row 3: this transition was refused. The timeline is unchanged.
    Transition(Vec<PitchRejection>),
    /// §5.7: the checkpoint carries text nobody heard.
    CheckpointHoldsUnspokenText,
    /// §5.7: the resume would not land on the checkpoint's segment.
    ResumeDivergesFromCheckpoint,
    /// §5.7: these faces were re-checked and no longer hold.
    RecheckStale(Vec<&'static str>),
    /// §5.7: these faces could not be re-checked. Refused, not assumed.
    RecheckUnavailable(Vec<&'static str>),
    /// A resume with no checkpoint behind it.
    NoCheckpoint,
}

impl PitchRefusal {
    /// Stable label for the decision log.
    pub fn label(&self) -> &'static str {
        match self {
            PitchRefusal::Plan(_) => "pitch.plan_refused",
            PitchRefusal::Transition(_) => "pitch.transition_refused",
            PitchRefusal::CheckpointHoldsUnspokenText => "pitch.checkpoint_holds_unspoken_text",
            PitchRefusal::ResumeDivergesFromCheckpoint => "pitch.resume_diverges_from_checkpoint",
            PitchRefusal::RecheckStale(_) => "pitch.recheck_stale",
            PitchRefusal::RecheckUnavailable(_) => "pitch.recheck_unavailable",
            PitchRefusal::NoCheckpoint => "pitch.no_checkpoint",
        }
    }

    /// The contract code a caller maps this refusal to. Distinct families get distinct codes, so
    /// "too soon" and "we could not re-check" are not the same answer.
    pub fn code(&self) -> ErrorCode {
        match self {
            PitchRefusal::Plan(_) | PitchRefusal::NoCheckpoint => ErrorCode::InvalidArgument,
            PitchRefusal::Transition(_) => ErrorCode::Conflict,
            PitchRefusal::CheckpointHoldsUnspokenText
            | PitchRefusal::ResumeDivergesFromCheckpoint => ErrorCode::InvalidArgument,
            PitchRefusal::RecheckStale(_) => ErrorCode::EvidenceExpired,
            PitchRefusal::RecheckUnavailable(_) => ErrorCode::CapabilityNotReady,
        }
    }

    /// Human-readable reason, naming the rules and the faces. Never empty: a refusal that cannot
    /// say why is indistinguishable from a silent skip.
    pub fn reason(&self) -> String {
        let names = |rejections: &[PitchRejection]| {
            rejections
                .iter()
                .map(|rejection| rejection.label())
                .collect::<Vec<_>>()
                .join(",")
        };
        match self {
            PitchRefusal::Plan(rejections) => {
                format!("the selling plan was refused: {}", names(rejections))
            }
            PitchRefusal::Transition(rejections) => format!(
                "this segment entry was refused and the loop did not advance: {}",
                names(rejections)
            ),
            PitchRefusal::CheckpointHoldsUnspokenText => {
                "the checkpoint carries generated text that was never played (§5.7)".to_string()
            }
            PitchRefusal::ResumeDivergesFromCheckpoint => {
                "the resume does not land on the checkpoint's segment (§5.7)".to_string()
            }
            PitchRefusal::RecheckStale(faces) => format!(
                "the resume re-check found these faces no longer hold (§5.7): {}",
                faces.join(",")
            ),
            PitchRefusal::RecheckUnavailable(faces) => format!(
                "the resume re-check could not be performed for these faces (§5.7): {} — an \
                 unmeasured face is not an authorisation",
                faces.join(",")
            ),
            PitchRefusal::NoCheckpoint => {
                "a resume was asked for but no checkpoint was taken".to_string()
            }
        }
    }
}

impl From<PitchRefusal> for RuntimeError {
    fn from(refusal: PitchRefusal) -> Self {
        err(
            refusal.code(),
            format!("{}: {}", refusal.label(), refusal.reason()),
        )
    }
}

// ---------------------------------------------------------------------------
// The plan the executor is started with
// ---------------------------------------------------------------------------

/// One composition plan plus the thresholds it is judged against.
///
/// `segments` stays text on purpose, exactly as in the kernel: it is the only shape in which
/// "an undefined segment name appears" is expressible.
pub struct PitchInput<'a> {
    pub segments: &'a [&'a str],
    pub checkpoint: Option<&'a Checkpoint>,
    pub resume: Option<SegmentKind>,
    pub entrances: &'a [Entrance],
    pub cues: &'a [ActionCue],
    pub rule_version: &'static str,
    pub cooldown_ms: u32,
    pub min_switch_ms: u32,
    pub current_facts: &'a [FactRef],
}

/// A running selling loop.
pub struct PitchRunner {
    rule_version: &'static str,
    cooldown_ms: u32,
    min_switch_ms: u32,
    current_facts: Vec<FactRef>,
    cues: Vec<ActionCue>,
    history: PitchHistory,
    /// Index into [`SEGMENT_ORDER`]: the segment the loop enters next.
    cursor: usize,
    checkpoint: Option<Checkpoint>,
}

impl PitchRunner {
    /// Start the loop: the whole plan is judged **first**, so a plan that could never run does
    /// not run half way.
    ///
    /// The four rows are judged in full (the kernel does not short-circuit them), and the loop
    /// only exists if nothing was rejected.
    pub fn start(input: PitchInput<'_>) -> Result<Self, PitchRefusal> {
        let plan = PitchPlan {
            segments: input.segments,
            checkpoint: input.checkpoint,
            resume: input.resume,
            entrances: input.entrances,
            cues: input.cues,
        };
        let policy = PitchPolicy {
            rule_version: input.rule_version,
            cooldown_ms: input.cooldown_ms,
            min_switch_ms: input.min_switch_ms,
            current_facts: input.current_facts,
        };
        let verdict = evaluate_pitch(&plan, &policy);
        if !verdict.allowed {
            return Err(PitchRefusal::Plan(verdict.rejections));
        }
        Ok(Self {
            rule_version: input.rule_version,
            cooldown_ms: input.cooldown_ms,
            min_switch_ms: input.min_switch_ms,
            current_facts: input.current_facts.to_vec(),
            cues: input.cues.to_vec(),
            history: PitchHistory::from_entrances(input.entrances),
            // §53's plan order *is* `SEGMENT_ORDER` (anything else was refused above), so the
            // cursor starts after the entries the timeline already carries.
            cursor: input.entrances.len() % SEGMENT_ORDER.len(),
            checkpoint: None,
        })
    }

    fn policy(&self) -> PitchPolicy<'_> {
        PitchPolicy {
            rule_version: self.rule_version,
            cooldown_ms: self.cooldown_ms,
            min_switch_ms: self.min_switch_ms,
            current_facts: &self.current_facts,
        }
    }

    /// The segment the loop would enter next.
    pub fn next_segment(&self) -> SegmentKind {
        SEGMENT_ORDER[self.cursor]
    }

    /// `(entered, per_cycle)` — how many entries the timeline carries.
    pub fn progress(&self) -> (usize, usize) {
        (self.history.count(), SEGMENT_ORDER.len())
    }

    /// When this segment was last entered, if ever.
    pub fn last_entry_of(&self, segment: SegmentKind) -> Option<u32> {
        self.history.last_entry_of(segment)
    }

    /// The action cues the plan carries, so a caller speaks from the same list the plan was
    /// judged with rather than from a second copy.
    pub fn cues(&self) -> &[ActionCue] {
        &self.cues
    }

    /// Enter the next segment.
    ///
    /// **Fail closed**: a refused entry leaves the timeline and the cursor exactly as they were,
    /// so a caller that ignores the error cannot end up ahead of the rules. The rules themselves
    /// come from [`evaluate_transition`] — the same function the batch judge uses.
    pub fn enter(&mut self, at_ms: u32) -> Result<Entrance, PitchRefusal> {
        let candidate = Entrance {
            segment: SEGMENT_ORDER[self.cursor],
            at_ms,
        };
        let refusals = {
            let policy = self.policy();
            evaluate_transition(&self.history, candidate, &policy)
        };
        if !refusals.is_empty() {
            return Err(PitchRefusal::Transition(refusals));
        }
        self.history.record(candidate);
        self.cursor = (self.cursor + 1) % SEGMENT_ORDER.len();
        Ok(candidate)
    }

    /// Take a checkpoint at the current position (§5.7).
    ///
    /// 「检查点保存段落与已完成句，**不保存未播出的生成文本**」. The unspoken-text rule is
    /// enforced here rather than at resume time, because a checkpoint that records text nobody
    /// heard has already violated the rule by the time it is written down.
    pub fn pause(
        &mut self,
        completed_sentence_ids: Vec<String>,
    ) -> Result<Checkpoint, PitchRefusal> {
        // An empty completed-sentence list is **not** a refusal: a checkpoint taken at the very
        // start of a segment legitimately has none yet. It is recorded as an empty list, which is
        // a value, not a gap — the unspoken-text rule is about `carried_text`, not about this.
        let checkpoint = Checkpoint {
            segment: self.current_or_next(),
            completed_sentence_ids,
            carried_text: None,
        };
        self.checkpoint = Some(checkpoint.clone());
        Ok(checkpoint)
    }

    /// The segment playback is inside: the one just entered, or the next one before any entry.
    fn current_or_next(&self) -> SegmentKind {
        let entered = self.history.count();
        if entered == 0 {
            SEGMENT_ORDER[0]
        } else {
            SEGMENT_ORDER[(entered - 1) % SEGMENT_ORDER.len()]
        }
    }

    /// 插问处理完毕，回到检查点所指段落.
    ///
    /// This is **not** a new entrance: 归位 continues the visit that was interrupted, so it is not
    /// judged against the segment cooldown. What it *is* judged against is §5.7's re-check — all
    /// five faces must be fresh. Anything else is refused by name, and the loop does not move.
    pub fn resume(&mut self, recheck: &ResumeRecheck) -> Result<SegmentKind, PitchRefusal> {
        let checkpoint = self
            .checkpoint
            .as_ref()
            .ok_or(PitchRefusal::NoCheckpoint)?
            .clone();
        if checkpoint.carried_text.is_some() {
            return Err(PitchRefusal::CheckpointHoldsUnspokenText);
        }
        if checkpoint.segment != self.current_or_next() {
            return Err(PitchRefusal::ResumeDivergesFromCheckpoint);
        }
        let stale = recheck.not_fresh(RecheckOutcome::Stale);
        if !stale.is_empty() {
            return Err(PitchRefusal::RecheckStale(stale));
        }
        let unavailable = recheck.not_fresh(RecheckOutcome::Unavailable);
        if !unavailable.is_empty() {
            return Err(PitchRefusal::RecheckUnavailable(unavailable));
        }
        // All five faces are fresh: continue on the checkpoint's segment, which is where the loop
        // already is. The cursor is unchanged for the same reason — 归位 is not a new entry.
        Ok(checkpoint.segment)
    }
}

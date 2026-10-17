//! Basic selling loop (V3.9 §53; acceptance B07).
//!
//! §53 moves 基础讲品循环 into R1 and says why in one sentence: R1 can already prove that the
//! system *answers one question safely*; it cannot yet prove that it *keeps describing one
//! product clearly*. §53 also fixes the shape of the gap — the four rows are 「确定性脚本与规则，
//! 不依赖工具规划」, so they are judgeable, and they need no capability pack (INV-33 holds: every
//! judgement below reads local data only).
//!
//! The four rows have four different subjects and stay separate, because folding them together
//! makes "which rule refused this" undecidable again — the same reason W66 did not reuse
//! [`crate::bridge::Degradation`] and W67 did not reuse [`crate::session::Session::change_mode`]:
//!
//! | §53 row | subject | judged by |
//! | --- | --- | --- |
//! | 段落编排 | a whole sequence | [`SEGMENT_ORDER`] + [`SegmentKind::label`] |
//! | 插问后归位 | one checkpoint, one resume | [`Checkpoint`] |
//! | 冷却与切换 | a series of entrance times | [`evaluate_pitch`] over [`Entrance`] |
//! | 事实约束的行动提示 | the claims inside one cue | [`Claim`] against [`PitchPolicy::current_facts`] |
//!
//! **Not to be confused with [`crate::evaluate::show`]**, which judges *one* `ShowPlan` on its
//! own — scene/version/segment id, duration, ref caps, interrupt policy, checkpoint *existence*,
//! evidence binding for sensitive scenes. This module judges the rules *across* segments. Nor
//! with the scheduler's use of the word 冷却: that one is per-viewer (dominance caps, §5.3),
//! this one is per-segment.
//!
//! **Scope boundary.** Thresholds live in [`PitchPolicy`] and are deliberately *not* built in
//! here: §48.7 lists them as pending a replay measurement and forbids external observed values,
//! and §5.3's numbers are explicitly 「场景策略初值，不是固定标准」. The judgement is only
//! "does the plan hold, given these thresholds". Producing segment text, scripts and TTS timing
//! is content authoring and runtime work, and re-validating product/price/campaign/material/
//! authorisation before a resume (§5.7) is the guard and knowledge layers' job — neither is
//! claimed here.

// ---------------------------------------------------------------------------
// §53 row 1 / §5.7: the fixed segments
// ---------------------------------------------------------------------------

/// §53: the four selling segments, in the one order the baseline fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SegmentKind {
    /// 卖点.
    Selling,
    /// 规格.
    Spec,
    /// 适用场景.
    Scenario,
    /// 售后说明.
    AfterSales,
}

/// §53: 「卖点 / 规格 / 适用场景 / 售后说明的固定段落与**顺序**」. The array order *is* the
/// order; a test transcribes it item by item so that quietly dropping one is not survivable.
pub const SEGMENT_ORDER: [SegmentKind; 4] = [
    SegmentKind::Selling,
    SegmentKind::Spec,
    SegmentKind::Scenario,
    SegmentKind::AfterSales,
];

impl SegmentKind {
    /// Position in [`SEGMENT_ORDER`].
    pub fn index(self) -> usize {
        match self {
            SegmentKind::Selling => 0,
            SegmentKind::Spec => 1,
            SegmentKind::Scenario => 2,
            SegmentKind::AfterSales => 3,
        }
    }

    /// Stable decision-log label, also the name a composition plan carries.
    pub fn label(self) -> &'static str {
        match self {
            SegmentKind::Selling => "selling",
            SegmentKind::Spec => "spec",
            SegmentKind::Scenario => "scenario",
            SegmentKind::AfterSales => "after_sales",
        }
    }

    /// Parse a segment name. `None` for anything the baseline does not define — an unknown name
    /// is never silently treated as a known segment.
    pub fn parse(name: &str) -> Option<Self> {
        SEGMENT_ORDER
            .iter()
            .copied()
            .find(|segment| segment.label() == name)
    }
}

// ---------------------------------------------------------------------------
// §53 row 4 / §45.3: the closed set of constrained claims
// ---------------------------------------------------------------------------

/// §45.3: the 禁用域 — 「价格、库存、优惠与活动条件、承诺与售后政策、以及任何个体化承诺」.
/// The set is closed: a claim that is not one of these is not constrained by §53 row 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClaimKind {
    /// 价格.
    Price,
    /// 库存.
    Stock,
    /// 优惠与活动条件.
    Promotion,
    /// 承诺与售后政策.
    Commitment,
    /// 任何个体化承诺.
    PersonalCommitment,
}

/// §45.3's five categories, transcribed. A test pins all five by name.
pub const CLAIM_KINDS: [ClaimKind; 5] = [
    ClaimKind::Price,
    ClaimKind::Stock,
    ClaimKind::Promotion,
    ClaimKind::Commitment,
    ClaimKind::PersonalCommitment,
];

impl ClaimKind {
    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            ClaimKind::Price => "price",
            ClaimKind::Stock => "stock",
            ClaimKind::Promotion => "promotion",
            ClaimKind::Commitment => "commitment",
            ClaimKind::PersonalCommitment => "personal_commitment",
        }
    }
}

// ---------------------------------------------------------------------------
// §5.7: the checkpoint
// ---------------------------------------------------------------------------

/// §5.7: 「检查点保存段落与已完成句，**不保存未播出的生成文本**」.
///
/// `carried_text` exists so that "this checkpoint carries generated text nobody heard" is a
/// *constructible* input. Without the field the violation could not be expressed at all, and
/// the guard would be vacuously true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// 段落 — where playback resumes.
    pub segment: SegmentKind,
    /// 已完成句, by id.
    pub completed_sentence_ids: Vec<String>,
    /// Unplayed generated text. §5.7 forbids it: this MUST be `None`.
    pub carried_text: Option<String>,
}

// ---------------------------------------------------------------------------
// §53 row 3 / §5.3: cooldown and switching
// ---------------------------------------------------------------------------

/// One entry into a segment, at a session-relative time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entrance {
    pub segment: SegmentKind,
    pub at_ms: u32,
}

/// The state one transition decision needs: the last entrance overall, and the last entrance
/// **of each segment**.
///
/// Row 3 has two rules with two different subjects — 「同一段落的**重复进入**不早于段落级冷却」
/// and 「任意**相邻**两次进入不早于最小切换间隔」. The first is about one segment's own
/// history, so it cannot be answered from the adjacent pair alone; a live loop that advances one
/// entry at a time and a batch plan that carries its whole timeline MUST therefore agree on a
/// single state shape, or the same timeline would be judged differently by the two callers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PitchHistory {
    last: Option<Entrance>,
    last_by_segment: [Option<u32>; 4],
    count: usize,
}

impl PitchHistory {
    /// Replay a whole timeline into the state, in order.
    pub fn from_entrances(entrances: &[Entrance]) -> Self {
        let mut history = Self::default();
        for entrance in entrances {
            history.record(*entrance);
        }
        history
    }

    /// How many entrances the timeline carries — the loop's progress, without a second counter
    /// that could drift from the state it describes.
    pub fn count(&self) -> usize {
        self.count
    }

    /// The most recent entrance, whatever the segment.
    pub fn last(&self) -> Option<Entrance> {
        self.last
    }

    /// When this segment was last entered, if ever.
    pub fn last_entry_of(&self, segment: SegmentKind) -> Option<u32> {
        self.last_by_segment[segment.index()]
    }

    /// Record an entrance. Recording is unconditional: the history is what *happened*, not what
    /// was allowed, so a violation is not allowed to erase its own evidence.
    pub fn record(&mut self, entrance: Entrance) {
        self.count += 1;
        // Keep the latest only; a caller that replays out of order gets the same state a
        // monotonic replay would produce for the maximum, which is why the monotonic rule stays
        // a separate refusal rather than being folded into this bookkeeping.
        if self.last.is_none_or(|last| entrance.at_ms >= last.at_ms) {
            self.last = Some(entrance);
        }
        let slot = &mut self.last_by_segment[entrance.segment.index()];
        if slot.is_none_or(|at| entrance.at_ms >= at) {
            *slot = Some(entrance.at_ms);
        }
    }
}

/// §53 row 3, judged for **one** transition.
///
/// The batch judge ([`evaluate_pitch`]) and a live loop both call this, so「哪条规则被违反」有
/// 唯一定义。Cooldown reads the segment's own last entry; switching reads the previous entry
/// overall. Both are statements about an interval, and an interval is undefined on a decreasing
/// series — so a decrease is refused **by name** and the interval rules are not evaluated at all.
pub fn evaluate_transition(
    history: &PitchHistory,
    next: Entrance,
    policy: &PitchPolicy<'_>,
) -> Vec<PitchRejection> {
    let mut rejections = Vec::new();
    let monotonic = history.last().is_none_or(|last| next.at_ms >= last.at_ms);
    if !monotonic {
        rejections.push(PitchRejection::EntranceTimesDecrease);
        return rejections;
    }
    if let Some(previous) = history.last_entry_of(next.segment)
        && next.at_ms - previous < policy.cooldown_ms
    {
        rejections.push(PitchRejection::CooldownNotElapsed);
    }
    if let Some(last) = history.last()
        && next.at_ms - last.at_ms < policy.min_switch_ms
    {
        rejections.push(PitchRejection::SwitchTooSoon);
    }
    rejections
}

// ---------------------------------------------------------------------------
// §53 row 4 / §5.4: fact-bound action cues
// ---------------------------------------------------------------------------

/// A revision of a fact, as §5.4's evidence stage and §45.4's invalidation rule name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactRef {
    pub object_id: String,
    pub revision: String,
}

/// One constrained assertion inside an action cue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub kind: ClaimKind,
    /// The fact this claim stands on. `None` is the "it sounds right" case §45.2 refuses.
    pub fact_ref: Option<FactRef>,
}

/// §53 row 4: 事实约束的行动提示 —— 只基于**有效事实版本**；禁虚构优惠与承诺.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionCue {
    pub cue_id: String,
    pub claims: Vec<Claim>,
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// Thresholds and the facts currently in force.
///
/// The two time thresholds are separate fields because §53 names two separate rules: a segment
/// may not come back before its own cooldown, and any two adjacent entries may not be closer
/// than the switch interval. Collapsing them would make "which rule was broken" undecidable.
pub struct PitchPolicy<'a> {
    pub rule_version: &'static str,
    /// 段落级冷却.
    pub cooldown_ms: u32,
    /// 最小切换间隔.
    pub min_switch_ms: u32,
    /// The revisions in force now (§5.4 evidence stage, §45.4 invalidation).
    pub current_facts: &'a [FactRef],
}

/// One composition plan, as read from data.
///
/// `segments` is text on purpose: it is the only input shape in which "an undefined segment name
/// appears" is expressible. A `[SegmentKind]` plan would make [`PitchRejection::UnknownSegment`]
/// unreachable — a guard that cannot fire.
pub struct PitchPlan<'a> {
    pub segments: &'a [&'a str],
    pub checkpoint: Option<&'a Checkpoint>,
    /// Where an interjection returns to. Judged only when a checkpoint is present.
    pub resume: Option<SegmentKind>,
    pub entrances: &'a [Entrance],
    pub cues: &'a [ActionCue],
}

// ---------------------------------------------------------------------------
// §53: every named refusal
// ---------------------------------------------------------------------------

/// One reason a composition plan must not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PitchRejection {
    /// A segment name the baseline does not define.
    UnknownSegment,
    /// A segment of the fixed four is absent.
    SegmentMissing,
    /// All four are present but not in the fixed order.
    SegmentOutOfOrder,
    /// §5.7: the checkpoint carries unplayed generated text.
    CheckpointHoldsUnspokenText,
    /// The resume does not land on the checkpoint's segment.
    ResumeDivergesFromCheckpoint,
    /// Entrance times are not non-decreasing.
    EntranceTimesDecrease,
    /// §53: the same segment came back before its cooldown.
    CooldownNotElapsed,
    /// §53: two adjacent entries are closer than the minimum switch interval.
    SwitchTooSoon,
    /// §53 row 4: a constrained claim binds no fact at all.
    ActionCueWithoutFactBinding,
    /// §53 row 4: a constrained claim binds a revision that is not the one in force.
    ActionCueFactStale,
}

impl PitchRejection {
    /// All ten refusal names. A test asserts the constructed set is exactly this.
    pub const ALL: [PitchRejection; 10] = [
        PitchRejection::UnknownSegment,
        PitchRejection::SegmentMissing,
        PitchRejection::SegmentOutOfOrder,
        PitchRejection::CheckpointHoldsUnspokenText,
        PitchRejection::ResumeDivergesFromCheckpoint,
        PitchRejection::EntranceTimesDecrease,
        PitchRejection::CooldownNotElapsed,
        PitchRejection::SwitchTooSoon,
        PitchRejection::ActionCueWithoutFactBinding,
        PitchRejection::ActionCueFactStale,
    ];

    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            PitchRejection::UnknownSegment => "pitch.unknown_segment",
            PitchRejection::SegmentMissing => "pitch.segment_missing",
            PitchRejection::SegmentOutOfOrder => "pitch.segment_out_of_order",
            PitchRejection::CheckpointHoldsUnspokenText => "pitch.checkpoint_holds_unspoken_text",
            PitchRejection::ResumeDivergesFromCheckpoint => "pitch.resume_diverges_from_checkpoint",
            PitchRejection::EntranceTimesDecrease => "pitch.entrance_times_decrease",
            PitchRejection::CooldownNotElapsed => "pitch.cooldown_not_elapsed",
            PitchRejection::SwitchTooSoon => "pitch.switch_too_soon",
            PitchRejection::ActionCueWithoutFactBinding => "pitch.action_cue_without_fact_binding",
            PitchRejection::ActionCueFactStale => "pitch.action_cue_fact_stale",
        }
    }
}

/// Outcome of judging one composition plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PitchVerdict {
    /// True only when nothing was rejected.
    pub allowed: bool,
    pub rejections: Vec<PitchRejection>,
    /// The absent segments, **by name** — "count mismatch" is not an answer.
    pub missing_segments: Vec<&'static str>,
    /// The undefined names seen, in order.
    pub unknown_segments: Vec<String>,
    /// Segments whose repeat came back before the cooldown.
    pub cooldown_violations: Vec<&'static str>,
    /// `(cue_id, kind)` for every claim with no fact behind it.
    pub unbound_claims: Vec<(String, ClaimKind)>,
    /// `(cue_id, kind)` for every claim standing on a revision that is no longer in force.
    pub stale_claims: Vec<(String, ClaimKind)>,
}

/// §53: judge one composition plan.
///
/// All four rows are judged and every violation is named; the rows do not short-circuit each
/// other, so "缺段" and "乱序" can both be reported for one plan.
///
/// The cooldown and switch rules are gated on their precondition: both are statements about an
/// *interval*, and an interval is undefined on a decreasing series. So a decreasing series is
/// refused by name ([`PitchRejection::EntranceTimesDecrease`]) and the interval rules are
/// evaluated only when the series really is non-decreasing. The precondition failing is never
/// read as a pass.
pub fn evaluate_pitch(plan: &PitchPlan<'_>, policy: &PitchPolicy<'_>) -> PitchVerdict {
    let mut rejections = Vec::new();
    let mut missing_segments = Vec::new();
    let mut unknown_segments = Vec::new();
    let mut cooldown_violations = Vec::new();
    let mut unbound_claims = Vec::new();
    let mut stale_claims = Vec::new();

    // ---- §53 row 1: composition ----
    let mut parsed = Vec::with_capacity(plan.segments.len());
    for name in plan.segments {
        match SegmentKind::parse(name) {
            Some(segment) => parsed.push(segment),
            None => unknown_segments.push((*name).to_string()),
        }
    }
    if !unknown_segments.is_empty() {
        rejections.push(PitchRejection::UnknownSegment);
    }
    for segment in SEGMENT_ORDER {
        if !parsed.contains(&segment) {
            missing_segments.push(segment.label());
        }
    }
    if !missing_segments.is_empty() {
        rejections.push(PitchRejection::SegmentMissing);
    }
    // Order is judged on the segments that *are* known, so a plan that is both short and wrong
    // reports both — never just the first one hit.
    let known: Vec<SegmentKind> = parsed.clone();
    let mut sorted = known.clone();
    sorted.sort();
    if known != sorted {
        rejections.push(PitchRejection::SegmentOutOfOrder);
    }

    // ---- §5.7: the checkpoint and the resume ----
    if let Some(checkpoint) = plan.checkpoint {
        if checkpoint.carried_text.is_some() {
            rejections.push(PitchRejection::CheckpointHoldsUnspokenText);
        }
        if plan.resume != Some(checkpoint.segment) {
            rejections.push(PitchRejection::ResumeDivergesFromCheckpoint);
        }
    }

    // ---- §53 row 3: cooldown and switching ----
    //
    // Replayed through [`evaluate_transition`] one entrance at a time, so the batch verdict and
    // a live loop cannot disagree about a transition. Cooldown is the segment's own last entry
    // (§53: 「同一段落的**重复进入**」), which a checker that looks only at adjacent pairs
    // misses: `selling@0 → spec@10 → selling@20` has no adjacent same-segment pair at all.
    let mut history = PitchHistory::default();
    for entrance in plan.entrances {
        for rejection in evaluate_transition(&history, *entrance, policy) {
            match rejection {
                PitchRejection::CooldownNotElapsed => {
                    if !cooldown_violations.contains(&entrance.segment.label()) {
                        cooldown_violations.push(entrance.segment.label());
                    }
                }
                other => {
                    if !rejections.contains(&other) {
                        rejections.push(other);
                    }
                }
            }
        }
        history.record(*entrance);
    }
    if !cooldown_violations.is_empty() {
        rejections.push(PitchRejection::CooldownNotElapsed);
    }

    // ---- §53 row 4: fact-bound action cues ----
    for cue in plan.cues {
        for claim in &cue.claims {
            match &claim.fact_ref {
                None => unbound_claims.push((cue.cue_id.clone(), claim.kind)),
                Some(fact) => {
                    if !policy.current_facts.iter().any(|current| current == fact) {
                        stale_claims.push((cue.cue_id.clone(), claim.kind));
                    }
                }
            }
        }
    }
    if !unbound_claims.is_empty() {
        rejections.push(PitchRejection::ActionCueWithoutFactBinding);
    }
    if !stale_claims.is_empty() {
        rejections.push(PitchRejection::ActionCueFactStale);
    }

    PitchVerdict {
        allowed: rejections.is_empty(),
        rejections,
        missing_segments,
        unknown_segments,
        cooldown_violations,
        unbound_claims,
        stale_claims,
    }
}

//! Context assembly (V3.9 §41; acceptance H01/H02).
//!
//! §41 is the L1 information boundary: what the model is shown, in what order, under what
//! per-segment budget, and what every fragment must carry. §41.5 fixes the acceptance as
//! **H01/H02**, and §51.3 keeps both inside R1 as *basic discipline* rather than an optional
//! subsystem — without them the default combination cannot call a model correctly at all.
//!
//! Until this module that requirement had a name, a registry entry and no implementation:
//! `harness.rs` (W66, H15/H17) mentions `Assembly` eleven times, but every one of those is a
//! **layer or field name** — `ObservationLayer::Assembly`, `FailureLayer::Assembly`, the
//! required fields `segment_budget` / `cropped_items` / `crop_reason`. None of them decides
//! whether an assembly is *legal*. This module decides exactly that.
//!
//! Two halves, each with its own flat rejection enum so that "which rule refused this" stays
//! decidable:
//!
//! * **H01** ([`evaluate_assembly`]): the nine segments in §41.1's fixed order, the three
//!   uncroppable ones, cropping that may only come from the back, per-segment budgets in three
//!   dimensions, and §41.3's provenance tags.
//! * **H02** ([`rebuild_assembly`]): whether the decision log alone can reproduce the same
//!   fragment sequence — i.e. whether the assembly depended on the live working memory.
//!
//! **Not to be confused with** [`crate::evaluate::harness`], which judges whether a record may
//! enter the log and where a *failure* lands in §12.2; nor with [`crate::evaluate::selling`],
//! whose four segments are §53's selling segments and a different axis. Folding either into
//! this module would make "which rule refused this" undecidable again — the same reason W66 did
//! not reuse [`crate::bridge::Degradation`] and W67 did not reuse `session::change_mode`.
//!
//! **Scope boundary.** Budget **values** are supplied by the caller, never built in here:
//! §41.2 says the initial values are only a starting point to be tuned by §47.2's golden-set
//! replay and recorded in an ADR, and §48.7 forbids external observed values. Feeding the model,
//! appending the record to the hash chain in `nai-storage`, and executing the six §47.2
//! regression classes (capability pack H16) are runtime work and are **not** claimed here.

// ---------------------------------------------------------------------------
// §41.1: the nine segments and their fixed order
// ---------------------------------------------------------------------------

/// §41.1: the nine assembly segments, in the one order the baseline fixes。
///
/// The order exists for one reason, stated in §41.1: **约束不被挤掉** — the constraints must not
/// be crowded out. That is why the first three are uncroppable and why cropping runs backwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Segment {
    /// 1 — 系统策略与安全约束 (§29.1). 不可裁.
    SystemPolicy,
    /// 2 — 人设、场景口吻与禁答范围 (§15). 不可裁.
    Persona,
    /// 3 — 任务与输出契约 (§20.6, §5.1). 不可裁.
    OutputContract,
    /// 4 — 当前事件与 Observation 摘要 (§5.1, §22.2). 可裁至摘要.
    EventSummary,
    /// 5 — 已批准事实与检索证据 (KB0/KB1 §6, §24). 最多 3–5 条（§6 初值）.
    Evidence,
    /// 6 — 记忆片段 (§42). 独立配额.
    Memory,
    /// 7 — 可用技能与工具描述 (§35.5, §46). 最多 5 个.
    Skills,
    /// 8 — 最近实际播完的句子 (§5.5 工作记忆). 可裁.
    RecentUtterances,
    /// 9 — 工具结果与外部返回 (§44, §46). 可裁；一律标不可信.
    ToolResults,
}

/// §41.1's table, transcribed row by row. The array order *is* the assembly order; tests pin
/// every entry by name so that quietly dropping or reordering one is not survivable.
pub const SEGMENT_ORDER: [Segment; 9] = [
    Segment::SystemPolicy,
    Segment::Persona,
    Segment::OutputContract,
    Segment::EventSummary,
    Segment::Evidence,
    Segment::Memory,
    Segment::Skills,
    Segment::RecentUtterances,
    Segment::ToolResults,
];

/// §41.1 marks rows 1–3 as 不可裁. The count is one number, not a second hard-coded list.
pub const UNCROPPABLE_COUNT: usize = 3;

impl Segment {
    /// Position in [`SEGMENT_ORDER`].
    pub fn index(self) -> usize {
        match self {
            Segment::SystemPolicy => 0,
            Segment::Persona => 1,
            Segment::OutputContract => 2,
            Segment::EventSummary => 3,
            Segment::Evidence => 4,
            Segment::Memory => 5,
            Segment::Skills => 6,
            Segment::RecentUtterances => 7,
            Segment::ToolResults => 8,
        }
    }

    /// Stable label, also the name a plan carries.
    pub fn label(self) -> &'static str {
        match self {
            Segment::SystemPolicy => "system_policy",
            Segment::Persona => "persona",
            Segment::OutputContract => "output_contract",
            Segment::EventSummary => "event_summary",
            Segment::Evidence => "evidence",
            Segment::Memory => "memory",
            Segment::Skills => "skills",
            Segment::RecentUtterances => "recent_utterances",
            Segment::ToolResults => "tool_results",
        }
    }

    /// §41.1's 裁剪 column. Rows 1–3 say 不可裁; the rest are croppable in some form.
    pub fn uncroppable(self) -> bool {
        self.index() < UNCROPPABLE_COUNT
    }

    /// Parse a segment name. `None` for anything §41.1 does not define — an unknown name is
    /// never silently treated as a known segment.
    pub fn parse(name: &str) -> Option<Self> {
        SEGMENT_ORDER
            .iter()
            .copied()
            .find(|segment| segment.label() == name)
    }
}

// ---------------------------------------------------------------------------
// §41.3: source kinds and the data/instruction tag
// ---------------------------------------------------------------------------

/// §41.3: 「来源类型（系统策略 / 场景 / 事件 / 知识证据 / 记忆 / 工具结果 / 外部智能体）」.
/// Closed: a seventh-and-a-half kind does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceKind {
    /// 系统策略.
    SystemPolicy,
    /// 场景（含人设）.
    Scenario,
    /// 事件（观众文本属此类）.
    Event,
    /// 知识证据（检索片段属此类）.
    KnowledgeEvidence,
    /// 记忆.
    Memory,
    /// 工具结果（工具返回属此类）.
    ToolResult,
    /// 外部智能体（外部智能体返回属此类）.
    ExternalAgent,
}

/// §41.3's seven source kinds, transcribed. Tests pin all seven by name.
pub const SOURCE_KINDS: [SourceKind; 7] = [
    SourceKind::SystemPolicy,
    SourceKind::Scenario,
    SourceKind::Event,
    SourceKind::KnowledgeEvidence,
    SourceKind::Memory,
    SourceKind::ToolResult,
    SourceKind::ExternalAgent,
];

impl SourceKind {
    /// Stable label.
    pub fn label(self) -> &'static str {
        match self {
            SourceKind::SystemPolicy => "system_policy",
            SourceKind::Scenario => "scenario",
            SourceKind::Event => "event",
            SourceKind::KnowledgeEvidence => "knowledge_evidence",
            SourceKind::Memory => "memory",
            SourceKind::ToolResult => "tool_result",
            SourceKind::ExternalAgent => "external_agent",
        }
    }

    /// §41.3: 「**观众文本、检索片段、工具返回、外部智能体返回**一律标为数据」.
    ///
    /// The four phrases map onto four kinds — 观众文本→[`SourceKind::Event`],
    /// 检索片段→[`SourceKind::KnowledgeEvidence`], 工具返回→[`SourceKind::ToolResult`],
    /// 外部智能体返回→[`SourceKind::ExternalAgent`]. The remaining three kinds carry no such
    /// constraint in §41.3, so they are not judged here (neither allowed nor forbidden).
    pub fn must_be_data(self) -> bool {
        matches!(
            self,
            SourceKind::Event
                | SourceKind::KnowledgeEvidence
                | SourceKind::ToolResult
                | SourceKind::ExternalAgent
        )
    }
}

/// §41.3: the 「数据或指令」 tag. Not an `Option` — a missing tag is unrepresentable, so only a
/// **wrong value** needs a rejection ([`AssemblyRejection::UntrustedMarkedAsInstruction`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// 数据.
    Data,
    /// 指令.
    Instruction,
}

impl Trust {
    /// Stable label.
    pub fn label(self) -> &'static str {
        match self {
            Trust::Data => "data",
            Trust::Instruction => "instruction",
        }
    }
}

// ---------------------------------------------------------------------------
// §41.3: one fragment
// ---------------------------------------------------------------------------

/// §41.3: every fragment entering the context carries 来源类型、来源标识、作用域、时效（如适用）
/// 与数据/指令标注.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment<'a> {
    /// 片段标识.
    pub id: &'a str,
    /// 来源类型.
    pub source: SourceKind,
    /// 来源标识.
    pub source_ref: &'a str,
    /// 作用域. §41.3 lists it without qualification, so it MUST be present.
    pub scope: Option<&'a str>,
    /// 时效. §41.3 writes 「如适用」, so a missing value is **not** a violation and is not judged.
    pub age_ms: Option<u64>,
    /// 数据或指令.
    pub trust: Trust,
}

// ---------------------------------------------------------------------------
// §41.1/§41.2: cropping, usage and per-segment budgets
// ---------------------------------------------------------------------------

/// §41.1/§41.2: a crop MUST record 「被裁了什么与原因」 — both, not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crop<'a> {
    /// Items actually dropped, by id.
    pub dropped: &'a [&'a str],
    /// Why they were dropped.
    pub reason: &'a str,
}

/// §41.2: 「Token 与字符同时记录」 and, from §41.1's 裁剪 column, item counts as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentUsage {
    pub tokens: u64,
    pub chars: u64,
    pub items: usize,
}

/// §41.2: 「预算**按段**分配，不是一个总上限」 plus a fixed floor for the uncroppable segments.
///
/// Every dimension is optional at the type level so that "this dimension was not recorded" is
/// expressible; whether its absence is a violation is decided in [`evaluate_assembly`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentBudget {
    pub tokens: Option<u64>,
    pub chars: Option<u64>,
    /// §41.1's 裁剪 column gives item caps for two segments (证据 3–5 条、技能 5 个). Optional
    /// here because §41.1 caps only those two; the **values** come from the caller (§41.2: the
    /// initial values are a starting point, to be tuned by golden-set replay).
    pub items: Option<usize>,
    /// §41.2: 「策略与人设段取固定下限」. Required on the uncroppable segments.
    pub floor: Option<u64>,
}

/// One segment as it is offered for assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentSlice<'a> {
    /// Deliberately a `&str`, not a [`Segment`]: an undefined segment name must be
    /// *expressible*, otherwise [`AssemblyRejection::UnknownSegment`] could never be
    /// constructed and its guard would be vacuously true.
    pub segment: &'a str,
    pub fragments: &'a [Fragment<'a>],
    pub usage: SegmentUsage,
    /// `None` is the "one total cap stands in for per-segment budgets" case.
    pub budget: Option<SegmentBudget>,
    pub crop: Option<Crop<'a>>,
}

/// §41.1/§41.5: one assembly, offered for judgement.
///
/// The version pair is required because §41.5 lists 「本次调用的模型与提示词版本」 among the
/// things the assembly MUST record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyPlan<'a> {
    pub slices: &'a [SegmentSlice<'a>],
    pub model_version: &'a str,
    pub prompt_version: &'a str,
}

// ---------------------------------------------------------------------------
// H01: named refusals
// ---------------------------------------------------------------------------

/// Every reason an assembly must not be used to call the model. Flat, with [`Self::ALL`] and a
/// stable [`Self::label`] — the house style shared with `MemoryRejection` / `RampRejection` /
/// `PitchRejection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssemblyRejection {
    /// A segment name §41.1 does not define.
    UnknownSegment,
    /// The segments are not in §41.1's fixed order (this includes a repeated segment).
    SegmentOutOfOrder,
    /// One of §41.1's three uncroppable segments is absent.
    UncroppableSegmentMissing,
    /// One of them was cropped.
    UncroppableSegmentCropped,
    /// A crop was recorded without a reason.
    CropWithoutReason,
    /// A crop was recorded without saying what was dropped.
    CropWithoutDroppedItems,
    /// A crop did not fall on a suffix of the croppable segments — i.e. cropping did not run
    /// from the back.
    CropFromFront,
    /// A segment carries no budget of its own (§41.2 requires分配**按段**).
    SegmentBudgetMissing,
    /// Token and character counts were not both recorded (§41.2: two dimensions).
    BudgetDimensionMissing,
    /// An uncroppable segment carries no fixed floor (§41.2).
    UncroppableWithoutFloor,
    /// Usage exceeded one of the segment's own limits; the dimension is named in
    /// [`BudgetFinding`], not in this variant.
    SegmentOverBudget,
    /// A fragment has no source reference (or a blank one).
    FragmentWithoutSourceRef,
    /// A fragment has no scope.
    FragmentWithoutScope,
    /// A fragment whose source kind §41.3 requires to be data was tagged as an instruction.
    UntrustedMarkedAsInstruction,
}

impl AssemblyRejection {
    /// All fourteen names. Tests assert the constructed set is exactly this.
    pub const ALL: [AssemblyRejection; 14] = [
        AssemblyRejection::UnknownSegment,
        AssemblyRejection::SegmentOutOfOrder,
        AssemblyRejection::UncroppableSegmentMissing,
        AssemblyRejection::UncroppableSegmentCropped,
        AssemblyRejection::CropWithoutReason,
        AssemblyRejection::CropWithoutDroppedItems,
        AssemblyRejection::CropFromFront,
        AssemblyRejection::SegmentBudgetMissing,
        AssemblyRejection::BudgetDimensionMissing,
        AssemblyRejection::UncroppableWithoutFloor,
        AssemblyRejection::SegmentOverBudget,
        AssemblyRejection::FragmentWithoutSourceRef,
        AssemblyRejection::FragmentWithoutScope,
        AssemblyRejection::UntrustedMarkedAsInstruction,
    ];

    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            AssemblyRejection::UnknownSegment => "assembly.unknown_segment",
            AssemblyRejection::SegmentOutOfOrder => "assembly.segment_out_of_order",
            AssemblyRejection::UncroppableSegmentMissing => "assembly.uncroppable_segment_missing",
            AssemblyRejection::UncroppableSegmentCropped => "assembly.uncroppable_segment_cropped",
            AssemblyRejection::CropWithoutReason => "assembly.crop_without_reason",
            AssemblyRejection::CropWithoutDroppedItems => "assembly.crop_without_dropped_items",
            AssemblyRejection::CropFromFront => "assembly.crop_from_front",
            AssemblyRejection::SegmentBudgetMissing => "assembly.segment_budget_missing",
            AssemblyRejection::BudgetDimensionMissing => "assembly.budget_dimension_missing",
            AssemblyRejection::UncroppableWithoutFloor => "assembly.uncroppable_without_floor",
            AssemblyRejection::SegmentOverBudget => "assembly.segment_over_budget",
            AssemblyRejection::FragmentWithoutSourceRef => "assembly.fragment_without_source_ref",
            AssemblyRejection::FragmentWithoutScope => "assembly.fragment_without_scope",
            AssemblyRejection::UntrustedMarkedAsInstruction => {
                "assembly.untrusted_marked_as_instruction"
            }
        }
    }
}

/// §41.2: which dimension of a per-segment budget was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDimension {
    /// Token.
    Tokens,
    /// 字符.
    Chars,
    /// 条数 (§41.1's 裁剪 column).
    Items,
}

impl BudgetDimension {
    /// Stable label.
    pub fn label(self) -> &'static str {
        match self {
            BudgetDimension::Tokens => "tokens",
            BudgetDimension::Chars => "chars",
            BudgetDimension::Items => "items",
        }
    }
}

/// One measurement that exceeded its segment's limit. Named per segment **and** per dimension —
/// splitting this into three rejection variants would lose "which segment, which dimension".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetFinding {
    pub segment: Segment,
    pub dimension: BudgetDimension,
    pub used: u64,
    pub limit: u64,
}

/// Outcome of judging one assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyVerdict<'a> {
    /// True only when nothing was refused. §41.1: an assembly missing an uncroppable segment
    /// MUST refuse the call; more generally an invalid assembly MUST NOT be used to call.
    pub callable: bool,
    pub rejections: Vec<AssemblyRejection>,
    /// Uncroppable segments that were absent — **named**, not counted.
    pub missing_segments: Vec<Segment>,
    /// Every segment reported as cropped.
    pub cropped_segments: Vec<Segment>,
    /// Segments offered without a budget of their own.
    pub budget_missing: Vec<Segment>,
    pub over_budget: Vec<BudgetFinding>,
    /// Fragment ids with no (or a blank) source reference.
    pub unprovenanced_fragments: Vec<&'a str>,
    /// Fragment ids with no scope.
    pub unscoped_fragments: Vec<&'a str>,
    /// Fragment ids whose source §41.3 requires to be data but which were tagged as an
    /// instruction.
    pub untrusted_as_instruction: Vec<&'a str>,
}

/// Record a refusal once. Every refusal is recorded at most once, so the verdict's list is a
/// set of reasons rather than a count of how many fragments tripped each one.
fn refuse<T: PartialEq>(list: &mut Vec<T>, rejection: T) {
    if !list.contains(&rejection) {
        list.push(rejection);
    }
}

// ---------------------------------------------------------------------------
// H01: the judgement
// ---------------------------------------------------------------------------

/// §41.1–§41.3: decide whether an assembly may be used, and name every reason it may not.
///
/// The checks run in a fixed order and **none of them short-circuits**: a plan that is both
/// short and out of order reports both, because a reader asking "why was this refused" is not
/// served by the first hit alone. The order below is the reading order of §41.1's table:
/// names → order → uncroppable presence → uncroppable cropping → crop records → crop position →
/// budgets → tags.
pub fn evaluate_assembly<'plan>(plan: &AssemblyPlan<'plan>) -> AssemblyVerdict<'plan> {
    let mut rejections = Vec::new();
    let mut missing_segments = Vec::new();
    let mut cropped_segments = Vec::new();
    let mut budget_missing = Vec::new();
    let mut over_budget = Vec::new();
    let mut unprovenanced_fragments = Vec::new();
    let mut unscoped_fragments = Vec::new();
    let mut untrusted_as_instruction = Vec::new();

    // Known segments, in plan order, each paired with whether it was cropped.
    let mut known: Vec<(Segment, bool)> = Vec::new();
    for slice in plan.slices {
        match Segment::parse(slice.segment) {
            Some(segment) => known.push((segment, slice.crop.is_some())),
            None => refuse(&mut rejections, AssemblyRejection::UnknownSegment),
        }
    }

    // Order: the known segments must be a strictly increasing subsequence of SEGMENT_ORDER.
    // A set comparison (sort, then compare) would let the right segments in the wrong order
    // through, so the comparison is on the sequence itself.
    let sequence: Vec<Segment> = known.iter().map(|(segment, _)| *segment).collect();
    let mut expected = sequence.clone();
    expected.sort();
    expected.dedup();
    if sequence != expected {
        refuse(&mut rejections, AssemblyRejection::SegmentOutOfOrder);
    }

    // §41.1: 不可裁剪段（1–3）缺失时 MUST 拒绝调用。Only these three are required to appear;
    // the other six may be absent (cropped or simply empty).
    for segment in SEGMENT_ORDER.iter().copied() {
        if segment.uncroppable() && !sequence.contains(&segment) {
            missing_segments.push(segment);
            refuse(
                &mut rejections,
                AssemblyRejection::UncroppableSegmentMissing,
            );
        }
    }

    // §41.1: 前段不可裁 — cropping may never reach rows 1–3.
    for (segment, cropped) in &known {
        if *cropped {
            cropped_segments.push(*segment);
            if segment.uncroppable() {
                refuse(
                    &mut rejections,
                    AssemblyRejection::UncroppableSegmentCropped,
                );
            }
        }
    }

    // §41.1: a crop MUST record 被裁了什么与原因 — both halves, separately.
    for slice in plan.slices {
        if let Some(crop) = &slice.crop {
            if crop.reason.trim().is_empty() {
                refuse(&mut rejections, AssemblyRejection::CropWithoutReason);
            }
            if crop.dropped.is_empty() {
                refuse(&mut rejections, AssemblyRejection::CropWithoutDroppedItems);
            }
        }
    }

    // §41.1: 预算不足时从后往前裁剪 ⇒ among the croppable segments, the cropped ones must form
    // a *suffix*. A shape like {evidence cropped, skills and utterances intact} cannot arise
    // from a back-to-front walk, which is why checking "a crop exists and has a reason" is not
    // enough.
    let mut seen_cropped = false;
    for (segment, cropped) in &known {
        if segment.uncroppable() {
            continue;
        }
        if *cropped {
            seen_cropped = true;
        } else if seen_cropped {
            refuse(&mut rejections, AssemblyRejection::CropFromFront);
        }
    }

    // §41.2: budgets are per segment, with floors for the uncroppable ones, and Token/char are
    // two dimensions that never substitute for each other.
    for slice in plan.slices {
        let Some(segment) = Segment::parse(slice.segment) else {
            continue;
        };
        let Some(budget) = &slice.budget else {
            budget_missing.push(segment);
            refuse(&mut rejections, AssemblyRejection::SegmentBudgetMissing);
            continue;
        };
        if budget.tokens.is_none() || budget.chars.is_none() {
            refuse(&mut rejections, AssemblyRejection::BudgetDimensionMissing);
        }
        if segment.uncroppable() && budget.floor.is_none() {
            refuse(&mut rejections, AssemblyRejection::UncroppableWithoutFloor);
        }

        let mut exceeded = false;
        if let Some(limit) = budget.tokens
            && slice.usage.tokens > limit
        {
            over_budget.push(BudgetFinding {
                segment,
                dimension: BudgetDimension::Tokens,
                used: slice.usage.tokens,
                limit,
            });
            exceeded = true;
        }
        if let Some(limit) = budget.chars
            && slice.usage.chars > limit
        {
            over_budget.push(BudgetFinding {
                segment,
                dimension: BudgetDimension::Chars,
                used: slice.usage.chars,
                limit,
            });
            exceeded = true;
        }
        if let Some(limit) = budget.items
            && slice.usage.items > limit
        {
            over_budget.push(BudgetFinding {
                segment,
                dimension: BudgetDimension::Items,
                used: slice.usage.items as u64,
                limit: limit as u64,
            });
            exceeded = true;
        }
        if exceeded {
            refuse(&mut rejections, AssemblyRejection::SegmentOverBudget);
        }
    }

    // §41.3: provenance, scope, and the data/instruction tag.
    for slice in plan.slices {
        for fragment in slice.fragments {
            if fragment.source_ref.trim().is_empty() {
                unprovenanced_fragments.push(fragment.id);
                refuse(&mut rejections, AssemblyRejection::FragmentWithoutSourceRef);
            }
            if fragment.scope.unwrap_or("").trim().is_empty() {
                unscoped_fragments.push(fragment.id);
                refuse(&mut rejections, AssemblyRejection::FragmentWithoutScope);
            }
            if fragment.source.must_be_data() && fragment.trust == Trust::Instruction {
                untrusted_as_instruction.push(fragment.id);
                refuse(
                    &mut rejections,
                    AssemblyRejection::UntrustedMarkedAsInstruction,
                );
            }
        }
    }

    AssemblyVerdict {
        callable: rejections.is_empty(),
        rejections,
        missing_segments,
        cropped_segments,
        budget_missing,
        over_budget,
        unprovenanced_fragments,
        unscoped_fragments,
        untrusted_as_instruction,
    }
}

// ---------------------------------------------------------------------------
// H02: what the decision log carries, and rebuilding from it
// ---------------------------------------------------------------------------

/// §41.5: 「装配 MUST 落决策日志：进入的片段标识、各段预算占用、裁剪项与原因、本次调用的模型与
/// 提示词版本」.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyRecord<'a> {
    /// 进入的片段标识, in the order they entered.
    pub fragment_ids: &'a [&'a str],
    pub model_version: &'a str,
    pub prompt_version: &'a str,
}

impl AssemblyRecord<'_> {
    /// The §47.1 L1 row as this record carries it.
    ///
    /// Kept in lockstep with [`crate::evaluate::harness::ObservationLayer::Assembly::required_fields`]
    /// by a test rather than by a shared constant: the two tables live in different modules for
    /// different reasons (§47.1 fixes the log shape, §41.5 fixes the record), and a test asserts
    /// they are still item-for-item equal — so editing one without the other fails.
    pub fn logged_field_names() -> [&'static str; 7] {
        [
            "fragment_ref",
            "fragment_source",
            "segment_budget",
            "cropped_items",
            "crop_reason",
            "model_version",
            "prompt_version",
        ]
    }
}

/// One fragment as the log carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedFragment<'a> {
    pub id: &'a str,
    /// 来源标识 — the `fragment_source` field of §47.1's L1 row.
    pub source_ref: &'a str,
    /// The fragment body. `None` means it only ever lived in the live working memory, which is
    /// exactly what H02 forbids depending on.
    pub content: Option<&'a str>,
}

/// Every reason the log cannot reproduce the assembly. Flat, with [`Self::ALL`] and a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildRejection {
    /// A fragment the record references has no entry in the log at all.
    FragmentNotLogged,
    /// The entry exists but its body was not logged — only a pointer into live memory.
    FragmentContentNotInLog,
    /// The log's own ordering of the referenced fragments differs from the recorded entry order.
    RebuildOrderDiffers,
    /// The record carries no model version (§41.5).
    ModelVersionMissing,
    /// The record carries no prompt version (§41.5).
    PromptVersionMissing,
    /// A logged body carries a plaintext credential (INV-11).
    PlaintextCredential,
    /// No **single** log entry carries this assembly: the chain holds the pieces, but no one entry
    /// ever assembled this context. Distinct from [`Self::FragmentNotLogged`] — every referenced id
    /// may be in the log and this reason still holds.
    NotInOneEntry,
}

impl RebuildRejection {
    /// All seven names. Tests assert the constructed set is exactly this.
    pub const ALL: [RebuildRejection; 7] = [
        RebuildRejection::FragmentNotLogged,
        RebuildRejection::FragmentContentNotInLog,
        RebuildRejection::RebuildOrderDiffers,
        RebuildRejection::ModelVersionMissing,
        RebuildRejection::PromptVersionMissing,
        RebuildRejection::PlaintextCredential,
        RebuildRejection::NotInOneEntry,
    ];

    /// Stable decision-log label.
    pub fn label(self) -> &'static str {
        match self {
            RebuildRejection::FragmentNotLogged => "rebuild.fragment_not_logged",
            RebuildRejection::FragmentContentNotInLog => "rebuild.fragment_content_not_in_log",
            RebuildRejection::RebuildOrderDiffers => "rebuild.order_differs",
            RebuildRejection::ModelVersionMissing => "rebuild.model_version_missing",
            RebuildRejection::PromptVersionMissing => "rebuild.prompt_version_missing",
            RebuildRejection::PlaintextCredential => "rebuild.plaintext_credential",
            RebuildRejection::NotInOneEntry => "rebuild.not_in_one_entry",
        }
    }
}

/// One fragment as it was rebuilt: the id and where it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuiltFragment<'a> {
    pub id: &'a str,
    pub source_ref: &'a str,
}

/// Outcome of rebuilding one assembly from the log alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildVerdict<'a> {
    /// True only when the log alone reproduces the recorded assembly.
    pub rebuildable: bool,
    pub rejections: Vec<RebuildRejection>,
    /// The rebuilt sequence, in the log's own order.
    pub rebuilt: Vec<RebuiltFragment<'a>>,
    /// Referenced ids with no log entry.
    pub unlogged_ids: Vec<String>,
    /// Referenced ids logged without their body.
    pub contentless_ids: Vec<String>,
    /// Ids whose logged body carried a credential literal.
    pub credential_ids: Vec<String>,
}

/// §41.5 / H02: reproduce an assembly from **one** pool of logged fragments.
///
/// The judgement is not "does the log mention these fragments" but "does the log, on its own,
/// yield the same sequence". So the rebuilt order is the **log's** order — using the record's
/// order to sort first would make the comparison below true by construction and turn H02 into a
/// comment.
///
/// "One pool" is a parameter, not a nicety: this function answers about whatever slice it is given,
/// and a caller that concatenates several log entries into one slice gets a weaker answer. The
/// chain face is [`rebuild_assembly_from_entries`], which keeps the entries apart.
///
/// INV-11 is enforced by reusing [`crate::evaluate::harness::CREDENTIAL_MARKERS`] rather than a
/// second marker table: two copies would drift into "the log recognises it, the assembly does
/// not", the same discipline that keeps §55's platform reason codes derived from the SDK.
pub fn rebuild_assembly<'log>(
    record: &AssemblyRecord<'_>,
    logged: &'log [LoggedFragment<'log>],
) -> RebuildVerdict<'log> {
    let mut rejections = version_rejections(record);
    let mut rebuilt = Vec::new();
    let mut unlogged_ids = Vec::new();
    let mut contentless_ids = Vec::new();
    let mut credential_ids = Vec::new();

    for id in record.fragment_ids {
        match logged.iter().find(|fragment| fragment.id == *id) {
            None => {
                unlogged_ids.push((*id).to_string());
                refuse(&mut rejections, RebuildRejection::FragmentNotLogged);
            }
            Some(fragment) => {
                if fragment.content.is_none() {
                    contentless_ids.push((*id).to_string());
                    refuse(&mut rejections, RebuildRejection::FragmentContentNotInLog);
                }
            }
        }
    }

    // The rebuilt sequence comes from the **log's** order, not the record's. Sorting by the
    // record first would make the comparison below true by construction.
    for fragment in logged {
        let Some(body) = fragment.content else {
            continue;
        };
        if !record.fragment_ids.contains(&fragment.id) {
            continue;
        }
        let lowered = body.to_ascii_lowercase();
        if crate::evaluate::harness::CREDENTIAL_MARKERS
            .iter()
            .any(|marker| lowered.contains(marker))
        {
            credential_ids.push(fragment.id.to_string());
            refuse(&mut rejections, RebuildRejection::PlaintextCredential);
        }
        rebuilt.push(RebuiltFragment {
            id: fragment.id,
            source_ref: fragment.source_ref,
        });
    }

    // The order comparison is gated on completeness, and the gate is named here rather than
    // silently applied: when a fragment is missing its own reason is already reported, and
    // comparing orders then would report a second, misleading cause. With every fragment
    // present, a differing order means the log alone cannot reproduce the assembly.
    if rebuilt.len() == record.fragment_ids.len() {
        let matches_record = rebuilt
            .iter()
            .zip(record.fragment_ids.iter())
            .all(|(rebuilt, recorded)| rebuilt.id == *recorded);
        if !matches_record {
            refuse(&mut rejections, RebuildRejection::RebuildOrderDiffers);
        }
    }

    RebuildVerdict {
        rebuildable: rejections.is_empty(),
        rejections,
        rebuilt,
        unlogged_ids,
        contentless_ids,
        credential_ids,
    }
}

/// §41.5's item four, as its own named step: both versions must be present.
///
/// Split out so the single-pool judgement and the per-entry one cannot answer this item
/// differently — a second copy of these two lines would be exactly the drift this file's module
/// docs warn about.
fn version_rejections(record: &AssemblyRecord<'_>) -> Vec<RebuildRejection> {
    let mut rejections = Vec::new();
    if record.model_version.trim().is_empty() {
        refuse(&mut rejections, RebuildRejection::ModelVersionMissing);
    }
    if record.prompt_version.trim().is_empty() {
        refuse(&mut rejections, RebuildRejection::PromptVersionMissing);
    }
    rejections
}

/// Whether one log entry — a single assembly row's fragments — carries **exactly** this assembly:
/// the same ids with the same multiplicities, in any order.
///
/// A **multiset**, not a set: a record naming `[f1, f1]` is not carried by an entry naming `[f1]`,
/// and a set comparison would call those equal (a superset entry is a *different* assembly — the
/// log reproducing it as this one would mean editing history). Order is deliberately outside the
/// question: an entry holding the right fragments in the wrong order **is** this assembly's entry,
/// and keeping it selected is what keeps [`RebuildRejection::RebuildOrderDiffers`] reachable below.
/// Selecting by exact order instead would make that rejection unreachable and silently swap a
/// specific reason for a vaguer one.
pub fn entry_carries_assembly<'a>(
    record: &AssemblyRecord<'_>,
    entry: &[LoggedFragment<'a>],
) -> bool {
    if entry.len() != record.fragment_ids.len() {
        return false;
    }
    let mut taken = vec![false; entry.len()];
    'ids: for id in record.fragment_ids {
        for (index, fragment) in entry.iter().enumerate() {
            if !taken[index] && fragment.id == *id {
                taken[index] = true;
                continue 'ids;
            }
        }
        return false;
    }
    true
}

/// §41.5 / H02 over the **chain**, where the log is a sequence of entries rather than one pool.
///
/// "Can the log alone reproduce *this* assembly" is asked of **one entry**, because one entry is
/// one assembly: §41.5's four items describe a single assembly and the write path appends them
/// together, so an entry is the unit the chain was written in. Pooling the entries first — the
/// W104 shape — answers a strictly weaker question ("can the log supply these fragments from
/// somewhere") under which a record whose halves live in two different rows reads as rebuildable
/// even though no entry ever held it.
///
/// The verdict is **existential** over the entries that carry the record: one entry that
/// reproduces it is enough for the chain to reproduce it, so a later entry carrying the same
/// fragments in the wrong order cannot veto an earlier entry that is right. When no carrying entry
/// reproduces it, the reported reasons are the **union** over the carrying entries — each label is
/// individually true of at least one of them and dropping the rest would hide real reasons — while
/// `rebuilt` and the id lists come from the **first** carrying entry, since the sequence is read in
/// that entry's own order.
pub fn rebuild_assembly_from_entries<'log>(
    record: &AssemblyRecord<'_>,
    entries: &[&'log [LoggedFragment<'log>]],
) -> RebuildVerdict<'log> {
    let mut carrying = entries
        .iter()
        .copied()
        .filter(|entry| entry_carries_assembly(record, entry));

    let Some(first) = carrying.next() else {
        // The first check is the one that can say "no entry carries this assembly", and it is a
        // fact about the chain, not a restatement of "a fragment is missing".
        let mut rejections = version_rejections(record);
        let mut unlogged_ids = Vec::new();
        for id in record.fragment_ids {
            // Against the **whole** chain, not an empty pool: an id sitting in some entry is in the
            // log, and calling it unlogged would be false rather than conservative.
            if !entries
                .iter()
                .any(|entry| entry.iter().any(|fragment| fragment.id == *id))
            {
                unlogged_ids.push((*id).to_string());
                refuse(&mut rejections, RebuildRejection::FragmentNotLogged);
            }
        }
        refuse(&mut rejections, RebuildRejection::NotInOneEntry);
        return RebuildVerdict {
            rebuildable: false,
            rejections,
            rebuilt: Vec::new(),
            unlogged_ids,
            contentless_ids: Vec::new(),
            credential_ids: Vec::new(),
        };
    };

    let mut verdict = rebuild_assembly(record, first);
    if verdict.rebuildable {
        return verdict;
    }
    for entry in carrying {
        let other = rebuild_assembly(record, entry);
        if other.rebuildable {
            return other;
        }
        for reason in other.rejections {
            refuse(&mut verdict.rejections, reason);
        }
        for id in other.unlogged_ids {
            if !verdict.unlogged_ids.contains(&id) {
                verdict.unlogged_ids.push(id);
            }
        }
        for id in other.contentless_ids {
            if !verdict.contentless_ids.contains(&id) {
                verdict.contentless_ids.push(id);
            }
        }
        for id in other.credential_ids {
            if !verdict.credential_ids.contains(&id) {
                verdict.credential_ids.push(id);
            }
        }
    }
    verdict
}

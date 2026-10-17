//! The context assembler (V3.11 §41.1–§41.2; acceptance H01/H02).
//!
//! The judge ([`crate::evaluate::assembly::evaluate_assembly`]) and the rebuild check already
//! existed, and `nai-runtime`'s `Composition::record_assembly` already wrote plans to the hash
//! chain — but **nothing produced a plan**. `AssemblyPlan` had exactly two kinds of user: the
//! judge that reads it and the tests that build it by hand. So the chain could never carry a
//! real assembly, and "H01 is wired" was true only in the sense that a body with no caller is
//! wired.
//!
//! This module is the missing producer: candidates grouped by segment go in, one
//! [`AssemblyPlan`](crate::evaluate::assembly::AssemblyPlan) comes out, ordered and cropped.
//!
//! # What this module decides, and what it deliberately does not
//!
//! It decides three things and nothing else:
//!
//! 1. **Order.** §41.1's fixed sequence is imposed on the plan. The caller's input order is not
//!    echoed — the order exists so that 约束不被挤掉, which is exactly what an echo would lose.
//! 2. **Presence.** A segment the caller offers nothing for is **absent** from the plan. It is
//!    never padded with an empty slice: §41.1 makes segments 1–3 required, and it is the
//!    judge's job to refuse an absent one
//!    ([`AssemblyRejection::UncroppableSegmentMissing`](crate::evaluate::assembly::AssemblyRejection::UncroppableSegmentMissing)).
//!    Padding would leave that rejection with no object on the real path.
//! 3. **Cropping.** Within a segment, over-budget candidates are dropped from the **tail** and
//!    the drop is recorded with both halves §41.1 asks for — what went and why. Rows 1–3 are
//!    never cropped, whatever the budget says; the resulting over-budget segment is the judge's
//!    to refuse.
//!
//! # Zero business constants
//!
//! There is no segment table here, no croppability table, and no default budget. The nine
//! segments and their order come from
//! [`SEGMENT_ORDER`](crate::evaluate::assembly::SEGMENT_ORDER), croppability from
//! [`Segment::uncroppable`](crate::evaluate::assembly::Segment::uncroppable), and every cap
//! comes from the call site's [`SegmentBudget`]. §41.2 says the initial values are only a
//! starting point to be tuned by §47.2's golden-set replay; inventing one here would answer
//! §41.2 a second time and freeze a number that no evidence chose.
//!
//! # One thing this module refuses
//!
//! A segment name §41.1 does not define is an [`AssembleError::UnknownSegment`], not a silent
//! drop. Dropping it would make `AssemblyRejection::UnknownSegment` unreachable from this side,
//! and "I did not recognise this" would become "there was nothing to recognise".
//!
//! # Not claimed here
//!
//! Feeding the assembled context to a model, calibrating §41.2's initial budget values against
//! §47.2's golden set (H16), and the six regression classes are **not** this module's work. The
//! plan is the object those steps consume; producing it is all that is claimed.

use crate::evaluate::assembly::{
    AssemblyPlan, Crop, Fragment, Segment, SegmentBudget, SegmentSlice, SegmentUsage,
};

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/// One fragment offered for assembly, with the cost it would add.
///
/// The cost lives beside the fragment rather than inside it because [`Fragment`] is §41.3's
/// provenance record — source, reference, scope, age, trust. Token and character counts are
/// §41.2's budgeting axis, a different concern, and folding them into the provenance record
/// would make "this fragment has no provenance" and "this fragment has no cost" one shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateFragment<'a> {
    pub fragment: Fragment<'a>,
    pub tokens: u64,
    pub chars: u64,
}

/// One segment's candidates, as the call site offers them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentInput<'a> {
    /// §41.1's name. Deliberately a `&str`: an undefined name must be *expressible*, otherwise
    /// [`AssembleError::UnknownSegment`] could never be constructed.
    pub segment: &'a str,
    pub candidates: &'a [CandidateFragment<'a>],
    /// §41.2's per-segment budget, from the call site. `None` stays `None` on the way out.
    pub budget: Option<SegmentBudget>,
}

/// Everything one assembly needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyInput<'a> {
    /// §41.5: the assembly MUST record the model version used for this call.
    pub model_version: &'a str,
    /// §41.5: likewise the prompt version.
    pub prompt_version: &'a str,
    pub segments: &'a [SegmentInput<'a>],
}

// ---------------------------------------------------------------------------
// Failure
// ---------------------------------------------------------------------------

/// The one thing the assembler refuses. Flat and named, in the house style: a caller can tell
/// exactly which rule stopped it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssembleError {
    /// A segment name §41.1 does not define. Not dropped, not renamed, not guessed at.
    UnknownSegment,
}

impl AssembleError {
    /// Stable label.
    pub fn label(self) -> &'static str {
        match self {
            AssembleError::UnknownSegment => "assemble.unknown_segment",
        }
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// One planned segment: the fragments that survived, what they cost, and what was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSegment<'a> {
    pub segment: &'a str,
    pub fragments: Vec<Fragment<'a>>,
    pub usage: SegmentUsage,
    /// §41.2's budget, passed through. Never filled in here.
    pub budget: Option<SegmentBudget>,
    /// Ids dropped by cropping, in the order they were dropped. Empty when nothing was cropped.
    pub dropped: Vec<&'a str>,
    /// Why they were dropped. Present exactly when [`Self::dropped`] is non-empty.
    pub crop_reason: Option<&'a str>,
}

impl<'a> PlannedSegment<'a> {
    /// §41.1's crop record, or `None` when nothing was dropped.
    ///
    /// A crop with an empty `dropped` list is unrepresentable here: the pair is derived from one
    /// vector and one optional reason, so "recorded a crop but named nothing" cannot be built.
    pub fn crop(&self) -> Option<Crop<'_>> {
        self.crop_reason.map(|reason| Crop {
            dropped: &self.dropped,
            reason,
        })
    }
}

/// The assembled plan, owned.
///
/// It is not an `AssemblyPlan` itself because `AssemblyPlan::slices` is a borrowed slice and the
/// plan is built at run time — a struct that owned its slices *and* borrowed them would be
/// self-referential. [`Self::with_plan`] is the borrow view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAssembly<'a> {
    model_version: &'a str,
    prompt_version: &'a str,
    segments: Vec<PlannedSegment<'a>>,
}

impl<'a> PlannedAssembly<'a> {
    /// The segments, in §41.1's order.
    pub fn segments(&self) -> &[PlannedSegment<'a>] {
        &self.segments
    }

    /// Hand a borrowed [`AssemblyPlan`] to `use_it`.
    ///
    /// The slice vector is built per call rather than stored, which is what keeps the type free
    /// of self-reference. Every call sees the same plan — nothing here mutates.
    pub fn with_plan<R>(&self, use_it: impl FnOnce(&AssemblyPlan<'_>) -> R) -> R {
        let slices: Vec<SegmentSlice<'_>> = self
            .segments
            .iter()
            .map(|segment| SegmentSlice {
                segment: segment.segment,
                fragments: &segment.fragments,
                usage: segment.usage,
                budget: segment.budget,
                crop: segment.crop(),
            })
            .collect();

        let plan = AssemblyPlan {
            slices: &slices,
            model_version: self.model_version,
            prompt_version: self.prompt_version,
        };

        use_it(&plan)
    }
}

// ---------------------------------------------------------------------------
// The assembler
// ---------------------------------------------------------------------------

/// Assemble §41.1's context from segment candidates.
///
/// Output is ordered by [`SEGMENT_ORDER`], one entry per segment that was offered, cropped from
/// the tail where the call site's budget says so. Segments with no candidates are absent.
pub fn plan_assembly<'a>(
    input: &'a AssemblyInput<'a>,
) -> Result<PlannedAssembly<'a>, AssembleError> {
    // Refuse before doing any work: a name §41.1 does not define cannot be ordered, and ordering
    // it by accident would be worse than refusing.
    let mut parsed: Vec<(usize, &'a SegmentInput<'a>)> = Vec::with_capacity(input.segments.len());
    for segment in input.segments {
        match Segment::parse(segment.segment) {
            Some(known) => parsed.push((known.index(), segment)),
            None => return Err(AssembleError::UnknownSegment),
        }
    }

    // A stable sort keeps two offers of the same segment in the order the caller gave them. That
    // is deliberate: a repeated segment is `SegmentOutOfOrder` to the judge, and if this module
    // reordered the duplicates the judge would lose the evidence it needs.
    parsed.sort_by_key(|(index, _)| *index);

    let segments: Vec<PlannedSegment<'a>> = parsed
        .into_iter()
        .map(|(_, input)| plan_segment(input))
        .collect();

    Ok(PlannedAssembly {
        model_version: input.model_version,
        prompt_version: input.prompt_version,
        segments,
    })
}

/// Plan one segment: crop the tail to fit the call site's budget, then report what survived.
fn plan_segment<'a>(input: &'a SegmentInput<'a>) -> PlannedSegment<'a> {
    // `plan_assembly` has already established that this parses.
    let known = Segment::parse(input.segment).expect("validated by plan_assembly");

    let offered = input.candidates;
    let mut keep = offered.len();
    let mut reason: Option<&'a str> = None;

    // §41.1 rows 1–3 are 不可裁. Cropping them would turn a budget problem into a
    // "the safety constraints were trimmed" problem — the opposite of what the order is for.
    if !known.uncroppable()
        && let Some(budget) = input.budget
    {
        if let Some(cap) = budget.items
            && keep > cap
        {
            keep = cap;
            reason = Some(ITEMS_OVER_BUDGET);
        }
        if let Some(cap) = budget.tokens {
            let (fits, _) = take_while_fits(offered, keep, cap, |candidate| candidate.tokens);
            if fits < keep {
                keep = fits;
                reason.get_or_insert(TOKENS_OVER_BUDGET);
            }
        }
        if let Some(cap) = budget.chars {
            let (fits, _) = take_while_fits(offered, keep, cap, |candidate| candidate.chars);
            if fits < keep {
                keep = fits;
                reason.get_or_insert(CHARS_OVER_BUDGET);
            }
        }
    }

    let kept = &offered[..keep];
    let usage = SegmentUsage {
        tokens: kept.iter().map(|candidate| candidate.tokens).sum(),
        chars: kept.iter().map(|candidate| candidate.chars).sum(),
        items: kept.len(),
    };

    PlannedSegment {
        segment: input.segment,
        fragments: kept
            .iter()
            .map(|candidate| candidate.fragment.clone())
            .collect(),
        usage,
        budget: input.budget,
        dropped: offered[keep..]
            .iter()
            .map(|candidate| candidate.fragment.id)
            .collect(),
        crop_reason: reason,
    }
}

/// How many leading candidates fit under `cap`, and what they cost.
///
/// Prefix, not "any subset": cropping runs from the back, so what survives is always a prefix.
fn take_while_fits<'a>(
    candidates: &'a [CandidateFragment<'a>],
    limit: usize,
    cap: u64,
    cost: impl Fn(&CandidateFragment<'a>) -> u64,
) -> (usize, u64) {
    let mut spent = 0u64;
    let mut taken = 0usize;
    for candidate in candidates.iter().take(limit) {
        let next = spent.saturating_add(cost(candidate));
        if next > cap {
            break;
        }
        spent = next;
        taken += 1;
    }
    (taken, spent)
}

const ITEMS_OVER_BUDGET: &str = "items_over_budget";
const TOKENS_OVER_BUDGET: &str = "tokens_over_budget";
const CHARS_OVER_BUDGET: &str = "chars_over_budget";

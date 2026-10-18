//! Criteria for the media lineage registry.
//!
//! Each case below is the regression for one degeneration:
//! * `cancellation_names_...` — reporting a count without a manifest;
//! * `terminal_states_...` — a terminal cell quietly disappearing;
//! * `grace_is_a_recorded_value_...` — grace implemented as waiting instead of
//!   as a value compared against a caller-supplied clock.

use nai_media_core::lineage::{
    FacetKind, LineageError, LineageId, LineageRegistry, NodeState, RECYCLE_GRACE_MS,
};
use std::collections::BTreeSet;

fn id(utterance: &str, epoch: u64) -> LineageId {
    LineageId::new("s", utterance, epoch)
}

fn registry() -> LineageRegistry {
    LineageRegistry::new("s", 7)
}

#[test]
fn facet_kinds_are_a_closed_set_with_distinct_names() {
    let mut names = BTreeSet::new();
    for kind in FacetKind::ALL {
        assert!(names.insert(kind.as_str()), "duplicate facet name");
    }
    assert_eq!(names.len(), FacetKind::ALL.len());
    assert_eq!(FacetKind::ALL.len(), 5);
}

#[test]
fn terminal_states_include_cancelled_and_recycled_and_number_three() {
    let terminal: Vec<NodeState> = NodeState::ALL
        .iter()
        .copied()
        .filter(|s| s.terminal())
        .collect();
    assert_eq!(terminal.len(), 3, "a terminal cell was dropped");
    assert!(NodeState::Cancelled.terminal());
    assert!(NodeState::Recycled.terminal());
    assert!(NodeState::Released.terminal());
    assert!(!NodeState::InFlight.terminal());
    // Every state is classified exactly once: terminal and non-terminal
    // partition ALL.
    let others = NodeState::ALL.iter().filter(|s| !s.terminal()).count();
    assert_eq!(terminal.len() + others, NodeState::ALL.len());
    assert_eq!(others, 1);
    let mut names = BTreeSet::new();
    for state in NodeState::ALL {
        assert!(names.insert(state.as_str()), "duplicate state name");
    }
    assert_eq!(names.len(), NodeState::ALL.len());
}

#[test]
fn cancellation_names_every_child_it_drops_and_the_count_is_the_manifest() {
    let mut reg = registry();
    for utterance in ["u1", "u2", "u3"] {
        reg.register(id(utterance, 7), FacetKind::UtteranceRow, 10_000)
            .unwrap();
        reg.register(id(utterance, 7), FacetKind::PendingGate, 10_000)
            .unwrap();
    }
    assert_eq!(reg.in_flight(), 6);
    let report = reg.cancel_scope();
    assert_eq!(report.count(), 6);
    assert_eq!(report.manifest.len(), report.count());
    for utterance in ["u1", "u2", "u3"] {
        for kind in [FacetKind::UtteranceRow, FacetKind::PendingGate] {
            assert!(
                report.manifest.iter().any(|r| r.id == id(utterance, 7)
                    && r.kind == kind
                    && r.state == NodeState::Cancelled),
                "cancellation did not name {utterance} / {}",
                kind.as_str()
            );
        }
    }
    // The count reports the nodes that really changed state, not a tally kept
    // beside them.
    let actually_cancelled = reg
        .snapshot()
        .iter()
        .filter(|r| r.state == NodeState::Cancelled)
        .count();
    assert_eq!(actually_cancelled, report.count());
    assert_eq!(reg.in_flight(), 0);
    // A second cancellation has nothing left to name, and reports that as zero
    // rather than skipping.
    let second = reg.cancel_scope();
    assert_eq!(second.count(), 0);
    assert!(second.is_empty());
}

#[test]
fn grace_is_a_recorded_value_and_reclaim_never_waits() {
    let mut reg = registry();
    reg.register(id("early", 7), FacetKind::UtteranceRow, 1_000)
        .unwrap();
    reg.register(id("late", 7), FacetKind::UtteranceRow, 9_000)
        .unwrap();
    // The deadline is the value handed in at registration, not a clock read
    // later.
    assert_eq!(
        reg.node(&id("early", 7), FacetKind::UtteranceRow)
            .unwrap()
            .recycle_at_ms,
        1_000
    );
    assert_eq!(
        reg.node(&id("late", 7), FacetKind::UtteranceRow)
            .unwrap()
            .recycle_at_ms,
        9_000
    );
    let _ = reg.cancel_scope();
    // Nothing is due before the earliest recorded value.
    assert!(reg.reclaim(999).is_empty());
    let first = reg.reclaim(1_000);
    assert_eq!(first.count(), 1, "reclaim took more than what was due");
    assert_eq!(first.manifest[0].id, id("early", 7));
    assert_eq!(first.manifest[0].state, NodeState::Recycled);
    let second = reg.reclaim(9_000);
    assert_eq!(second.count(), 1);
    assert_eq!(second.manifest[0].id, id("late", 7));
    // Reclaiming twice at the same clock reports zero the second time.
    assert!(reg.reclaim(9_000).is_empty());
    assert_eq!(reg.len(), 0);

    // A node still in flight is never reclaimed, however far the clock runs.
    let mut live = registry();
    live.register(id("busy", 7), FacetKind::UtteranceRow, 1_000)
        .unwrap();
    assert!(live.reclaim(u64::MAX).is_empty());
    assert_eq!(live.in_flight(), 1);
}

#[test]
fn a_child_without_a_parent_and_an_out_of_scope_id_are_refused() {
    let mut reg = registry();
    assert_eq!(
        reg.register(id("u", 7), FacetKind::PendingGate, 1_000),
        Err(LineageError::UnknownParent)
    );
    assert_eq!(
        reg.register(
            LineageId::new("other", "u", 7),
            FacetKind::UtteranceRow,
            1_000
        ),
        Err(LineageError::ScopeMismatch)
    );
    assert_eq!(
        reg.register(id("u", 8), FacetKind::UtteranceRow, 1_000),
        Err(LineageError::ScopeMismatch)
    );
    assert_eq!(reg.advance_epoch(6), Err(LineageError::EpochRegression));
    reg.advance_epoch(8).unwrap();
    // Once the scope has moved, the previous generation is out of scope.
    assert_eq!(
        reg.register(id("u", 7), FacetKind::UtteranceRow, 1_000),
        Err(LineageError::ScopeMismatch)
    );
    reg.register(id("u", 8), FacetKind::UtteranceRow, 1_000)
        .unwrap();
    reg.register(id("u", 8), FacetKind::AudioBlock, 1_000)
        .unwrap();
    assert_eq!(reg.in_flight(), 2);
}

#[test]
fn release_names_the_child_it_moved_and_reports_zero_the_second_time() {
    let mut reg = registry();
    reg.register(id("u", 7), FacetKind::UtteranceRow, 1_000)
        .unwrap();
    reg.register(id("u", 7), FacetKind::SlotTicket, 1_000)
        .unwrap();
    let released = reg.release(&id("u", 7), FacetKind::SlotTicket);
    assert_eq!(released.count(), 1);
    assert_eq!(released.manifest[0].state, NodeState::Released);
    assert_eq!(
        reg.node(&id("u", 7), FacetKind::SlotTicket).unwrap().state,
        NodeState::Released
    );
    assert_eq!(reg.release(&id("u", 7), FacetKind::SlotTicket).count(), 0);
    assert_eq!(reg.release(&id("u", 7), FacetKind::OutputAck).count(), 0);
    // Releasing a child leaves the parent alone.
    assert_eq!(reg.in_flight(), 1);
}

#[test]
fn reclamation_is_bounded_across_generations() {
    let mut reg = registry();
    let mut clock = 0u64;
    for epoch in 7..12 {
        let _ = reg.cancel_scope();
        reg.advance_epoch(epoch).unwrap();
        for utterance in ["u1", "u2"] {
            reg.register(
                LineageId::new("s", utterance, epoch),
                FacetKind::UtteranceRow,
                clock + RECYCLE_GRACE_MS,
            )
            .unwrap();
        }
        let _ = reg.reclaim(clock);
        assert_eq!(reg.len(), 2, "generation {epoch} left stale nodes behind");
        assert_eq!(reg.in_flight(), 2);
        clock += RECYCLE_GRACE_MS + 1;
    }
    // Only the newest generation is still held; everything older is gone.
    let _ = reg.cancel_scope();
    assert_eq!(reg.reclaim(clock).count(), 2);
    assert_eq!(reg.len(), 0);
}

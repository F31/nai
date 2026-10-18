//! Causal lineage registry for the media path.
//!
//! One lineage is one utterance in one generation of one session:
//! `(session_id, utterance_id, generation_epoch)`. Children register under a
//! [`FacetKind`]; the parent is the recoverable copy of the utterance, modelled
//! here as [`FacetKind::UtteranceRow`] (the SQL `utterances` row is the copy
//! that survives a restart).
//!
//! Two properties this module exists to hold:
//!
//! * Cancellation **names** what it dropped. A [`LineageReport`] carries a
//!   manifest and a count derived from that manifest, so the two cannot drift
//!   apart — there is no separately maintained counter to disagree with.
//! * The grace before reclamation is a **value** (`recycle_at_ms`), never a
//!   sleep. Nothing in this module blocks on time: `reclaim` compares a
//!   caller-supplied clock against the recorded deadline.

use std::collections::BTreeMap;

/// Grace added to a lease/cue expiry when a node's reclamation deadline is
/// recorded. Reclamation never waits for it; see [`LineageRegistry::reclaim`].
pub const RECYCLE_GRACE_MS: u64 = 5_000;

/// One causal chain: a single utterance in a single generation of a session.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LineageId {
    pub session_id: String,
    pub utterance_id: String,
    pub generation_epoch: u64,
}

impl LineageId {
    pub fn new(session_id: &str, utterance_id: &str, generation_epoch: u64) -> Self {
        Self {
            session_id: session_id.to_owned(),
            utterance_id: utterance_id.to_owned(),
            generation_epoch,
        }
    }
}

/// What a registered node is. Closed set: a child of a lineage is always
/// registered under exactly one of these, and the parent is `UtteranceRow`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FacetKind {
    /// The recoverable utterance row itself. The parent every child needs.
    UtteranceRow,
    /// An utterance waiting in the playback gate queue.
    PendingGate,
    /// A decoded audio block owned by the mixing path.
    AudioBlock,
    /// A frame-slot ticket held by the renderer.
    SlotTicket,
    /// An output acknowledgement not yet settled.
    OutputAck,
}

impl FacetKind {
    /// Closed world: every kind the registry can hold. Adding a variant here
    /// without registering it in `tests/lineage.rs` fails the closed-world case.
    pub const ALL: [FacetKind; 5] = [
        FacetKind::UtteranceRow,
        FacetKind::PendingGate,
        FacetKind::AudioBlock,
        FacetKind::SlotTicket,
        FacetKind::OutputAck,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            FacetKind::UtteranceRow => "utterance_row",
            FacetKind::PendingGate => "pending_gate",
            FacetKind::AudioBlock => "audio_block",
            FacetKind::SlotTicket => "slot_ticket",
            FacetKind::OutputAck => "output_ack",
        }
    }
}

/// Closed set of four. Two of them are terminal cells (`cancelled`,
/// `recycled`) and the classification is derived in exactly one place:
/// [`NodeState::terminal`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NodeState {
    InFlight,
    Released,
    Cancelled,
    Recycled,
}

/// The only place terminality is decided. `tests/lineage.rs` asserts the
/// partition covers `ALL` exactly once.
const TERMINAL_STATES: [NodeState; 3] = [
    NodeState::Released,
    NodeState::Cancelled,
    NodeState::Recycled,
];

impl NodeState {
    pub const ALL: [NodeState; 4] = [
        NodeState::InFlight,
        NodeState::Released,
        NodeState::Cancelled,
        NodeState::Recycled,
    ];
    pub fn terminal(self) -> bool {
        TERMINAL_STATES.contains(&self)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            NodeState::InFlight => "in_flight",
            NodeState::Released => "released",
            NodeState::Cancelled => "cancelled",
            NodeState::Recycled => "recycled",
        }
    }
}

/// A registered node. `recycle_at_ms` is the reclamation deadline as a value,
/// recorded when the node is registered and never recomputed from a clock that
/// is read later.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LineageNode {
    pub state: NodeState,
    pub recycle_at_ms: u64,
}

/// One named entry of a report: which lineage, which facade, and the state that
/// entry was moved into.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LineageRef {
    pub id: LineageId,
    pub kind: FacetKind,
    pub state: NodeState,
}

/// Manifest and count together. `count()` is derived from `manifest`, so a
/// report can never claim a number it cannot name.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LineageReport {
    pub manifest: Vec<LineageRef>,
}

impl LineageReport {
    pub fn count(&self) -> usize {
        self.manifest.len()
    }
    pub fn is_empty(&self) -> bool {
        self.manifest.is_empty()
    }
}

/// Why a registration or a scope move was refused. Closed set of three.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineageError {
    /// The id belongs to another session or another generation than the
    /// registry scope.
    ScopeMismatch,
    /// No `UtteranceRow` is registered for this id: a child with no parent.
    UnknownParent,
    /// The scope was asked to move to an earlier generation.
    EpochRegression,
}

/// The lineage of one media gate: children of the utterances that are admitted
/// under the gate's current `(session, generation)`.
pub struct LineageRegistry {
    scope_session: String,
    scope_epoch: u64,
    nodes: BTreeMap<(LineageId, FacetKind), LineageNode>,
}

impl LineageRegistry {
    pub fn new(session: &str, generation_epoch: u64) -> Self {
        Self {
            scope_session: session.to_owned(),
            scope_epoch: generation_epoch,
            nodes: BTreeMap::new(),
        }
    }
    pub fn scope_session(&self) -> &str {
        &self.scope_session
    }
    pub fn scope_epoch(&self) -> u64 {
        self.scope_epoch
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn node(&self, id: &LineageId, kind: FacetKind) -> Option<&LineageNode> {
        self.nodes.get(&(id.clone(), kind))
    }
    /// Nodes still in flight. Derived by counting, never stored.
    pub fn in_flight(&self) -> usize {
        self.nodes
            .values()
            .filter(|node| node.state == NodeState::InFlight)
            .count()
    }
    /// Every node the registry holds, in key order.
    pub fn snapshot(&self) -> Vec<LineageRef> {
        self.nodes
            .iter()
            .map(|((id, kind), node)| LineageRef {
                id: id.clone(),
                kind: *kind,
                state: node.state,
            })
            .collect()
    }

    /// Register a node. A node already in flight is refreshed (the deadline is
    /// re-recorded) rather than refused, because re-admitting the same child of
    /// the same lineage is the same child, not a second one.
    pub fn register(
        &mut self,
        id: LineageId,
        kind: FacetKind,
        recycle_at_ms: u64,
    ) -> Result<(), LineageError> {
        if id.session_id != self.scope_session || id.generation_epoch != self.scope_epoch {
            return Err(LineageError::ScopeMismatch);
        }
        if kind != FacetKind::UtteranceRow
            && !self
                .nodes
                .contains_key(&(id.clone(), FacetKind::UtteranceRow))
        {
            return Err(LineageError::UnknownParent);
        }
        self.nodes.insert(
            (id, kind),
            LineageNode {
                state: NodeState::InFlight,
                recycle_at_ms,
            },
        );
        Ok(())
    }

    /// Release one child. The manifest names what actually changed state; a
    /// child that is absent or already terminal yields an empty manifest, which
    /// is a reported zero rather than a silent skip.
    pub fn release(&mut self, id: &LineageId, kind: FacetKind) -> LineageReport {
        let mut manifest = Vec::new();
        if let Some(node) = self.nodes.get_mut(&(id.clone(), kind))
            && node.state == NodeState::InFlight
        {
            node.state = NodeState::Released;
            manifest.push(LineageRef {
                id: id.clone(),
                kind,
                state: NodeState::Released,
            });
        }
        LineageReport { manifest }
    }

    /// Cancel everything in flight in this scope and name it. A node keeps the
    /// `recycle_at_ms` it was registered with: cancellation does not consult a
    /// clock, it only moves the state.
    pub fn cancel_scope(&mut self) -> LineageReport {
        let mut manifest = Vec::new();
        for ((id, kind), node) in self.nodes.iter_mut() {
            if node.state == NodeState::InFlight {
                node.state = NodeState::Cancelled;
                manifest.push(LineageRef {
                    id: id.clone(),
                    kind: *kind,
                    state: NodeState::Cancelled,
                });
            }
        }
        LineageReport { manifest }
    }

    /// Reclaim every node whose recorded deadline has come due. In-flight nodes
    /// are never reclaimed. The manifest names what was taken; a second call at
    /// the same clock reports zero.
    pub fn reclaim(&mut self, utc_ms: u64) -> LineageReport {
        let mut manifest = Vec::new();
        let mut due = Vec::new();
        for ((id, kind), node) in self.nodes.iter() {
            if node.state != NodeState::InFlight && node.recycle_at_ms <= utc_ms {
                due.push((id.clone(), *kind));
            }
        }
        for (id, kind) in due {
            self.nodes.remove(&(id.clone(), kind));
            manifest.push(LineageRef {
                id,
                kind,
                state: NodeState::Recycled,
            });
        }
        LineageReport { manifest }
    }

    /// Move the scope to a newer generation. Nodes of older generations are not
    /// dropped here; they stay cancelled until their deadline comes due.
    pub fn advance_epoch(&mut self, generation_epoch: u64) -> Result<(), LineageError> {
        if generation_epoch < self.scope_epoch {
            return Err(LineageError::EpochRegression);
        }
        self.scope_epoch = generation_epoch;
        Ok(())
    }
}

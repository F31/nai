//! Memory write-gate runtime wiring: judge a memory candidate against §42.2's source gate and, only
//! when it passes, land the record in the **real** `memory_records` table (V3.8 §42.2; W107,
//! acceptance H03).
//!
//! W57 landed both halves and wired neither. `nai_kernel::evaluate::memory::admit_write` decides
//! whether a write may become a fact or a promise, and `memory_records` (from `0001-foundation.sql`)
//! is where such a record belongs — but `admit_write` had no production caller at all, and no
//! production path ever touched the table. So "does a **refused** write still land?" was a question
//! only a code reader could answer. This module is the runtime half.
//!
//! Two invariants, both testable:
//!
//! * **Judged admissible ⇒ the judged record lands.** The nine columns are copied from the record
//!   the gate saw. The runtime is a *transport*, not an enricher: it recomputes no column, defaults
//!   none, and lands nothing the gate did not vet.
//! * **Judged inadmissible ⇒ a named refusal, and not one column written.** The refusal carries the
//!   gate's own label, so "which §42.2 rule refused this" stays decidable at the call site.
//!
//! Judgement and persistence are one entry point on purpose. Offering a separate `admit` and
//! `persist` would move the "never persist an unjudged write" duty onto every caller, where one
//! omission breaks the invariant and no test can see it.
//!
//! The judgement is answered **once**. [`Store::record_memory`] checks its own layer (scope
//! identifiers, session permission, primary key) and MUST NOT re-run §42.2 — a gate answered twice
//! drifts (W88).
//!
//! **Boundary (disclosed, not implied):** the §42.3 **read** judgement
//! ([`nai_kernel::evaluate::memory::read_records`]) is **not** wired here. Its entry-level inputs are
//! `ScopedRecord`'s `revoked` / `permitted` / `approximate_of`, and `memory_records` carries no
//! revocation column — asserting `revoked: false` would fabricate a judgement input, which this
//! project forbids (W94's shape). The storage accessor
//! [`Store::memory_records`] is a per-column fetch and MUST NOT be read as having answered it.

use crate::composition::short;
use crate::{Composition, Result, err};
use nai_contracts::v1::{ErrorCode, MemoryRecord};
use nai_kernel::evaluate::memory::{MemoryCandidate, MemoryPolicy, admit_write};

/// Why a memory write was refused: which record, and which §42.2 rule.
///
/// Both halves are required, for the same reason [`MemoryRejection`] exists at all — a refusal that
/// named neither the record nor the rule would be indistinguishable from a silent drop.
///
/// [`MemoryRejection`]: nai_kernel::evaluate::memory::MemoryRejection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWriteRefusal {
    /// The candidate's own record id, so a caller can act on the right record.
    pub record_id: String,
    /// The gate's stable label for the rule that refused it.
    pub reason: &'static str,
}

/// The outcome of offering one memory candidate to the real memory store.
///
/// Two arms, and that is the whole classification: H03 is **one** judgement (§42.2's source gate),
/// so there is one axis — admitted or not. A third arm would name a state no fact corresponds to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryWriteOutcome {
    /// The gate admitted it; the record now sits in `memory_records`.
    Written(MemoryRecord),
    /// §42.2 refused it. **Nothing** was written.
    Refused(MemoryWriteRefusal),
}

impl Composition {
    /// Judge `candidate` against §42.2 and, only when it passes, persist it to `memory_records`.
    ///
    /// The persisted columns are the judged record's own nine fields and nothing else — see the
    /// module docs for why the gate's other inputs (`source`, `claim`, `source_ref`,
    /// `written_at_utc_ms`, `publish_ref`) are *not* columns here and so are not persisted.
    ///
    /// Returns `Err` (not a refusal arm) for the store's own failures — a duplicate record id, a
    /// missing permission, a bad identifier. Those are not §42.2 verdicts, and folding them into
    /// [`MemoryWriteOutcome::Refused`] would put a storage fault under the gate's name.
    pub fn write_memory(
        &mut self,
        candidate: &MemoryCandidate,
        policy: &MemoryPolicy,
    ) -> Result<MemoryWriteOutcome> {
        if let Err(rejection) = admit_write(candidate, policy) {
            return Ok(MemoryWriteOutcome::Refused(MemoryWriteRefusal {
                record_id: candidate.record.record_id.clone(),
                reason: rejection.label(),
            }));
        }
        let record = candidate.record.clone();
        let operation = short()?;
        let workspace = self.workspace().to_owned();
        let actor = self.actor().to_owned();
        let mut store = self
            .store()
            .try_lock()
            .map_err(|_| err(ErrorCode::Conflict, "memory store busy"))?;
        store
            .record_memory(&operation, &workspace, &actor, &record)
            .map_err(crate::RuntimeError::from)?;
        Ok(MemoryWriteOutcome::Written(record))
    }
}

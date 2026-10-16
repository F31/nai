//! The runtime's upgrade seam: the activation boundary, and the one call site
//! that drives an `UpgradeTransaction`.
//!
//! The upgrade core owns the *order* of the steps and the *decisions* between
//! them, but it deliberately holds no session state: "is anyone on air right
//! now?" is a fact this assembly root owns. W123 wired the root to *answer* that
//! question; this module is where the answer is *used*.
//!
//! Two calls, not one, because a package is safe to stage at any time while only
//! the swap of a running version needs silence. A live show therefore **defers**
//! an upgrade instead of failing it, and the already-staged package is what the
//! next silent boundary activates.
//!
//! # What is real here and what is injected
//!
//! The session fact is real: it is read from the `Session` this composition
//! owns, through the `ActiveSessionGate` implementation in `composition.rs`.
//! The host and the journal are **injected seams** — the platform installer that
//! stages bytes, snapshots a data directory and swaps a manifest with a rename
//! is a later injection, and so is the real package distributor. This module
//! therefore never claims to install anything: it owns the orchestration and the
//! gate, and it reports every world effect through the seams it was handed.
use crate::{Composition, Result};
use nai_upgrade_core::{
    Activation, Operation, Outcome, Prepared, SessionGate, UpgradeHost, UpgradeJournal,
    UpgradeRequest, UpgradeTransaction,
};

/// The verdict at the activation boundary.
///
/// Deliberately not a `bool`, for the same reason the core's `Activation` is not
/// one: *why* a swap did not happen is part of the answer, so a caller waits on
/// the right thing instead of polling a flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeActivation {
    /// No session was on air, so the transaction ran and reported its terminal
    /// `Outcome`. `Outcome::RolledBack` is a completed run with a named cause —
    /// a failed swap is *reported*, never deferred.
    Ran(Outcome),
    /// A live session deferred the swap. The staged package stays valid and MUST
    /// be reused at the next silent boundary, never staged a second time.
    ///
    /// An **unobservable** session is not this variant: a lock the root cannot
    /// take is reported as an error, because reading it as "no live session"
    /// would swap the running version out from under a live show.
    Deferred { active: usize },
}

impl Composition {
    /// Verify, preflight and stage a package. **Staging is safe at any time**,
    /// so this MUST NOT consult the session gate: doing so would turn a live show
    /// into a failed upgrade rather than a deferred one.
    ///
    /// The budget is the caller's, as at every entry point of the upgrade core.
    /// Only a caller knows how long this package's migration may take, and a
    /// budget chosen here would be a second, drifting policy.
    pub fn stage_upgrade(
        &self,
        host: &mut dyn UpgradeHost,
        journal: &mut dyn UpgradeJournal,
        op: &Operation,
        request: &UpgradeRequest<'_>,
    ) -> Result<Prepared> {
        let mut transaction = UpgradeTransaction::new(host, journal);
        let prepared = transaction.prepare(op, request)?;
        Ok(prepared)
    }

    /// The activation boundary: ask the gate **first**, and swap only when it
    /// allows. A deferral is a state, not an error.
    ///
    /// The gate reads this composition's real session, and it is asked here and
    /// nowhere else, so "nobody is on air" is decided once — a second decision
    /// site would be a second definition to keep true.
    pub fn activate_staged(
        &self,
        host: &mut dyn UpgradeHost,
        journal: &mut dyn UpgradeJournal,
        op: &Operation,
        prepared: &Prepared,
    ) -> Result<UpgradeActivation> {
        match SessionGate::new(self).activation(op)? {
            Activation::Deferred { active } => Ok(UpgradeActivation::Deferred { active }),
            Activation::Allowed => {
                let mut transaction = UpgradeTransaction::new(host, journal);
                let outcome = transaction.activate(op, prepared)?;
                Ok(UpgradeActivation::Ran(outcome))
            }
        }
    }
}

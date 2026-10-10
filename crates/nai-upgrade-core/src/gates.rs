//! Session gates: staging is always allowed, activation waits for silence, and
//! a downgrade drains before it retires.
//!
//! The core keeps no session state of its own. "Is anyone live right now?" is a
//! fact the assembly root owns, so it enters through [`ActiveSessionGate`] and
//! the core only decides what to do with the answer. A cached session count
//! inside the upgrade core would be a second, stale truth — and the one thing
//! worse than not gating an activation is gating it on a number that is no
//! longer true.
//!
//! Gating is a *separate call* from activation on purpose. Staging is safe at
//! any time; only the swap of a running version needs silence. Modelling that as
//! two calls means a live-session block defers the upgrade instead of failing
//! it, and the already-staged package is what the post-drain activation uses.
use crate::{Result, error, manifest::UpgradePackage};
use nai_contracts::v1::ErrorCode;
use nai_security::Operation;
use semver::{Version, VersionReq};

/// Live sessions, as seen from outside the core.
pub trait ActiveSessionGate {
    /// Sessions that are live right now.
    fn active(&self) -> Result<usize>;
}

/// The verdict on activation. Deliberately not a `bool`: *why* activation is
/// deferred is part of the answer, so a caller waits on the right thing instead
/// of polling a bare flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    Allowed,
    Deferred { active: usize },
}

/// The session fact, wrapped so the core's decision is a single call.
pub struct SessionGate<'a> {
    gate: &'a dyn ActiveSessionGate,
}

impl<'a> SessionGate<'a> {
    pub fn new(gate: &'a dyn ActiveSessionGate) -> Self {
        Self { gate }
    }

    /// Whether activation may proceed now. Staging is unaffected: the caller
    /// stages first and only asks here at the activation boundary.
    pub fn activation(&self, op: &Operation) -> Result<Activation> {
        op.check()?;
        let active = self.gate.active()?;
        Ok(if active == 0 {
            Activation::Allowed
        } else {
            Activation::Deferred { active }
        })
    }

    /// Refuse activation while any session is live. A deferral is not an error
    /// to retry blindly — it is the signal that the staged package waits for the
    /// next silent boundary.
    pub fn require_idle(&self, op: &Operation) -> Result<()> {
        match self.activation(op)? {
            Activation::Allowed => Ok(()),
            Activation::Deferred { .. } => Err(error(
                ErrorCode::CapabilityNotReady,
                "activation requires no live session",
            )),
        }
    }
}

/// The effect side of a downgrade: ending surplus sessions and retiring the
/// capabilities they depended on.
pub trait SessionDrain {
    /// Sessions still depending on `capability`.
    fn dependents(&self, capability: &str) -> Result<usize>;
    /// Ask those sessions to finish and report how many remain. MUST be
    /// idempotent: draining twice ends nothing twice.
    fn drain(&mut self, op: &Operation, capability: &str) -> Result<usize>;
    /// Disable a capability once nothing depends on it. MUST NOT delete stored
    /// data: a downgrade retires code, never facts.
    fn retire(&mut self, op: &Operation, capability: &str) -> Result<()>;
}

/// What a downgrade did, so a caller reports it rather than guesses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DowngradeReport {
    pub retired: Vec<String>,
    pub sessions_ended: usize,
}

/// Drain, then retire — per capability, in that order.
///
/// The order is the whole requirement. A capability is disabled only after its
/// dependents are gone, and the drain's own report is what is believed — not the
/// assumption that asking was enough. A capability whose sessions outlive the
/// drain is left running and the downgrade stops, rather than stranding those
/// sessions on something that no longer exists. Nothing here deletes data: the
/// [`SessionDrain::retire`] contract forbids it, and a downgrade that loses
/// facts is worse than one that declines.
pub fn downgrade(
    op: &Operation,
    drain: &mut dyn SessionDrain,
    retire: &[String],
) -> Result<DowngradeReport> {
    op.check()?;
    if retire.is_empty() {
        return Err(error(
            ErrorCode::InvalidArgument,
            "downgrade retires no capability",
        ));
    }
    let mut report = DowngradeReport::default();
    for capability in retire {
        if capability.trim().is_empty() || capability.len() > 128 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid capability reference",
            ));
        }
        op.check()?;
        let before = drain.dependents(capability)?;
        let remaining = drain.drain(op, capability)?;
        if remaining > 0 {
            return Err(error(
                ErrorCode::Conflict,
                "sessions still depend on a capability to retire",
            ));
        }
        op.check()?;
        drain.retire(op, capability)?;
        report.sessions_ended += before;
        report.retired.push(capability.clone());
    }
    Ok(report)
}

/// Whether a rollback to `target` is permitted.
///
/// A target outside the package's `rollback_range` is refused with a version
/// error rather than silently attempted: a program that was never built to
/// un-migrate a newer data layout is exactly how a "successful" rollback
/// corrupts data. Incompatible is a refusal, not a preference to override.
pub fn require_rollback_compatible(package: &UpgradePackage, target: &str) -> Result<()> {
    let target_version = Version::parse(target)
        .map_err(|_| error(ErrorCode::InvalidArgument, "unparsable rollback target"))?;
    let range = VersionReq::parse(&package.rollback_range)
        .map_err(|_| error(ErrorCode::InvalidArgument, "unparsable rollback range"))?;
    if range.matches(&target_version) {
        Ok(())
    } else {
        Err(error(
            ErrorCode::VersionIncompatible,
            "rollback target outside the declared rollback range",
        ))
    }
}

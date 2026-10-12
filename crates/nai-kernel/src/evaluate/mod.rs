//! Portable agent/guard evaluation: decision routing, four-stage guard with
//! bounded rewrite, skill/show/memory rules. Pure logic, no I/O: verdicts are
//! derived from controlled rules, never from model self-report, and zero
//! values never mean pass, granted permission or authorization.
pub mod agent;
pub mod guard;
pub mod harness;
pub mod memory;
pub mod ramp;
pub mod show;
pub mod skill;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluateError {
    Unauthorized,
    Invalid,
    Conflict,
    Expired,
    Capacity,
    Stale,
}

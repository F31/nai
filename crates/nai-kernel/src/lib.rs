//! Portable guards the V5.6.1 workbench actually calls: output ownership, OBS
//! window observation, session lifecycle, and the golden-set data model used by
//! `cargo xtask check --golden`.
//!
//! Removed with the V5.6.1 baseline (ADR 0193), all of it V4.8 and without a
//! producer once the capability catalog froze to three `workbench.*` entries:
//!
//! * `evaluate::*` — the 62-INV judgement plane over `agent.decide`,
//!   `guard.check`, `knowledge.retrieve`, `live.program`, …
//! * `registry` — plugin admission/binding bound to `guard.check`,
//!   `avatar.profile`, platform supervision and budget admission references.
//!   Capability-name validation now has one home: `nai-plugin-sdk` against
//!   `CAPABILITY_IDS`.
//! * `assemble`, `demo`, `device`, `bridge`, `platform`, `realistic`,
//!   `multiplex`, `scheduler`, `monitoring` — only reachable from the plane above.
//!
//! Re-adding any of it needs a new OpenSpec change and ADR, not a silent restore.
pub mod duty;
pub mod golden;
pub mod metrics;
pub mod obs;
pub mod obs_dest;
pub mod obs_observe;
pub mod output_owner;
pub mod review;
pub mod session;

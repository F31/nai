//! Portable registration/binding kernel. Ready is binding preflight, not host
//! startup, content authorization, a playback lease or device certification.
pub mod device;
pub mod duty;
pub mod evaluate;
pub mod golden;
pub mod metrics;
pub mod monitoring;
pub mod multiplex;
pub mod platform;
pub mod realistic;
mod registry;
pub mod review;
pub mod scheduler;
pub mod session;
pub use registry::{Admission, CallContext, Limits, Registry};

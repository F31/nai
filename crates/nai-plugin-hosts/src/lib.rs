//! Host boundary: CapabilityHost, AccessPolicy, identity validation, verified
//! load paths and native isolation. Hosting an untrusted native subprocess is
//! explicitly unavailable until a certified OS sandbox is verified.
pub mod acl;
pub mod boundary;
pub mod host;

pub use acl::{AccessPolicy, AccessRule};
pub use boundary::{NativeBoundary, OsPeer, scrub_env, verify_within_root};
pub use host::{CapabilityHost, HostLimits, IdentityValidator};

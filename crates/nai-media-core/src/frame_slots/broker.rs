//! Linux-only protected-handle authorization broker (ADR 0018). Handle delivery
//! is still spawn inheritance: this grants control operations only to peers that
//! kernel state associates with a root-declared identity. There is no
//! SCM_RIGHTS, Windows named-pipe handoff or production IPC authentication, and
//! a grant is never device authorization, output readiness or a PlaybackAck.
//!
//! `UnixStream::peer_cred`/`SO_PEERCRED` is still unstable on the pinned
//! toolchain, so the connecting peer's pid cannot be read directly without
//! `unsafe` or a new dependency. Identity is therefore derived from `/proc`:
//! the declared pid must exist, be owned by the pool owner uid, and hold the
//! pool file (matched by mount id + inode). A UDS connection is required as the
//! liveness trigger, but this substitutes kernel FD possession for connection
//! peer authentication; see the report for the residual gap.
use super::shared_ram::SharedFramePool;
#[cfg(target_os = "linux")]
use super::shared_ram::owner_uid;
use super::{IoBudget, PoolSpec, Result, SlotOwner, error, shared_ram::OwnedSlotFrame};
use nai_contracts::v1::{ErrorCode, FrameMetadata, FrameSlotCommit, FrameSlotTicket};
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixListener;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::time::Duration;
use std::time::Instant;

/// Bounded accept budget for one `poll_grants` call; a flood cannot force an
/// unbounded accept loop.
pub const MAX_ACCEPT_ATTEMPTS: usize = 8;

/// Cooperative client liveness trigger only, not an authorization receipt.
pub fn connect_trigger(path: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::{fs::FileTypeExt, net::UnixStream};
        let metadata = std::fs::symlink_metadata(path).map_err(io_failure)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != owner_uid()?
            || metadata.mode() & 0o077 != 0
        {
            return Err(error(
                ErrorCode::PermissionDenied,
                "broker trigger path rejected",
            ));
        }
        let stream = UnixStream::connect(path).map_err(io_failure)?;
        stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(io_failure)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Err(error(
            ErrorCode::CapabilityNotReady,
            "broker client is Linux-only",
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerRole {
    Producer,
    Consumer,
}

#[derive(Clone, Debug)]
pub struct PeerIdentity {
    pub role: PeerRole,
    pub pid: i32,
    pub uid: u32,
}

#[cfg(target_os = "linux")]
fn io_failure(_: std::io::Error) -> nai_contracts::v1::ContractError {
    error(
        ErrorCode::CapabilityNotReady,
        "broker socket IO unavailable or rejected",
    )
}

/// Owner uid of a live process, straight from kernel `/proc` state.
#[cfg(target_os = "linux")]
fn process_uid(pid: i32) -> Option<u32> {
    std::fs::metadata(format!("/proc/{pid}"))
        .ok()
        .map(|meta| meta.uid())
}

/// Whether the process currently holds a descriptor for the pool identified by
/// `(mount, inode)`. Descriptor targets are matched through `/proc/<pid>/fdinfo`.
#[cfg(target_os = "linux")]
fn holds_pool_file(pid: i32, mount: u64, inode: u64) -> bool {
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(info) = std::fs::read_to_string(format!(
            "/proc/{pid}/fdinfo/{}",
            entry.file_name().to_string_lossy()
        )) else {
            continue;
        };
        let mut candidate_mount = None;
        let mut candidate_inode = None;
        for line in info.lines() {
            if let Some(value) = line.strip_prefix("mnt_id:") {
                candidate_mount = value.trim().parse::<u64>().ok();
            } else if let Some(value) = line.strip_prefix("ino:") {
                candidate_inode = value.trim().parse::<u64>().ok();
            }
        }
        if candidate_mount == Some(mount) && candidate_inode == Some(inode) {
            return true;
        }
    }
    false
}

pub struct FrameBroker {
    pool: SharedFramePool,
    peers: Vec<PeerIdentity>,
    producer_authorized: bool,
    consumer_authorized: bool,
    #[cfg(target_os = "linux")]
    listener: UnixListener,
    #[cfg(target_os = "linux")]
    socket_path: std::path::PathBuf,
}
impl FrameBroker {
    /// Root-only. Reuses the ADR 0017 private-directory and pool-creation rules,
    /// then binds `<pool_id>.sock` with mode `0600` inside that directory. Peers
    /// may be supplied up front or declared after a spawn via `declare_peer`.
    pub fn bind(private_dir: &Path, spec: PoolSpec, peers: Vec<PeerIdentity>) -> Result<Self> {
        if peers.len() > 2
            || peers.iter().any(|p| p.pid <= 0)
            || peers
                .iter()
                .enumerate()
                .any(|(i, p)| peers[..i].iter().any(|q| q.role == p.role))
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "broker requires at most two distinct peers",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            let pool = SharedFramePool::create(private_dir, spec)?;
            let socket_path = private_dir.join(format!("{}.sock", pool.owner().spec().pool_id));
            let listener = UnixListener::bind(&socket_path).map_err(io_failure)?;
            std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))
                .map_err(io_failure)?;
            listener.set_nonblocking(true).map_err(io_failure)?;
            Ok(Self {
                pool,
                peers,
                producer_authorized: false,
                consumer_authorized: false,
                listener,
                socket_path,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (private_dir, spec, peers);
            Err(error(
                ErrorCode::CapabilityNotReady,
                "protected handle broker is Linux-only; SCM_RIGHTS and Windows named pipes are not implemented",
            ))
        }
    }
    pub fn owner(&self) -> &SlotOwner {
        self.pool.owner()
    }
    #[cfg(target_os = "linux")]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
    /// Root declares a peer after spawning it (root knows the pid it launched);
    /// the identity is matched against kernel credentials, never self-reported.
    pub fn declare_peer(&mut self, identity: PeerIdentity) -> Result<()> {
        if identity.pid <= 0 {
            return Err(error(ErrorCode::InvalidArgument, "peer pid rejected"));
        }
        if self.peers.len() >= 2 || self.peers.iter().any(|p| p.role == identity.role) {
            return Err(error(
                ErrorCode::Conflict,
                "peer role already declared or capacity reached",
            ));
        }
        self.peers.push(identity);
        Ok(())
    }
    /// Accepts pending peer connections until a declared identity is verified
    /// from kernel `/proc` evidence, the deadline elapses, or the accept budget
    /// is exhausted. The connection is only a liveness trigger: the connecting
    /// peer is not authenticated (see the module note). Rejected peers are
    /// closed and never granted anything.
    pub fn poll_grants(&mut self, deadline: Instant) -> Result<PeerIdentity> {
        #[cfg(target_os = "linux")]
        {
            let mut attempts = 0usize;
            let mut rejected = false;
            loop {
                if Instant::now() >= deadline {
                    return Err(if rejected {
                        error(ErrorCode::PermissionDenied, "peer identity not verified")
                    } else {
                        error(
                            ErrorCode::DeadlineExceeded,
                            "no peer connection within deadline",
                        )
                    });
                }
                match self.listener.accept() {
                    Ok((stream, _)) => {
                        attempts += 1;
                        if attempts > MAX_ACCEPT_ATTEMPTS {
                            return Err(error(
                                ErrorCode::ResourceExhausted,
                                "broker accept budget exhausted",
                            ));
                        }
                        drop(stream);
                        if let Some(identity) = self.verify() {
                            return Ok(identity);
                        }
                        rejected = true;
                    }
                    Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => {
                        return Err(error(ErrorCode::CapabilityNotReady, "broker accept failed"));
                    }
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = deadline;
            Err(error(
                ErrorCode::CapabilityNotReady,
                "protected handle broker is Linux-only",
            ))
        }
    }
    /// Kernel-derived verification of the next unverified declared peer. It
    /// requires the declared pid to be owned by the pool owner uid and to hold
    /// the pool file; a wrong or foreign pid can never satisfy this.
    #[cfg(target_os = "linux")]
    fn verify(&mut self) -> Option<PeerIdentity> {
        let owner = owner_uid().ok()?;
        let (mount, inode) = self.pool.pool_identity().ok()?;
        let index = self.peers.iter().position(|peer| {
            peer.uid == owner
                && !self.authorized(peer.role)
                && process_uid(peer.pid) == Some(owner)
                && holds_pool_file(peer.pid, mount, inode)
        })?;
        let identity = self.peers[index].clone();
        self.grant(identity.role);
        Some(identity)
    }
    fn authorized(&self, role: PeerRole) -> bool {
        match role {
            PeerRole::Producer => self.producer_authorized,
            PeerRole::Consumer => self.consumer_authorized,
        }
    }
    #[cfg(target_os = "linux")]
    fn grant(&mut self, role: PeerRole) {
        match role {
            PeerRole::Producer => self.producer_authorized = true,
            PeerRole::Consumer => self.consumer_authorized = true,
        }
    }
    fn revoke(&mut self, role: PeerRole) {
        match role {
            PeerRole::Producer => self.producer_authorized = false,
            PeerRole::Consumer => self.consumer_authorized = false,
        }
    }
    fn require(&self, role: PeerRole) -> Result<()> {
        // Authorization is established once, at grant time, from the kernel
        // evidence in `verify` (declared pid, owner uid and pool FD); revocation
        // is explicit via `disconnect`. Re-checking liveness per operation would
        // reject a producer that finished writing and exited before root commits.
        if self.authorized(role) && !self.pool.owner().is_retired() {
            Ok(())
        } else {
            Err(error(
                ErrorCode::PermissionDenied,
                "peer identity not verified",
            ))
        }
    }
    pub fn reserve(&mut self, frame: FrameMetadata, now: Instant) -> Result<FrameSlotTicket> {
        self.require(PeerRole::Producer)?;
        let producer = self.pool.owner().spec().producer_ref.clone();
        self.pool.reserve(&producer, frame, now)
    }
    pub fn commit(&mut self, commit: FrameSlotCommit, now: Instant) -> Result<()> {
        self.require(PeerRole::Producer)?;
        let producer = self.pool.owner().spec().producer_ref.clone();
        self.pool.commit(&producer, commit, now)
    }
    pub fn producer_file(&self) -> Result<std::fs::File> {
        self.require(PeerRole::Producer)?;
        let producer = self.pool.owner().spec().producer_ref.clone();
        self.pool.producer_file(&producer)
    }
    /// Root-only spawn-inheritance handle. Root created the pool, so it may clone
    /// the pool file to launch the producer before that producer is authorized;
    /// the launched process still cannot reserve, commit or read until its
    /// declared identity is verified. This is not a peer grant.
    pub fn producer_spawn_handle(&self) -> Result<std::fs::File> {
        let producer = self.pool.owner().spec().producer_ref.clone();
        self.pool.producer_file(&producer)
    }
    pub fn consumer_spawn_handle(&self) -> Result<std::fs::File> {
        self.pool
            .consumer_file(&self.pool.owner().spec().consumer_ref)
    }
    pub fn begin_consume(
        &mut self,
        ticket: &FrameSlotTicket,
        now: Instant,
    ) -> Result<FrameSlotCommit> {
        self.require(PeerRole::Consumer)?;
        let actor = self.owner().spec().consumer_ref.clone();
        self.pool.acquire(&actor, ticket, now)
    }
    pub fn release(&mut self, ticket: &FrameSlotTicket, now: Instant) -> Result<()> {
        self.require(PeerRole::Consumer)?;
        let actor = self.owner().spec().consumer_ref.clone();
        self.pool.release(&actor, ticket, now)
    }
    pub fn read_frame(
        &mut self,
        ticket: &FrameSlotTicket,
        budget: &IoBudget,
    ) -> Result<OwnedSlotFrame> {
        self.require(PeerRole::Consumer)?;
        let consumer = self.pool.owner().spec().consumer_ref.clone();
        self.pool.read_frame(&consumer, ticket, budget)
    }
    /// Revokes the role's grant and retires the whole pool (ADR 0017 semantics).
    pub fn disconnect(&mut self, role: PeerRole) -> Result<()> {
        let actor = match role {
            PeerRole::Producer => self.pool.owner().spec().producer_ref.clone(),
            PeerRole::Consumer => self.pool.owner().spec().consumer_ref.clone(),
        };
        self.revoke(role);
        self.pool.disconnect(&actor)
    }
}
impl Drop for FrameBroker {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

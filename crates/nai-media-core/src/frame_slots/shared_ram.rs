//! Linux-only, explicitly selected tmpfs-file diagnostic backend (ADR 0017).
//! Safe read_at/write_at with copies; no mmap, production IPC or device grant.
#[cfg(target_os = "linux")]
use super::MAX_POOL_BYTES;
use super::{
    IoBudget, MAX_BUDGET, PoolSpec, Result, SlotOwner, error, frame_bytes, validate_ticket,
};
use nai_contracts::v1::{ErrorCode, FrameMetadata, FrameSlotCommit, FrameSlotTicket};
use sha2::{Digest, Sha256};
#[cfg(target_os = "linux")]
use std::{
    fs,
    io::Read,
    os::unix::fs::{FileExt, MetadataExt, PermissionsExt},
};
use std::{fs::File, path::Path, time::Instant};

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[cfg(target_os = "linux")]
fn io_failure(_: std::io::Error) -> nai_contracts::v1::ContractError {
    error(
        ErrorCode::CapabilityNotReady,
        "shared RAM IO unavailable or corrupted",
    )
}

#[cfg(target_os = "linux")]
fn tmpfs_device() -> Result<u64> {
    // This is a Linux engineering backend, not arbitrary filesystem transport.
    let mut mounts = String::new();
    File::open("/proc/self/mountinfo")
        .map_err(io_failure)?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut mounts)
        .map_err(io_failure)?;
    if mounts.len() > 1024 * 1024
        || !mounts.lines().any(|line| {
            line.split_once(" - ").is_some_and(|(left, right)| {
                left.split_whitespace().nth(4) == Some("/dev/shm")
                    && right.split_whitespace().next() == Some("tmpfs")
            })
        })
    {
        return Err(error(
            ErrorCode::CapabilityNotReady,
            "/dev/shm tmpfs backend not verified",
        ));
    }
    Ok(fs::metadata("/dev/shm").map_err(io_failure)?.dev())
}
#[cfg(target_os = "linux")]
pub(crate) fn owner_uid() -> Result<u32> {
    Ok(fs::metadata("/proc/self").map_err(io_failure)?.uid())
}
#[cfg(target_os = "linux")]
pub(crate) fn validate_file(file: &File) -> Result<u64> {
    let meta = file.metadata().map_err(io_failure)?;
    if !meta.is_file()
        || meta.dev() != tmpfs_device()?
        || meta.uid() != owner_uid()?
        || meta.permissions().mode() & 0o077 != 0
        || meta.len() == 0
        || meta.len() > MAX_POOL_BYTES
    {
        return Err(error(
            ErrorCode::PermissionDenied,
            "FD is not a private bounded shared RAM object",
        ));
    }
    Ok(meta.len())
}

/// Owner-ledger-independent pixel read shared by the single-owner pool and the
/// consumer-side reader. It validates ticket/commit/pool identity, exact pool and
/// RGBA length, chunked reads and SHA256 only; it grants no output authorization,
/// consults no slot ledger and produces no PlaybackAck.
pub fn read_committed_frame(
    file: &File,
    spec: &PoolSpec,
    consumer: &str,
    ticket: &FrameSlotTicket,
    commit: &FrameSlotCommit,
    budget: &IoBudget,
) -> Result<Vec<u8>> {
    budget.check()?;
    let bytes = validate_ticket(ticket)?;
    if ticket.consumer_ref != consumer {
        return Err(error(ErrorCode::PermissionDenied, "not pool consumer"));
    }
    if ticket.pool_id != spec.pool_id
        || ticket.session_id != spec.session_id
        || ticket.generation_epoch != spec.generation_epoch
        || ticket.producer_ref != spec.producer_ref
        || ticket.consumer_ref != spec.consumer_ref
        || ticket.max_slot_bytes != spec.slot_bytes
        || ticket.slot_index >= spec.slots
        || commit.ticket.as_ref() != Some(ticket)
        || commit.byte_length != bytes
        || commit.sha256.len() != 64
    {
        return Err(error(
            ErrorCode::InvalidArgument,
            "commit/ticket/pool mismatch rejected",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let length = validate_file(file)?;
        if length != spec.slots as u64 * spec.slot_bytes as u64 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "shared RAM length changed",
            ));
        }
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(commit.byte_length as usize)
            .map_err(|_| error(ErrorCode::ResourceExhausted, "frame copy allocation failed"))?;
        rgba.resize(commit.byte_length as usize, 0);
        let offset = ticket.slot_index as u64 * ticket.max_slot_bytes as u64;
        for (index, chunk) in rgba.chunks_mut(65536).enumerate() {
            budget.check()?;
            file.read_exact_at(chunk, offset + index as u64 * 65536)
                .map_err(io_failure)?;
        }
        if digest(&rgba) != commit.sha256 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "shared frame SHA256 mismatch",
            ));
        }
        budget.check()?;
        Ok(rgba)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (file, spec, bytes);
        Err(error(
            ErrorCode::CapabilityNotReady,
            "shared RAM backend unavailable",
        ))
    }
}

pub struct SharedFramePool {
    owner: SlotOwner,
    #[cfg(target_os = "linux")]
    file: File,
}
pub struct OwnedSlotFrame {
    pub ticket: FrameSlotTicket,
    pub metadata: FrameMetadata,
    pub rgba: Vec<u8>,
}
impl SharedFramePool {
    pub fn acquire(
        &mut self,
        actor: &str,
        ticket: &FrameSlotTicket,
        now: Instant,
    ) -> Result<FrameSlotCommit> {
        self.owner.acquire(actor, ticket, now)
    }
    pub fn consumer_file(&self, actor: &str) -> Result<File> {
        if self.owner.is_retired() || actor != self.owner.spec().consumer_ref {
            return Err(error(
                ErrorCode::PermissionDenied,
                "consumer FD access rejected",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            File::open(format!("/proc/self/fd/{}", self.file.as_raw_fd())).map_err(io_failure)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(error(
                ErrorCode::CapabilityNotReady,
                "consumer backend unavailable",
            ))
        }
    }
    /// Root-only creation. The supplied pool_id is replaced by a fresh file/inode
    /// identity; paths never arrive in a peer control message.
    pub fn create(private_dir: &Path, spec: PoolSpec) -> Result<Self> {
        spec.validate()?;
        #[cfg(target_os = "linux")]
        {
            let mut spec = spec;
            let canonical = fs::canonicalize(private_dir).map_err(io_failure)?;
            let meta = fs::symlink_metadata(private_dir).map_err(io_failure)?;
            if canonical != private_dir
                || !canonical.starts_with("/dev/shm")
                || !meta.is_dir()
                || meta.dev() != tmpfs_device()?
                || meta.uid() != owner_uid()?
                || meta.permissions().mode() & 0o077 != 0
            {
                return Err(error(
                    ErrorCode::PermissionDenied,
                    "private shared RAM directory rejected",
                ));
            }
            let temp = tempfile::Builder::new()
                .prefix("nai-frame-")
                .tempfile_in(private_dir)
                .map_err(io_failure)?;
            let file = temp.as_file().try_clone().map_err(io_failure)?;
            spec.pool_id = format!(
                "{}-{}",
                temp.path().file_name().unwrap().to_string_lossy(),
                file.metadata().map_err(io_failure)?.ino()
            );
            drop(temp); // Unlink immediately; only inherited/root-owned FDs remain.
            file.set_len(spec.slots as u64 * spec.slot_bytes as u64)
                .map_err(io_failure)?;
            validate_file(&file)?;
            Ok(Self {
                owner: SlotOwner::new(spec)?,
                file,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (private_dir, spec);
            Err(error(
                ErrorCode::CapabilityNotReady,
                "tmpfs-file-drill is Linux-only; production mapping unavailable",
            ))
        }
    }
    pub fn owner(&self) -> &SlotOwner {
        &self.owner
    }
    pub fn reserve(
        &mut self,
        producer: &str,
        frame: FrameMetadata,
        now: Instant,
    ) -> Result<FrameSlotTicket> {
        self.owner.reserve(producer, frame, MAX_BUDGET, now)
    }
    pub fn poll(&mut self, now: Instant) -> Result<()> {
        self.owner.poll(now)
    }
    pub fn disconnect(&mut self, actor: &str) -> Result<()> {
        self.owner.disconnect(actor)
    }
    pub fn commit(&mut self, actor: &str, commit: FrameSlotCommit, now: Instant) -> Result<()> {
        self.owner.commit(actor, commit, now)
    }
    pub fn release(&mut self, actor: &str, ticket: &FrameSlotTicket, now: Instant) -> Result<()> {
        self.owner.release(actor, ticket, now)
    }
    /// Diagnostic bootstrap only. A producer holding this FD can still touch old
    /// RAM after logical retirement; supervisor must stop/reap that process.
    pub fn producer_file(&self, actor: &str) -> Result<File> {
        if self.owner.is_retired() || actor != self.owner.spec().producer_ref {
            return Err(error(
                ErrorCode::PermissionDenied,
                "producer FD access rejected",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            self.file.try_clone().map_err(io_failure)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(error(
                ErrorCode::CapabilityNotReady,
                "shared RAM backend unavailable",
            ))
        }
    }
    pub fn read_frame(
        &mut self,
        consumer: &str,
        ticket: &FrameSlotTicket,
        budget: &IoBudget,
    ) -> Result<OwnedSlotFrame> {
        let commit = self.owner.acquire(consumer, ticket, Instant::now())?;
        // The closure exists to turn the two `?` handlers below into one
        // `result`, which is also why the non-Linux branch is written as a plain
        // block: there no `?` runs from this body, so an immediately-invoked
        // closure would be a pure wrapper.
        #[cfg(target_os = "linux")]
        let result = (|| {
            let rgba = read_committed_frame(
                &self.file,
                self.owner.spec(),
                consumer,
                ticket,
                &commit,
                budget,
            )?;
            self.owner.validate_read(consumer, ticket, Instant::now())?;
            Ok(OwnedSlotFrame {
                ticket: ticket.clone(),
                metadata: ticket.frame.as_ref().unwrap().clone(),
                rgba,
            })
        })();
        #[cfg(not(target_os = "linux"))]
        let result: Result<OwnedSlotFrame> = {
            let _ = (consumer, ticket, budget, &commit);
            Err(error(
                ErrorCode::CapabilityNotReady,
                "shared RAM backend unavailable",
            ))
        };
        if result.is_err() {
            self.owner.retire();
        }
        result
    }
}

#[cfg(target_os = "linux")]
impl SharedFramePool {
    /// Kernel identity (mount id, inode) of the private pool file, used for
    /// `/proc`-based holder verification. It is not a path and not a grant.
    pub(crate) fn pool_identity(&self) -> Result<(u64, u64)> {
        use std::os::fd::AsRawFd;
        let info = fs::read_to_string(format!("/proc/self/fdinfo/{}", self.file.as_raw_fd()))
            .map_err(io_failure)?;
        let mut mount = None;
        let mut inode = None;
        for line in info.lines() {
            if let Some(value) = line.strip_prefix("mnt_id:") {
                mount = value.trim().parse().ok();
            } else if let Some(value) = line.strip_prefix("ino:") {
                inode = value.trim().parse().ok();
            }
        }
        match (mount, inode) {
            (Some(mount), Some(inode)) if inode != 0 => Ok((mount, inode)),
            _ => Err(error(
                ErrorCode::CapabilityNotReady,
                "pool file identity unavailable",
            )),
        }
    }
}

/// Cooperative producer IO. This validates structure/integrity, not a peer's
/// permission to play; only the owner can accept the commit against its ledger.
pub fn write_frame(
    file: &File,
    ticket: &FrameSlotTicket,
    rgba: &[u8],
    budget: &IoBudget,
) -> Result<FrameSlotCommit> {
    budget.check()?;
    let bytes = validate_ticket(ticket)?;
    if bytes as usize != rgba.len() || frame_bytes(ticket.frame.as_ref().unwrap())? != bytes {
        return Err(error(
            ErrorCode::InvalidArgument,
            "producer frame length rejected",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let offset = validate_producer_handle(file, ticket)?;
        for (index, chunk) in rgba.chunks(65536).enumerate() {
            budget.check()?;
            file.write_all_at(chunk, offset + index as u64 * 65536)
                .map_err(io_failure)?;
        }
        let sha256 = digest(rgba);
        budget.check()?;
        Ok(FrameSlotCommit {
            ticket: Some(ticket.clone()),
            byte_length: bytes,
            sha256,
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (file, rgba);
        Err(error(
            ErrorCode::CapabilityNotReady,
            "shared RAM backend unavailable",
        ))
    }
}

pub fn validate_producer_handle(file: &File, ticket: &FrameSlotTicket) -> Result<u64> {
    let bytes = validate_ticket(ticket)?;
    #[cfg(target_os = "linux")]
    {
        let length = validate_file(file)?;
        let offset = ticket.slot_index as u64 * ticket.max_slot_bytes as u64;
        if !length.is_multiple_of(ticket.max_slot_bytes as u64)
            || length / ticket.max_slot_bytes as u64 > super::MAX_SLOTS as u64
            || offset + bytes as u64 > length
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "producer slot extent rejected",
            ));
        }
        Ok(offset)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (file, bytes);
        Err(error(
            ErrorCode::CapabilityNotReady,
            "shared RAM backend unavailable",
        ))
    }
}

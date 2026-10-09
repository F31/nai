//! Single-owner control ledger. Tickets/commits contain no RGBA data and grant
//! no output authorization. Retirement is permanent; create a fresh pool.
pub mod broker;
pub mod consumer;
pub mod shared_ram;
use nai_contracts::v1::{
    ContractError, ErrorCode, FrameMetadata, FrameSlotCommit, FrameSlotTicket,
};
use prost::Message;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const MAX_SLOTS: u32 = 16;
pub const MAX_SLOT_BYTES: u32 = 8 * 1024 * 1024;
pub const MAX_POOL_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_BUDGET: Duration = Duration::from_millis(250);
pub type Result<T> = std::result::Result<T, ContractError>;
pub(crate) fn error(code: ErrorCode, message: &str) -> ContractError {
    ContractError {
        code: code as i32,
        category: "frame-slots".into(),
        message: message.into(),
        ..Default::default()
    }
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

#[derive(Clone, Debug)]
pub struct PoolSpec {
    pub pool_id: String,
    pub session_id: String,
    pub generation_epoch: u64,
    pub producer_ref: String,
    pub consumer_ref: String,
    pub slots: u32,
    pub slot_bytes: u32,
}
impl PoolSpec {
    pub fn validate(&self) -> Result<()> {
        if !id(&self.pool_id)
            || !id(&self.session_id)
            || !id(&self.producer_ref)
            || !id(&self.consumer_ref)
            || self.producer_ref == self.consumer_ref
            || self.generation_epoch == 0
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "pool identity/scope rejected",
            ));
        }
        if !(1..=MAX_SLOTS).contains(&self.slots)
            || !(4..=MAX_SLOT_BYTES).contains(&self.slot_bytes)
            || self.slots as u64 * self.slot_bytes as u64 > MAX_POOL_BYTES
        {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "frame pool capacity rejected",
            ));
        }
        Ok(())
    }
}

pub fn frame_bytes(frame: &FrameMetadata) -> Result<u32> {
    if frame.encoded_len() > 1024
        || !id(&frame.session_id)
        || !id(&frame.utterance_id)
        || frame.generation_epoch == 0
        || frame.width == 0
        || frame.height == 0
        || frame.width > 1920
        || frame.height > 1080
        || frame.pixel_format != "rgba8"
        || frame.pts < 0
        || frame.duration == 0
        || frame.time_base_num == 0
        || frame.time_base_den == 0
    {
        return Err(error(ErrorCode::InvalidArgument, "RGBA metadata rejected"));
    }
    Ok(frame.width * frame.height * 4)
}
pub fn buffer_ref(ticket: &FrameSlotTicket) -> String {
    format!(
        "{}:{}:{}",
        ticket.pool_id, ticket.slot_index, ticket.slot_generation
    )
}
pub fn validate_ticket(ticket: &FrameSlotTicket) -> Result<u32> {
    if ticket.schema_version != 1
        || !id(&ticket.pool_id)
        || !id(&ticket.session_id)
        || !id(&ticket.producer_ref)
        || !id(&ticket.consumer_ref)
        || ticket.producer_ref == ticket.consumer_ref
        || ticket.generation_epoch == 0
        || ticket.slot_generation == 0
        || ticket.slot_index >= MAX_SLOTS
        || !(4..=MAX_SLOT_BYTES).contains(&ticket.max_slot_bytes)
        || !(1..=250).contains(&ticket.remaining_budget_ms)
    {
        return Err(error(ErrorCode::InvalidArgument, "slot ticket rejected"));
    }
    let frame = ticket
        .frame
        .as_ref()
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "missing slot metadata"))?;
    let bytes = frame_bytes(frame)?;
    if frame.session_id != ticket.session_id
        || frame.generation_epoch != ticket.generation_epoch
        || frame.buffer_ref != buffer_ref(ticket)
        || bytes > ticket.max_slot_bytes
    {
        return Err(error(
            ErrorCode::InvalidArgument,
            "ticket frame/length mismatch",
        ));
    }
    Ok(bytes)
}

/// IO budget is independent of the owner's non-renewable slot deadline.
pub struct IoBudget {
    pub deadline: Instant,
    pub cancelled: Arc<AtomicBool>,
}
impl IoBudget {
    pub fn new(duration: Duration) -> Result<Self> {
        Self::starting_at(Instant::now(), duration)
    }
    /// Anchors the deadline to an explicit start (e.g. a frame event's observed
    /// time) so a stale event cannot silently borrow fresh consumer IO time.
    pub fn starting_at(start: Instant, duration: Duration) -> Result<Self> {
        if duration.is_zero() || duration > MAX_BUDGET {
            return Err(error(ErrorCode::InvalidArgument, "IO budget rejected"));
        }
        Ok(Self {
            deadline: start + duration,
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(error(ErrorCode::Cancelled, "frame IO cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(error(ErrorCode::DeadlineExceeded, "frame IO expired"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotPhase {
    Free,
    Writing,
    Ready,
    Reading,
    Retired,
}
struct Slot {
    generation: u64,
    phase: SlotPhase,
    deadline: Option<Instant>,
    ticket: Option<FrameSlotTicket>,
    commit: Option<FrameSlotCommit>,
}
pub struct SlotOwner {
    spec: PoolSpec,
    slots: Vec<Slot>,
    retired: bool,
}
impl SlotOwner {
    /// Root supplies a fresh protected pool identity (never reuse after retirement).
    pub fn new(spec: PoolSpec) -> Result<Self> {
        spec.validate()?;
        let slots = (0..spec.slots)
            .map(|_| Slot {
                generation: 0,
                phase: SlotPhase::Free,
                deadline: None,
                ticket: None,
                commit: None,
            })
            .collect();
        Ok(Self {
            spec,
            slots,
            retired: false,
        })
    }
    pub fn spec(&self) -> &PoolSpec {
        &self.spec
    }
    pub fn is_retired(&self) -> bool {
        self.retired
    }
    pub fn phase(&self, index: u32) -> Result<SlotPhase> {
        self.slots
            .get(index as usize)
            .map(|s| s.phase)
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "slot index rejected"))
    }
    pub fn retire(&mut self) {
        self.retired = true;
        for slot in &mut self.slots {
            slot.phase = SlotPhase::Retired;
            slot.ticket = None;
            slot.commit = None;
            slot.deadline = None;
        }
    }
    pub fn poll(&mut self, now: Instant) -> Result<()> {
        if self.retired {
            return Err(error(ErrorCode::Cancelled, "frame pool retired"));
        }
        if self
            .slots
            .iter()
            .any(|s| s.deadline.is_some_and(|d| now >= d))
        {
            self.retire();
            return Err(error(
                ErrorCode::DeadlineExceeded,
                "frame slot expired; pool retired",
            ));
        }
        Ok(())
    }
    pub fn disconnect(&mut self, actor: &str) -> Result<()> {
        if actor != self.spec.producer_ref && actor != self.spec.consumer_ref {
            return Err(error(ErrorCode::PermissionDenied, "unknown pool peer"));
        }
        self.retire();
        Ok(())
    }
    pub fn reserve(
        &mut self,
        producer: &str,
        mut frame: FrameMetadata,
        budget: Duration,
        now: Instant,
    ) -> Result<FrameSlotTicket> {
        self.poll(now)?;
        if producer != self.spec.producer_ref {
            return Err(error(ErrorCode::PermissionDenied, "not pool producer"));
        }
        let bytes = frame_bytes(&frame)?;
        if frame.session_id != self.spec.session_id
            || frame.generation_epoch != self.spec.generation_epoch
        {
            return Err(error(ErrorCode::StaleEpoch, "frame pool scope rejected"));
        }
        if bytes > self.spec.slot_bytes {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "frame does not fit slot",
            ));
        }
        if budget < Duration::from_millis(1) || budget > MAX_BUDGET {
            return Err(error(ErrorCode::InvalidArgument, "slot budget rejected"));
        }
        let (index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.phase == SlotPhase::Free)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "frame slots full"))?;
        let generation = slot
            .generation
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "slot generation exhausted"))?;
        let mut ticket = FrameSlotTicket {
            schema_version: 1,
            pool_id: self.spec.pool_id.clone(),
            session_id: self.spec.session_id.clone(),
            generation_epoch: self.spec.generation_epoch,
            slot_index: index as u32,
            slot_generation: generation,
            producer_ref: self.spec.producer_ref.clone(),
            consumer_ref: self.spec.consumer_ref.clone(),
            frame: None,
            max_slot_bytes: self.spec.slot_bytes,
            remaining_budget_ms: budget.as_millis() as u32,
        };
        frame.buffer_ref = buffer_ref(&ticket);
        ticket.frame = Some(frame);
        slot.generation = generation;
        slot.phase = SlotPhase::Writing;
        slot.deadline = Some(now + budget);
        slot.ticket = Some(ticket.clone());
        Ok(ticket)
    }
    fn matching(&self, ticket: &FrameSlotTicket, phase: SlotPhase) -> Result<usize> {
        let index = ticket.slot_index as usize;
        let slot = self
            .slots
            .get(index)
            .ok_or_else(|| error(ErrorCode::StaleEpoch, "unknown slot"))?;
        if slot.phase != phase || slot.ticket.as_ref() != Some(ticket) {
            return Err(error(
                ErrorCode::StaleEpoch,
                "slot ticket/phase stale or changed",
            ));
        }
        Ok(index)
    }
    pub fn commit(&mut self, producer: &str, commit: FrameSlotCommit, now: Instant) -> Result<()> {
        self.poll(now)?;
        if producer != self.spec.producer_ref {
            return Err(error(ErrorCode::PermissionDenied, "not pool producer"));
        }
        let ticket = commit
            .ticket
            .as_ref()
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "commit missing ticket"))?;
        let index = self.matching(ticket, SlotPhase::Writing)?;
        if commit.byte_length != validate_ticket(ticket)?
            || commit.sha256.len() != 64
            || !commit
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "commit length/hash rejected",
            ));
        }
        self.slots[index].phase = SlotPhase::Ready;
        self.slots[index].commit = Some(commit);
        Ok(())
    }
    pub fn acquire(
        &mut self,
        consumer: &str,
        ticket: &FrameSlotTicket,
        now: Instant,
    ) -> Result<FrameSlotCommit> {
        self.poll(now)?;
        if consumer != self.spec.consumer_ref {
            return Err(error(ErrorCode::PermissionDenied, "not pool consumer"));
        }
        let index = self.matching(ticket, SlotPhase::Ready)?;
        self.slots[index].phase = SlotPhase::Reading;
        Ok(self.slots[index].commit.as_ref().unwrap().clone())
    }
    pub fn validate_read(
        &mut self,
        consumer: &str,
        ticket: &FrameSlotTicket,
        now: Instant,
    ) -> Result<()> {
        self.poll(now)?;
        if consumer != self.spec.consumer_ref {
            return Err(error(ErrorCode::PermissionDenied, "not pool consumer"));
        }
        self.matching(ticket, SlotPhase::Reading).map(|_| ())
    }
    pub fn release(
        &mut self,
        consumer: &str,
        ticket: &FrameSlotTicket,
        now: Instant,
    ) -> Result<()> {
        self.validate_read(consumer, ticket, now)?;
        let slot = &mut self.slots[ticket.slot_index as usize];
        slot.phase = SlotPhase::Free;
        slot.ticket = None;
        slot.commit = None;
        slot.deadline = None;
        Ok(())
    }
}

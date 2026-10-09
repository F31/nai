//! Consumer-side frame view and consumption. Read-only: it validates lease
//! gating, epoch and integrity and reports an explicit standby reason otherwise.
//! It never produces a PlaybackAck and never grants device output.
use super::shared_ram::OwnedSlotFrame;
use super::{PoolSpec, Result};
use nai_contracts::v1::{ErrorCode, FrameSlotEvent, frame_slot_event::Event};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandbyReason {
    NoFrame,
    LeaseLost,
    StaleEpoch,
    Retired,
    IntegrityFailure,
    Timeout,
    NotReady,
}
impl StandbyReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            StandbyReason::NoFrame => "no_frame",
            StandbyReason::LeaseLost => "lease_lost",
            StandbyReason::StaleEpoch => "stale_epoch",
            StandbyReason::Retired => "retired",
            StandbyReason::IntegrityFailure => "integrity_failure",
            StandbyReason::Timeout => "timeout",
            StandbyReason::NotReady => "not_ready",
        }
    }
}

pub enum ConsumeOutcome {
    Delivered(Box<OwnedSlotFrame>),
    Standby(StandbyReason),
}
impl ConsumeOutcome {
    pub fn delivered(&self) -> Option<&OwnedSlotFrame> {
        match self {
            ConsumeOutcome::Delivered(frame) => Some(frame),
            ConsumeOutcome::Standby(_) => None,
        }
    }
    pub fn standby(&self) -> Option<StandbyReason> {
        match self {
            ConsumeOutcome::Standby(reason) => Some(*reason),
            ConsumeOutcome::Delivered(_) => None,
        }
    }
}

/// Consumer-side read-only view over an inherited pool file. `open` revalidates
/// the private bounded tmpfs object; no path arrives over the control bus.
pub struct FrameReader {
    spec: PoolSpec,
    #[cfg(target_os = "linux")]
    file: std::fs::File,
}
impl FrameReader {
    pub fn open(file: std::fs::File, spec: PoolSpec) -> Result<Self> {
        spec.validate()?;
        #[cfg(target_os = "linux")]
        {
            super::shared_ram::validate_file(&file)?;
            Ok(Self { spec, file })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = file;
            Err(super::error(
                ErrorCode::CapabilityNotReady,
                "consumer frame reader is Linux-only",
            ))
        }
    }
    pub fn spec(&self) -> &PoolSpec {
        &self.spec
    }
}

pub struct FrameConsumer {
    session_id: String,
    min_epoch: u64,
    last_delivered_epoch: u64,
}
impl FrameConsumer {
    pub fn new(session_id: &str, min_epoch: u64) -> Self {
        Self {
            session_id: session_id.into(),
            min_epoch,
            last_delivered_epoch: 0,
        }
    }
    pub fn last_delivered_epoch(&self) -> u64 {
        self.last_delivered_epoch
    }
    /// A false `lease_valid` never touches the payload. Epoch, pool identity and
    /// integrity are rechecked; success returns owned pixels and records the
    /// delivered epoch. It still grants no output and emits no PlaybackAck.
    pub fn submit(
        &mut self,
        event: FrameSlotEvent,
        lease_valid: bool,
        reader: &FrameReader,
        now: Instant,
    ) -> ConsumeOutcome {
        if !lease_valid {
            return ConsumeOutcome::Standby(StandbyReason::LeaseLost);
        }
        let Some(event) = event.event else {
            return ConsumeOutcome::Standby(StandbyReason::NoFrame);
        };
        let commit = match event {
            Event::WriteStarted(_) => return ConsumeOutcome::Standby(StandbyReason::NoFrame),
            Event::Error(_) => return ConsumeOutcome::Standby(StandbyReason::Retired),
            Event::Committed(commit) => *commit,
        };
        let Some(ticket) = commit.ticket.clone() else {
            return ConsumeOutcome::Standby(StandbyReason::IntegrityFailure);
        };
        if ticket.session_id != self.session_id {
            return ConsumeOutcome::Standby(StandbyReason::StaleEpoch);
        }
        if super::validate_ticket(&ticket).is_err() || ticket.pool_id != reader.spec.pool_id {
            return ConsumeOutcome::Standby(StandbyReason::IntegrityFailure);
        }
        if ticket.generation_epoch <= self.min_epoch
            || (self.last_delivered_epoch != 0
                && ticket.generation_epoch <= self.last_delivered_epoch)
        {
            return ConsumeOutcome::Standby(StandbyReason::StaleEpoch);
        }
        #[cfg(target_os = "linux")]
        {
            let Ok(budget) = super::IoBudget::starting_at(now, super::MAX_BUDGET) else {
                return ConsumeOutcome::Standby(StandbyReason::Timeout);
            };
            match super::shared_ram::read_committed_frame(
                &reader.file,
                &reader.spec,
                &reader.spec.consumer_ref,
                &ticket,
                &commit,
                &budget,
            ) {
                Ok(rgba) => {
                    self.last_delivered_epoch = ticket.generation_epoch;
                    ConsumeOutcome::Delivered(Box::new(OwnedSlotFrame {
                        ticket: ticket.clone(),
                        metadata: ticket.frame.as_ref().unwrap().clone(),
                        rgba,
                    }))
                }
                Err(error) => ConsumeOutcome::Standby(match ErrorCode::try_from(error.code) {
                    Ok(ErrorCode::DeadlineExceeded | ErrorCode::Cancelled) => {
                        StandbyReason::Timeout
                    }
                    Ok(ErrorCode::CapabilityNotReady) => StandbyReason::NotReady,
                    _ => StandbyReason::IntegrityFailure,
                }),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = now;
            ConsumeOutcome::Standby(StandbyReason::NotReady)
        }
    }
}

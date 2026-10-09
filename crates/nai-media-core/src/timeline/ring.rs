//! Bounded PCM ring buffering. Overflow is a capacity error, never silent loss
//! or truncation; readiness is expressed in bytes for water-level checks.
use crate::MediaError;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Slot {
    pub utterance_id: String,
    pub seq: u32,
    pub bytes: Vec<u8>,
}

pub struct PcmRing {
    slots: VecDeque<Slot>,
    bytes: usize,
    max_slots: usize,
    max_bytes: usize,
}
impl PcmRing {
    pub fn new(max_slots: usize, max_bytes: usize) -> Self {
        Self {
            slots: VecDeque::new(),
            bytes: 0,
            max_slots: max_slots.max(1),
            max_bytes: max_bytes.max(1),
        }
    }
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn capacity(&self) -> (usize, usize) {
        (self.max_slots, self.max_bytes)
    }
    /// Push a slot; Capacity on overrun, Stale on empty metadata.
    pub fn push(&mut self, slot: Slot) -> Result<(), MediaError> {
        if slot.utterance_id.is_empty() || slot.bytes.is_empty() {
            return Err(MediaError::Stale);
        }
        if self.slots.len() >= self.max_slots
            || self.bytes.saturating_add(slot.bytes.len()) > self.max_bytes
        {
            return Err(MediaError::Capacity);
        }
        self.bytes = self.bytes.saturating_add(slot.bytes.len());
        self.slots.push_back(slot);
        Ok(())
    }
    pub fn pop(&mut self) -> Option<Slot> {
        let slot = self.slots.pop_front()?;
        self.bytes = self.bytes.saturating_sub(slot.bytes.len());
        Some(slot)
    }
    /// Drain a fixed number of bytes in FIFO order into one buffer slice.
    pub fn read_up_to(&mut self, bytes: usize) -> Vec<u8> {
        let mut out = Vec::new();
        while !self.slots.is_empty() && out.len() < bytes {
            let slot = self.slots.front().unwrap();
            let take = (bytes - out.len()).min(slot.bytes.len());
            out.extend_from_slice(&slot.bytes[..take]);
            let mut slot = self.slots.pop_front().unwrap();
            if take < slot.bytes.len() {
                let rest = slot.bytes.split_off(take);
                slot.bytes = rest;
                self.slots.push_front(slot);
            }
            self.bytes = self.bytes.saturating_sub(take);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, seq: u32, len: usize) -> Slot {
        Slot {
            utterance_id: id.into(),
            seq,
            bytes: vec![0u8; len],
        }
    }

    #[test]
    fn overflow_is_capacity_not_silent_loss() {
        let mut ring = PcmRing::new(2, 1024);
        assert_eq!(ring.push(slot("u", 0, 512)), Ok(()));
        assert_eq!(ring.push(slot("u", 1, 512)), Ok(()));
        assert_eq!(ring.push(slot("u", 2, 1)), Err(MediaError::Capacity));
        assert_eq!(ring.pop().unwrap().seq, 0);
        assert_eq!(ring.push(slot("u", 2, 1)), Ok(()));
    }

    #[test]
    fn byte_bound_and_fifo_read_are_respected() {
        let mut ring = PcmRing::new(8, 100);
        assert_eq!(ring.push(slot("u", 0, 60)), Ok(()));
        assert_eq!(ring.push(slot("u", 1, 60)), Err(MediaError::Capacity));
        let drained = ring.read_up_to(30);
        assert_eq!(drained.len(), 30);
        assert_eq!(ring.push(slot("u", 1, 60)), Ok(()));
        assert_eq!(ring.read_up_to(200), vec![0u8; 90]);
        assert!(ring.is_empty());
    }
}

//! Lossless sentence planning and a bounded prefetch mechanism, without IO or authority grants.
use crate::foundation::Result;
use nai_contracts::workbench::WorkbenchError as E;

pub const MAX_SENTENCE_CHARS: usize = 120;
pub const MAX_SENTENCES: usize = 128;
pub const PREFETCH_LIMIT: usize = 2;
pub const MAX_PLAN_BYTES: usize = 32768;

/// Preserve every character, including whitespace. Oversized plans are rejected, never truncated.
pub fn sentences(text: &str) -> Result<Vec<String>> {
    if text.is_empty()
        || text.len() > MAX_PLAN_BYTES
        || text.chars().count() > MAX_SENTENCE_CHARS * MAX_SENTENCES
    {
        return Err(E::ResourceExhausted);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let limit = (start + MAX_SENTENCE_CHARS).min(chars.len());
        let mut end = None;
        for i in start..limit {
            let c = chars[i];
            let natural = matches!(c, '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n')
                || (c == '.' && chars.get(i + 1).is_none_or(|n| n.is_whitespace()));
            if natural {
                end = Some(i + 1);
                break;
            }
        }
        let end = end.unwrap_or_else(|| {
            if limit == chars.len() {
                limit
            } else {
                (start..limit)
                    .rev()
                    .find(|&i| chars[i].is_whitespace() || matches!(chars[i], ',' | '，' | '、'))
                    .map_or(limit, |i| i + 1)
            }
        });
        out.push(chars[start..end].iter().collect());
        if out.len() > MAX_SENTENCES {
            return Err(E::ResourceExhausted);
        }
        start = end;
    }
    // Whitespace-only fragments are retained and attached, never sent alone to TTS.
    let mut merged: Vec<String> = Vec::new();
    let mut pending = String::new();
    for part in out {
        if part.trim().is_empty() {
            pending.push_str(&part);
        } else {
            let combined = format!("{pending}{part}");
            if combined.chars().count() > MAX_SENTENCE_CHARS {
                return Err(E::ResourceExhausted);
            }
            merged.push(combined);
            pending.clear();
        }
    }
    if !pending.is_empty() {
        let last = merged.last_mut().ok_or(E::InvalidArgument)?;
        last.push_str(&pending);
        if last.chars().count() > MAX_SENTENCE_CHARS {
            return Err(E::ResourceExhausted);
        }
    }
    Ok(merged)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub generation: u64,
    pub attempt: u64,
    pub index: usize,
    pub deadline_ms: u64,
}
#[derive(Debug)]
enum Slot<T> {
    Pending(Ticket),
    Ready(Ticket, T),
    Taken(Ticket),
}
impl<T> Slot<T> {
    fn ticket(&self) -> Ticket {
        match self {
            Self::Pending(t) | Self::Ready(t, _) | Self::Taken(t) => *t,
        }
    }
}

/// Mechanism only: the caller must bind one instance to a validated source/session/epoch.
/// Payloads are not playback permits. Only trusted submission confirmation advances the cursor.
pub struct Prefetch<T> {
    generation: u64,
    attempt: u64,
    cursor: usize,
    total: usize,
    paused: bool,
    uncertain: bool,
    slots: Vec<Slot<T>>,
}
impl<T> Prefetch<T> {
    pub fn new(total: usize, generation: u64) -> Result<Self> {
        if total == 0 || total > MAX_SENTENCES || generation == 0 {
            return Err(E::InvalidArgument);
        }
        Ok(Self {
            generation,
            attempt: 0,
            cursor: 0,
            total,
            paused: false,
            uncertain: false,
            slots: Vec::new(),
        })
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn occupied(&self) -> usize {
        self.slots.len()
    }
    pub fn uncertain(&self) -> bool {
        self.uncertain
    }
    /// A handed-off current sentence cannot be replayed after cancellation without an explicit decision.
    /// This records a skip, never a successful submission; the caller must audit it separately.
    pub fn skip_uncertain(&mut self) -> Result<()> {
        if !self.uncertain || !self.slots.is_empty() || self.cursor >= self.total {
            return Err(E::RevisionConflict);
        }
        self.cursor += 1;
        self.uncertain = false;
        self.paused = true;
        Ok(())
    }
    pub fn pause(&mut self) {
        self.paused = true;
    }
    pub fn resume(&mut self) {
        self.paused = false;
    }
    pub fn reserve(&mut self, index: usize, now: u64, ttl: u64) -> Result<Ticket> {
        if ttl == 0 || ttl > 45000 || now.checked_add(ttl).is_none() {
            return Err(E::InvalidArgument);
        }
        if self.paused
            || self.uncertain
            || index < self.cursor
            || index >= self.total
            || index >= self.cursor + PREFETCH_LIMIT
        {
            return Err(E::RevisionConflict);
        }
        if self.slots.len() == PREFETCH_LIMIT
            || self.slots.iter().any(|s| s.ticket().index == index)
        {
            return Err(E::ResourceExhausted);
        }
        self.attempt = self.attempt.checked_add(1).ok_or(E::ResourceExhausted)?;
        let t = Ticket {
            generation: self.generation,
            attempt: self.attempt,
            index,
            deadline_ms: now + ttl,
        };
        self.slots.push(Slot::Pending(t));
        Ok(t)
    }
    pub fn complete(&mut self, ticket: Ticket, now: u64, value: T) -> Result<()> {
        if ticket.generation != self.generation || now >= ticket.deadline_ms {
            return Err(E::Cancelled);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|s| s.ticket() == ticket)
            .ok_or(E::Cancelled)?;
        if !matches!(slot, Slot::Pending(_)) {
            return Err(E::RevisionConflict);
        }
        *slot = Slot::Ready(ticket, value);
        Ok(())
    }
    pub fn take_current(&mut self, now: u64) -> Result<(Ticket, T)> {
        if self.paused || self.uncertain {
            return Err(E::Cancelled);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|s| s.ticket().index == self.cursor)
            .ok_or(E::CapabilityUnavailable)?;
        let t = slot.ticket();
        if now >= t.deadline_ms {
            return Err(E::Cancelled);
        }
        if !matches!(slot, Slot::Ready(_, _)) {
            return Err(E::CapabilityUnavailable);
        }
        let Slot::Ready(t, v) = std::mem::replace(slot, Slot::Taken(t)) else {
            unreachable!()
        };
        Ok((t, v))
    }
    pub fn submitted(&mut self, ticket: Ticket) -> Result<()> {
        let pos = self
            .slots
            .iter()
            .position(|s| matches!(s, Slot::Taken(t) if *t == ticket))
            .ok_or(E::RevisionConflict)?;
        if ticket.generation != self.generation || ticket.index != self.cursor {
            return Err(E::Cancelled);
        }
        self.slots.remove(pos);
        self.cursor += 1;
        Ok(())
    }
    /// Drop an unconsumed failed/expired reservation. Taken media needs explicit cancellation.
    pub fn release(&mut self, ticket: Ticket) -> Result<()> {
        let pos = self
            .slots
            .iter()
            .position(|s| s.ticket() == ticket)
            .ok_or(E::Cancelled)?;
        if matches!(self.slots[pos], Slot::Taken(_)) {
            return Err(E::RevisionConflict);
        }
        self.slots.remove(pos);
        Ok(())
    }
    pub fn cancel(&mut self) -> Result<()> {
        let next = self.generation.checked_add(1).ok_or(E::ResourceExhausted)?;
        self.uncertain |= self.slots.iter().any(|s| matches!(s, Slot::Taken(_)));
        self.slots.clear();
        self.generation = next;
        self.paused = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lossless_decimal_unicode_and_long_text() {
        let cases = [
            "容量500.5毫升。\n重量1.25千克！",
            "参考 https://example.cn/a.b 数值3.14。",
            "你好！  世界？\n",
            &"😀测".repeat(180),
            &"abc ，文字 ".repeat(80),
        ];
        for text in cases {
            let s = sentences(text).unwrap();
            assert_eq!(s.concat(), text);
            assert!(
                s.iter()
                    .all(|p| !p.trim().is_empty() && p.chars().count() <= 120 && p.len() <= 1024)
            );
        }
        assert_eq!(
            sentences("数值3.14。结束。").unwrap(),
            ["数值3.14。", "结束。"]
        );
    }
    #[test]
    fn bounds_are_not_silent_truncation() {
        assert!(sentences(&"字".repeat(120 * 128 + 1)).is_err());
        assert!(sentences(&"短。".repeat(129)).is_err());
        assert!(sentences(" \n ").is_err());
        assert_eq!(sentences(&"a".repeat(120 * 128)).unwrap().len(), 128);
    }
    #[test]
    fn next_ready_never_overtakes_current_and_submit_is_explicit() {
        let mut q = Prefetch::new(3, 1).unwrap();
        let a = q.reserve(0, 10, 100).unwrap();
        let b = q.reserve(1, 10, 100).unwrap();
        assert!(q.reserve(2, 10, 100).is_err());
        q.complete(b, 20, "second").unwrap();
        assert!(q.take_current(20).is_err());
        q.complete(a, 21, "first").unwrap();
        assert_eq!(q.take_current(22).unwrap(), (a, "first"));
        assert!(q.take_current(23).is_err());
        assert!(q.submitted(b).is_err());
        assert_eq!(q.cursor(), 0);
        q.submitted(a).unwrap();
        assert_eq!(q.take_current(24).unwrap(), (b, "second"));
        assert!(q.submitted(a).is_err());
        q.submitted(b).unwrap();
        assert_eq!(q.cursor(), 2);
        assert_eq!(q.occupied(), 0);
    }
    #[test]
    fn pause_expiry_retry_and_cancel_fence_late_work() {
        let mut q = Prefetch::new(2, 7).unwrap();
        let a = q.reserve(0, 0, 10).unwrap();
        q.pause();
        assert!(q.reserve(1, 1, 10).is_err());
        q.complete(a, 2, 1).unwrap();
        assert!(q.take_current(3).is_err());
        q.resume();
        assert!(q.take_current(10).is_err());
        q.release(a).unwrap();
        let b = q.reserve(0, 11, 10).unwrap();
        assert!(q.complete(a, 3, 9).is_err());
        q.cancel().unwrap();
        q.resume();
        let c = q.reserve(0, 12, 10).unwrap();
        assert_ne!(b.generation, c.generation);
        assert!(q.complete(b, 13, 8).is_err());
        q.complete(c, 14, 2).unwrap();
        assert_eq!(q.take_current(15).unwrap().1, 2);
        assert!(q.release(c).is_err());
        assert_eq!(q.cursor(), 0);
    }
    #[test]
    fn cancelled_handoff_cannot_resume_into_implicit_replay() {
        let mut q = Prefetch::new(2, 1).unwrap();
        let a = q.reserve(0, 0, 100).unwrap();
        q.complete(a, 1, "media").unwrap();
        q.take_current(2).unwrap();
        q.cancel().unwrap();
        q.resume();
        assert!(q.uncertain());
        assert!(q.reserve(0, 3, 100).is_err());
        assert!(q.submitted(a).is_err());
        q.skip_uncertain().unwrap();
        assert_eq!(q.cursor(), 1);
        assert!(!q.uncertain());
        assert!(q.reserve(1, 3, 100).is_err());
        q.resume();
        q.reserve(1, 3, 100).unwrap();
        assert!(q.skip_uncertain().is_err());
    }
}

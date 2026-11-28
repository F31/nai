//! Process-local arbitration only. Tokens are neither IPC authority nor device leases.
use std::sync::{
    Arc, Weak,
    atomic::{AtomicU64, Ordering},
};

const OWNER_MASK: u64 = 3;
const STOPPED: u64 = 4;
const GENERATION_STEP: u64 = 8;

#[derive(Clone, Copy)]
pub enum Speaker {
    Narration = 1,
    Answer = 2,
    Human = 3,
}

/// A token must only be checked against the exact gate instance that issued it.
#[derive(Clone, Copy)]
pub struct Token<'a>(u64, &'a AtomicU64);

#[derive(Default)]
pub struct OutputOwner {
    state: AtomicU64,
}

/// Owned receipt without extending the lifetime of its control root.
#[derive(Clone)]
pub struct OutputLease {
    value: u64,
    owner: Weak<OutputOwner>,
}

impl OutputLease {
    pub fn current(&self) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|owner| owner.state.load(Ordering::Acquire) == self.value)
    }
    /// Explicit release; dropping a clone must not release another user's output.
    pub fn release(&self) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|owner| owner.release(Token(self.value, &owner.state)))
    }
}

impl OutputOwner {
    pub fn acquire_lease(self: &Arc<Self>, speaker: Speaker) -> Option<OutputLease> {
        Some(OutputLease {
            value: self.acquire(speaker)?.0,
            owner: Arc::downgrade(self),
        })
    }
    /// Scope-bound nonblocking predicate for media callbacks. Dropping the root fails closed.
    pub fn acquire_callback(
        self: &Arc<Self>,
        speaker: Speaker,
    ) -> Option<impl Fn() -> bool + Send + Sync + 'static> {
        let lease = self.acquire_lease(speaker)?;
        Some(move || lease.current())
    }
    /// Trusted control-plane call. A same-priority request explicitly replaces its predecessor.
    pub fn acquire(&self, speaker: Speaker) -> Option<Token<'_>> {
        let priority = speaker as u64;
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                if state & STOPPED != 0 || state & OWNER_MASK > priority {
                    return None;
                }
                let generation = (state & !7).checked_add(GENERATION_STEP)?;
                Some(generation | priority)
            })
            .ok()
            .map(|old| Token(((old & !7) + GENERATION_STEP) | priority, &self.state))
    }
    pub fn current(&self, token: Token<'_>) -> bool {
        std::ptr::eq(&self.state, token.1) && self.state.load(Ordering::Acquire) == token.0
    }
    /// A late finally/release from an old task must never clear the new owner.
    pub fn release(&self, token: Token<'_>) -> bool {
        if !std::ptr::eq(&self.state, token.1) {
            return false;
        }
        let Some(next) = (token.0 & !7).checked_add(GENERATION_STEP) else {
            self.stop();
            return false;
        };
        self.state
            .compare_exchange(token.0, next, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    /// Terminal for this scope; resumption requires a new explicitly created scope.
    pub fn stop(&self) {
        self.state.fetch_or(STOPPED, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_result_cannot_pass_after_takeover() {
        let root = Arc::new(OutputOwner::default());
        let permitted = root.acquire_callback(Speaker::Answer).unwrap();
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let task = std::thread::spawn(move || {
            release_rx.recv().unwrap();
            permitted()
        });
        let _human = root.acquire(Speaker::Human).unwrap();
        release_tx.send(()).unwrap();
        assert!(!task.join().unwrap());
    }
    #[test]
    fn callback_cancels_on_takeover_and_root_drop() {
        let root = Arc::new(OutputOwner::default());
        let old = root.acquire_callback(Speaker::Narration).unwrap();
        assert!(old());
        let human = root.acquire_callback(Speaker::Human).unwrap();
        assert!(!old());
        assert!(human());
        drop(root);
        assert!(!human());
    }
    #[test]
    fn token_cannot_cross_scope_even_with_equal_generations() {
        let first = OutputOwner::default();
        let second = OutputOwner::default();
        let a = first.acquire(Speaker::Answer).unwrap();
        let b = second.acquire(Speaker::Answer).unwrap();
        assert!(!second.current(a));
        assert!(!second.release(a));
        assert!(second.current(b));
    }
    #[test]
    fn stop_from_another_thread_invalidates_callback_token() {
        let gate = OutputOwner::default();
        let token = gate.acquire(Speaker::Narration).unwrap();
        std::thread::scope(|s| {
            s.spawn(|| gate.stop()).join().unwrap();
        });
        assert!(!gate.current(token));
        assert!(gate.acquire(Speaker::Answer).is_none());
    }
    #[test]
    fn takeover_invalidates_old_task_and_its_finally() {
        let gate = OutputOwner::default();
        let old = gate.acquire(Speaker::Narration).unwrap();
        let answer = gate.acquire(Speaker::Answer).unwrap();
        assert!(!gate.current(old));
        assert!(!gate.release(old));
        assert!(gate.current(answer));
        let human = gate.acquire(Speaker::Human).unwrap();
        assert!(!gate.current(answer));
        assert!(gate.acquire(Speaker::Answer).is_none());
        assert!(gate.release(human));
        let resumed = gate.acquire(Speaker::Narration).unwrap();
        assert!(gate.current(resumed));
        assert!(!gate.current(old));
    }
    #[test]
    fn stop_is_terminal_and_stale_release_cannot_reopen() {
        let gate = OutputOwner::default();
        let token = gate.acquire(Speaker::Human).unwrap();
        gate.stop();
        assert!(!gate.current(token));
        assert!(!gate.release(token));
        assert!(gate.acquire(Speaker::Human).is_none());
    }
    #[test]
    fn newer_answer_invalidates_previous_answer() {
        let gate = OutputOwner::default();
        let old = gate.acquire(Speaker::Answer).unwrap();
        let new = gate.acquire(Speaker::Answer).unwrap();
        assert!(!gate.current(old));
        assert!(gate.current(new));
    }
}

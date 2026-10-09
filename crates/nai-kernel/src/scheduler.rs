//! Bounded per-session scheduling; P0 deliberately has no ordinary queue slot.
use nai_contracts::v1::{EventKind, PlatformEvent, QuantitySemantics};
use prost::Message;
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleError {
    Invalid,
    Scope,
    Expired,
    Capacity,
    Debt,
    Admission,
    Conflict,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingest {
    Queued,
    Merged,
    Duplicate,
}
#[derive(Default, Debug)]
pub struct Metrics {
    pub overflow: u64,
    pub expired: u64,
    pub duplicate: u64,
    pub merged: u64,
    pub stale: u64,
    pub debt_blocked: u64,
    pub admission_denied: u64,
    pub dropped_likes: u64,
}
pub struct EventIngress {
    session: String,
    capacity: usize,
    state_capacity: usize,
    dedup_capacity: usize,
    normal: VecDeque<PlatformEvent>,
    states: BTreeMap<String, PlatformEvent>,
    seen: BTreeMap<String, u64>,
    pub metrics: Metrics,
}
fn event_scope(e: &PlatformEvent) -> String {
    format!(
        "{:?}",
        (
            &e.platform,
            &e.app_scope,
            &e.room_id,
            &e.room_source_ref,
            e.source,
            e.generation_epoch,
            e.connection_generation
        )
    )
}
fn critical(kind: i32) -> bool {
    matches!(
        EventKind::try_from(kind),
        Ok(EventKind::RoomEnded
            | EventKind::RoomStarted
            | EventKind::PermissionChanged
            | EventKind::ConnectionChanged)
    )
}
impl EventIngress {
    pub fn new(
        session: &str,
        capacity: usize,
        state_capacity: usize,
        dedup_capacity: usize,
    ) -> Result<Self, ScheduleError> {
        if session.is_empty()
            || capacity == 0
            || capacity > 4096
            || state_capacity == 0
            || state_capacity > 256
            || dedup_capacity == 0
            || dedup_capacity > 65536
        {
            return Err(ScheduleError::Invalid);
        }
        Ok(Self {
            session: session.into(),
            capacity,
            state_capacity,
            dedup_capacity,
            normal: VecDeque::new(),
            states: BTreeMap::new(),
            seen: BTreeMap::new(),
            metrics: Metrics::default(),
        })
    }
    fn expire(&mut self, now: u64) {
        self.seen.retain(|_, expiry| *expiry > now);
        let before = self.normal.len() + self.states.len();
        self.normal.retain(|e| e.expires_at_utc_ms > now);
        self.states.retain(|_, e| e.expires_at_utc_ms > now);
        self.metrics.expired = self
            .metrics
            .expired
            .saturating_add((before - self.normal.len() - self.states.len()) as u64);
    }
    pub fn push(&mut self, e: PlatformEvent, now: u64) -> Result<Ingest, ScheduleError> {
        self.expire(now);
        if e.session_id != self.session {
            return Err(ScheduleError::Scope);
        }
        if e.event_id.is_empty()
            || e.event_id.len() > 256
            || e.encoded_len() > 65536
            || EventKind::try_from(e.kind).is_err()
        {
            return Err(ScheduleError::Invalid);
        }
        if e.expires_at_utc_ms <= now {
            self.metrics.expired = self.metrics.expired.saturating_add(1);
            return Err(ScheduleError::Expired);
        }
        let scope = event_scope(&e);
        let key = format!("{scope}:{:?}", e.event_id);
        if critical(e.kind) {
            let state_key = format!("{scope}:{}", e.kind);
            if self
                .states
                .get(&state_key)
                .is_some_and(|old| old.event_id == e.event_id)
            {
                self.metrics.duplicate = self.metrics.duplicate.saturating_add(1);
                return Ok(Ingest::Duplicate);
            }
            if !self.states.contains_key(&state_key) && self.states.len() >= self.state_capacity {
                self.metrics.overflow = self.metrics.overflow.saturating_add(1);
                return Err(ScheduleError::Capacity);
            }
            let merged = self.states.insert(state_key, e).is_some();
            if merged {
                self.metrics.merged = self.metrics.merged.saturating_add(1);
            }
            return Ok(if merged {
                Ingest::Merged
            } else {
                Ingest::Queued
            });
        }
        if self.seen.contains_key(&key) {
            self.metrics.duplicate = self.metrics.duplicate.saturating_add(1);
            return Ok(Ingest::Duplicate);
        }
        if self.seen.len() >= self.dedup_capacity {
            self.metrics.overflow = self.metrics.overflow.saturating_add(1);
            return Err(ScheduleError::Capacity);
        }
        let merged = self.normal.iter_mut().find(|old| {
            old.kind == e.kind
                && event_scope(old) == scope
                && old.viewer_scoped_id == e.viewer_scoped_id
                && ((e.kind == EventKind::Like as i32)
                    || (e.kind == EventKind::Gift as i32
                        && !e.combo_id.is_empty()
                        && old.combo_id == e.combo_id))
                && old.text == e.text
                && old.quantity_semantics == e.quantity_semantics
                && matches!(
                    QuantitySemantics::try_from(e.quantity_semantics),
                    Ok(QuantitySemantics::Delta | QuantitySemantics::Cumulative)
                )
                && old.quantity.is_some()
                && e.quantity.is_some()
        });
        if let Some(old) = merged {
            let quantity = if e.quantity_semantics == QuantitySemantics::Delta as i32 {
                old.quantity
                    .unwrap()
                    .checked_add(e.quantity.unwrap())
                    .ok_or(ScheduleError::Invalid)?
            } else {
                old.quantity.unwrap().max(e.quantity.unwrap())
            };
            old.quantity = Some(quantity);
            old.expires_at_utc_ms = old.expires_at_utc_ms.min(e.expires_at_utc_ms);
            self.seen.insert(key, e.expires_at_utc_ms);
            self.metrics.merged = self.metrics.merged.saturating_add(1);
            return Ok(Ingest::Merged);
        }
        if self.normal.len() >= self.capacity
            && e.kind != EventKind::Like as i32
            && let Some(index) = self
                .normal
                .iter()
                .position(|old| old.kind == EventKind::Like as i32)
        {
            self.normal.remove(index);
            self.metrics.dropped_likes = self.metrics.dropped_likes.saturating_add(1);
            self.metrics.overflow = self.metrics.overflow.saturating_add(1);
        }
        if self.normal.len() >= self.capacity {
            self.metrics.overflow = self.metrics.overflow.saturating_add(1);
            return Err(ScheduleError::Capacity);
        }
        self.seen.insert(key, e.expires_at_utc_ms);
        self.normal.push_back(e);
        Ok(Ingest::Queued)
    }
    pub fn pop(&mut self, now: u64) -> Option<PlatformEvent> {
        self.expire(now);
        // Room end takes precedence, then the latest per-key state slots.
        let key = self
            .states
            .iter()
            .find(|(_, e)| e.kind == EventKind::RoomEnded as i32)
            .or_else(|| self.states.iter().next())
            .map(|(k, _)| k.clone());
        if let Some(key) = key {
            self.states.remove(&key)
        } else {
            self.normal.pop_front()
        }
    }
    pub fn len(&self) -> usize {
        self.normal.len() + self.states.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: String,
    pub session: String,
    pub epoch: u64,
    pub viewer: String,
    pub topic: String,
    pub priority: u8,
    pub speech_ms: u32,
    pub deadline: Instant,
}
#[derive(Clone, Debug)]
pub struct Policy {
    pub capacity: usize,
    pub max_speech_ms: u64,
    pub cooldown: Duration,
    pub fairness_window: usize,
    pub max_viewer_share_percent: u32,
    pub starvation: Duration,
}
pub struct Selection<R> {
    pub candidate: Candidate,
    pub reservation: R,
}
struct Waiting {
    candidate: Candidate,
    queued: Instant,
}
pub struct Scheduler {
    session: String,
    epoch: u64,
    policy: Policy,
    waiting: VecDeque<Waiting>,
    utterances: BTreeMap<String, Candidate>,
    history: VecDeque<String>,
    cooldowns: BTreeMap<String, Instant>,
    debt: u64,
    pub metrics: Metrics,
}
impl Scheduler {
    pub fn new(session: &str, epoch: u64, policy: Policy) -> Result<Self, ScheduleError> {
        if session.is_empty()
            || policy.capacity == 0
            || policy.capacity > 1024
            || policy.max_speech_ms == 0
            || policy.max_speech_ms > 600000
            || policy.fairness_window < 2
            || policy.fairness_window > 1024
            || !(1..=100).contains(&policy.max_viewer_share_percent)
            || policy.starvation.is_zero()
            || policy.starvation > Duration::from_secs(600)
            || policy.cooldown > Duration::from_secs(600)
        {
            return Err(ScheduleError::Invalid);
        }
        Ok(Self {
            session: session.into(),
            epoch,
            policy,
            waiting: VecDeque::new(),
            utterances: BTreeMap::new(),
            history: VecDeque::new(),
            cooldowns: BTreeMap::new(),
            debt: 0,
            metrics: Metrics::default(),
        })
    }
    pub fn debt_ms(&self) -> u64 {
        self.debt
    }
    pub fn pending(&self) -> usize {
        self.waiting.len()
    }
    fn expire(&mut self, now: Instant) {
        let before = self.waiting.len();
        self.waiting.retain(|w| w.candidate.deadline > now);
        self.metrics.expired = self
            .metrics
            .expired
            .saturating_add((before - self.waiting.len()) as u64);
        self.cooldowns.retain(|_, until| *until > now);
        // Outstanding debt is not released merely because generation timed out.
    }
    pub fn push(
        &mut self,
        candidate: Candidate,
        now: Instant,
    ) -> Result<Option<String>, ScheduleError> {
        self.expire(now);
        if candidate.session != self.session || candidate.epoch != self.epoch {
            self.metrics.stale = self.metrics.stale.saturating_add(1);
            return Err(ScheduleError::Scope);
        }
        if candidate.id.is_empty()
            || candidate.id.len() > 256
            || candidate.viewer.len() > 256
            || candidate.topic.len() > 256
            || !(1..=5).contains(&candidate.priority)
            || candidate.speech_ms == 0
        {
            return Err(ScheduleError::Invalid);
        }
        if candidate.deadline <= now {
            self.metrics.expired = self.metrics.expired.saturating_add(1);
            return Err(ScheduleError::Expired);
        }
        if self.utterances.contains_key(&candidate.id)
            || self.waiting.iter().any(|w| w.candidate.id == candidate.id)
        {
            return Err(ScheduleError::Conflict);
        }
        if let Some(old) = self.waiting.iter_mut().find(|w| {
            !candidate.topic.is_empty()
                && w.candidate.viewer == candidate.viewer
                && w.candidate.topic == candidate.topic
                && w.candidate.priority == candidate.priority
        }) {
            old.candidate.deadline = old.candidate.deadline.min(candidate.deadline);
            self.metrics.merged = self.metrics.merged.saturating_add(1);
            return Ok(Some(old.candidate.id.clone()));
        }
        if self.waiting.len() >= self.policy.capacity {
            self.metrics.overflow = self.metrics.overflow.saturating_add(1);
            return Err(ScheduleError::Capacity);
        }
        self.waiting.push_back(Waiting {
            candidate,
            queued: now,
        });
        Ok(None)
    }
    pub fn select<R>(
        &mut self,
        now: Instant,
        mut admit: impl FnMut(&Candidate) -> Result<R, ScheduleError>,
    ) -> Result<Option<Selection<R>>, ScheduleError> {
        self.expire(now);
        if self.utterances.len() >= self.policy.capacity {
            return Err(ScheduleError::Capacity);
        }
        let mut eligible = Vec::new();
        for (index, w) in self.waiting.iter().enumerate() {
            let c = &w.candidate;
            let debt = self.debt + u64::from(c.speech_ms);
            if debt > self.policy.max_speech_ms
                || now
                    .checked_add(Duration::from_millis(debt))
                    .is_none_or(|end| end >= c.deadline)
            {
                self.metrics.debt_blocked = self.metrics.debt_blocked.saturating_add(1);
                continue;
            }
            if c.priority >= 3
                && !c.viewer.is_empty()
                && (self.cooldowns.contains_key(&c.viewer)
                    || (!self.cooldowns.contains_key(&c.viewer)
                        && self.cooldowns.len()
                            >= self.policy.capacity + self.policy.fairness_window))
            {
                continue;
            }
            let priority = if c.priority >= 3
                && now.saturating_duration_since(w.queued) >= self.policy.starvation
            {
                2
            } else {
                c.priority
            };
            eligible.push((index, priority, w.queued));
        }
        eligible.sort_by_key(|(_, p, t)| (*p, *t));
        let winner = eligible
            .iter()
            .find(|(index, _, _)| {
                let c = &self.waiting[*index].candidate;
                if c.priority <= 2
                    || c.viewer.is_empty()
                    || self.history.len() < self.policy.fairness_window
                {
                    return true;
                }
                let count = self
                    .history
                    .iter()
                    .skip(1)
                    .filter(|v| **v == c.viewer)
                    .count()
                    + 1;
                let peer = eligible
                    .iter()
                    .any(|(i, _, _)| self.waiting[*i].candidate.viewer != c.viewer);
                !peer
                    || count * 100
                        <= self.policy.max_viewer_share_percent as usize
                            * self.policy.fairness_window
            })
            .map(|(index, _, _)| *index);
        let Some(index) = winner else {
            return Ok(None);
        };
        let receipt = match admit(&self.waiting[index].candidate) {
            Ok(r) => r,
            Err(e) => {
                self.metrics.admission_denied = self.metrics.admission_denied.saturating_add(1);
                return Err(e);
            }
        };
        let candidate = self.waiting.remove(index).unwrap().candidate;
        self.debt += u64::from(candidate.speech_ms);
        if candidate.priority >= 3 && !candidate.viewer.is_empty() {
            self.cooldowns
                .insert(candidate.viewer.clone(), now + self.policy.cooldown);
            self.history.push_back(candidate.viewer.clone());
            if self.history.len() > self.policy.fairness_window {
                self.history.pop_front();
            }
        }
        self.utterances
            .insert(candidate.id.clone(), candidate.clone());
        Ok(Some(Selection {
            candidate,
            reservation: receipt,
        }))
    }
    /// Actual end/cancellation confirmation releases this task's remaining debt.
    pub fn revise_speech(&mut self, id: &str, actual_ms: u32) -> Result<bool, ScheduleError> {
        if actual_ms == 0 {
            return Err(ScheduleError::Invalid);
        }
        let c = self.utterances.get_mut(id).ok_or(ScheduleError::Conflict)?;
        self.debt = self.debt - u64::from(c.speech_ms) + u64::from(actual_ms);
        c.speech_ms = actual_ms;
        Ok(self.debt > self.policy.max_speech_ms)
    }
    /// Actual end/cancellation confirmation releases this task's remaining debt.
    pub fn finish(&mut self, id: &str) -> Result<(), ScheduleError> {
        let c = self.utterances.remove(id).ok_or(ScheduleError::Conflict)?;
        self.debt -= u64::from(c.speech_ms);
        Ok(())
    }
    /// Called after D06 media mute/epoch revoke, not instead of that path.
    pub fn revoke(&mut self, new_epoch: u64) -> Result<Vec<String>, ScheduleError> {
        if new_epoch <= self.epoch {
            return Err(ScheduleError::Scope);
        }
        let cancelled = self.utterances.keys().cloned().collect();
        self.waiting.clear();
        self.utterances.clear();
        self.debt = 0;
        self.epoch = new_epoch;
        Ok(cancelled)
    }
}

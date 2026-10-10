//! Multi-session isolation for Pro: one namespace per session, a closed share
//! whitelist, and per-family admission that refuses a new session rather than
//! starving a live one.
//!
//! §12.3 states the three things this module has to make true at once: every
//! piece of state is isolated by `session_id` (each session gets its own epoch,
//! authorization, queue, budget, knowledge binding, approval and output id);
//! **sharing is limited to** immutable models, read-only knowledge snapshots and
//! a controlled connection pool; and GPU throughput, video memory, audio timing
//! and API quota are admitted **separately**, refusing a new session when the
//! machine is full rather than letting an on-air session OOM.
//!
//! The shape of the types is the guarantee. Cross-talk is not prevented by a
//! check that somebody could forget — a lane is only reachable through its
//! owning session, so a different requester is refused by construction. Sharing
//! is a closed enum of three kinds, and shared state has **no write path**, so
//! "read-only snapshot" is not a promise but an absent function. Admission is
//! two-phase — check every family, then commit — so a refusal leaves every live
//! session's usage exactly where it was.
use nai_contracts::v1::ErrorCode;
use std::collections::{BTreeMap, BTreeSet};

/// Upper bound on live sessions, so the registry cannot grow without bound.
pub const MAX_SESSIONS: usize = 64;
const MAX_ID: usize = 128;

/// A [`MultiplexError`] is a stable code plus a portable token. Tokens never
/// change with locale, so a report can classify a refusal without a debug build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiplexError {
    pub code: ErrorCode,
    pub reason: &'static str,
}

impl std::fmt::Display for MultiplexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "multiplex: {} ({})",
            self.code.as_str_name(),
            self.reason
        )
    }
}

impl std::error::Error for MultiplexError {}

fn error(code: ErrorCode, reason: &'static str) -> MultiplexError {
    MultiplexError { code, reason }
}

type Result<T> = std::result::Result<T, MultiplexError>;

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && !id.contains(char::is_whitespace)
}

/// The per-session lanes A11 enumerates: task, audio, approval, budget,
/// knowledge and output. Each belongs to exactly one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lane {
    Task,
    Audio,
    Approval,
    Budget,
    Knowledge,
    Output,
}

impl Lane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Audio => "audio",
            Self::Approval => "approval",
            Self::Budget => "budget",
            Self::Knowledge => "knowledge",
            Self::Output => "output",
        }
    }
}

/// All the lanes, for exhaustive iteration.
pub const LANES: &[Lane] = &[
    Lane::Task,
    Lane::Audio,
    Lane::Approval,
    Lane::Budget,
    Lane::Knowledge,
    Lane::Output,
];

/// One session's private namespace. Public so an audit can be a pure function
/// over plain values rather than over the registry's internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNamespace {
    pub session_id: String,
    /// Independent per session: advancing one never moves another.
    pub epoch: u64,
    pub lanes: BTreeSet<Lane>,
}

/// What one session gets when it reaches a lane it owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneGrant {
    pub owner: String,
    pub epoch: u64,
    pub lane: Lane,
}

/// The session registry. Every lane is keyed by its owning session, which is
/// what makes cross-talk unrepresentable rather than merely forbidden.
#[derive(Debug, Clone, Default)]
pub struct Multiplexer {
    namespaces: BTreeMap<String, SessionNamespace>,
}

impl Multiplexer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a session with its own epoch. A session id is a namespace key, so
    /// opening it twice is a conflict rather than a silent reuse.
    pub fn open(&mut self, session_id: &str, epoch: u64) -> Result<()> {
        if !valid_id(session_id) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid session reference",
            ));
        }
        if self.namespaces.contains_key(session_id) {
            return Err(error(ErrorCode::Conflict, "session already open"));
        }
        if self.namespaces.len() >= MAX_SESSIONS {
            return Err(error(ErrorCode::ResourceExhausted, "session limit"));
        }
        self.namespaces.insert(
            session_id.to_owned(),
            SessionNamespace {
                session_id: session_id.to_owned(),
                epoch,
                lanes: BTreeSet::new(),
            },
        );
        Ok(())
    }

    pub fn close(&mut self, session_id: &str) -> Result<()> {
        self.namespaces
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "session is not open"))
    }

    /// Advance one session's epoch. Only that session moves — a shared counter
    /// would let one session's restart invalidate another session's work.
    pub fn advance_epoch(&mut self, session_id: &str) -> Result<u64> {
        let namespace = self
            .namespaces
            .get_mut(session_id)
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "session is not open"))?;
        namespace.epoch = namespace
            .epoch
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "epoch exhausted"))?;
        Ok(namespace.epoch)
    }

    /// Bind a lane to the session that owns it.
    pub fn bind(&mut self, session_id: &str, lane: Lane) -> Result<()> {
        let namespace = self
            .namespaces
            .get_mut(session_id)
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "session is not open"))?;
        if !namespace.lanes.insert(lane) {
            return Err(error(ErrorCode::Conflict, "lane already bound"));
        }
        Ok(())
    }

    pub fn namespace(&self, session_id: &str) -> Option<&SessionNamespace> {
        self.namespaces.get(session_id)
    }

    pub fn sessions(&self) -> Vec<SessionNamespace> {
        self.namespaces.values().cloned().collect()
    }

    /// Reach a lane. The only route is through the owning session: a different
    /// requester is refused, which is "无串场" as a property of the signature
    /// rather than a rule someone has to remember to check.
    pub fn access(&self, requester: &str, owner: &str, lane: Lane) -> Result<LaneGrant> {
        if requester != owner {
            return Err(error(
                ErrorCode::Conflict,
                "lane belongs to another session",
            ));
        }
        let namespace = self
            .namespaces
            .get(owner)
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "session is not open"))?;
        if !namespace.lanes.contains(&lane) {
            return Err(error(ErrorCode::CapabilityAbsent, "lane is not bound"));
        }
        Ok(LaneGrant {
            owner: owner.to_owned(),
            epoch: namespace.epoch,
            lane,
        })
    }
}

/// The only three things sessions may share. Immutable models and read-only
/// knowledge snapshots cannot be corrupted by a second reader; a connection pool
/// is shared because it is **controlled** (bounded below).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SharedKind {
    ImmutableModel,
    KnowledgeSnapshot,
    ConnectionPool,
}

impl SharedKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImmutableModel => "immutable_model",
            Self::KnowledgeSnapshot => "knowledge_snapshot",
            Self::ConnectionPool => "connection_pool",
        }
    }
}

/// The three shareable kinds are enumerated; live lane state (a budget, an
/// approval, an output id) has no representation here at all, which is how
/// "共享只限…" is enforced — there is no function that turns a lane into a
/// shareable resource to call by mistake.
pub const SHAREABLE: &[SharedKind] = &[
    SharedKind::ImmutableModel,
    SharedKind::KnowledgeSnapshot,
    SharedKind::ConnectionPool,
];

/// A bounded set of shared leases: `(kind, key)` to the sessions holding it.
#[derive(Debug, Clone)]
pub struct SharedPool {
    max_holders: usize,
    leases: BTreeMap<(SharedKind, String), BTreeSet<String>>,
}

impl SharedPool {
    /// A holder limit of zero would make every pool unusable, so it is refused
    /// at construction rather than discovered on the first acquire.
    pub fn new(max_holders: usize) -> Result<Self> {
        if max_holders == 0 {
            return Err(error(ErrorCode::InvalidArgument, "holder limit is zero"));
        }
        Ok(Self {
            max_holders,
            leases: BTreeMap::new(),
        })
    }

    /// Take a shared lease. Sharing a resource is a **read**; the pool is
    /// controlled by its holder limit, so the refusal is a resource exhaustion,
    /// not a permission error.
    pub fn acquire(&mut self, session: &str, kind: SharedKind, key: &str) -> Result<()> {
        if !valid_id(session) || !valid_id(key) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid shared reference",
            ));
        }
        let holders = self.leases.entry((kind, key.to_owned())).or_default();
        if !holders.contains(session) && holders.len() >= self.max_holders {
            return Err(error(ErrorCode::ResourceExhausted, "shared holder limit"));
        }
        holders.insert(session.to_owned());
        Ok(())
    }

    pub fn release(&mut self, session: &str, kind: SharedKind, key: &str) -> Result<()> {
        let holders = self
            .leases
            .get_mut(&(kind, key.to_owned()))
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "shared resource is not held"))?;
        if !holders.remove(session) {
            return Err(error(
                ErrorCode::CapabilityAbsent,
                "session does not hold it",
            ));
        }
        Ok(())
    }

    pub fn holders(&self, kind: SharedKind, key: &str) -> usize {
        self.leases
            .get(&(kind, key.to_owned()))
            .map_or(0, BTreeSet::len)
    }

    /// The shared set is read-only, so there is deliberately **no** way to write
    /// through it. This method exists to make that explicit and testable: it
    /// always refuses, which is the difference between a read-only snapshot and
    /// a snapshot someone forgot to lock.
    pub fn write(&self, _session: &str, _kind: SharedKind, _key: &str) -> Result<()> {
        Err(error(
            ErrorCode::PermissionDenied,
            "shared resource is read-only",
        ))
    }

    pub fn snapshot(&self) -> Vec<(SharedKind, String, Vec<String>)> {
        self.leases
            .iter()
            .map(|((kind, key), holders)| (*kind, key.clone(), holders.iter().cloned().collect()))
            .collect()
    }
}

/// The four families §12.3 admits **separately**. A family is not a synonym for
/// another: video memory running out says nothing about API quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AdmissionFamily {
    GpuThroughput,
    Vram,
    AudioTiming,
    ApiQuota,
}

impl AdmissionFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GpuThroughput => "gpu_throughput",
            Self::Vram => "vram",
            Self::AudioTiming => "audio_timing",
            Self::ApiQuota => "api_quota",
        }
    }
}

/// One request for a family's capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub family: AdmissionFamily,
    pub amount: u64,
}

impl Request {
    pub fn new(family: AdmissionFamily, amount: u64) -> Self {
        Self { family, amount }
    }
}

/// Per-family capacity and per-session usage.
///
/// Admission is **two-phase**: every family is checked against the whole-machine
/// total before any usage is written. That is what keeps a refusal from half-
/// committing and starving the session that is already on air.
#[derive(Debug, Clone)]
pub struct Admission {
    limits: BTreeMap<AdmissionFamily, u64>,
    usage: BTreeMap<(String, AdmissionFamily), u64>,
}

impl Admission {
    /// Build a limit table. All four families should be present; a missing one
    /// admits nothing, which is the conservative reading of an unstated budget.
    pub fn new(limits: &[(AdmissionFamily, u64)]) -> Self {
        Self {
            limits: limits.iter().copied().collect(),
            usage: BTreeMap::new(),
        }
    }

    pub fn limit(&self, family: AdmissionFamily) -> u64 {
        self.limits.get(&family).copied().unwrap_or(0)
    }

    /// Total in use across every session, for one family.
    pub fn used_total(&self, family: AdmissionFamily) -> u64 {
        self.usage
            .iter()
            .filter(|((_, used_family), _)| *used_family == family)
            .fold(0u64, |sum, (_, amount)| sum.saturating_add(*amount))
    }

    pub fn used_by(&self, session: &str, family: AdmissionFamily) -> u64 {
        self.usage
            .get(&(session.to_owned(), family))
            .copied()
            .unwrap_or(0)
    }

    /// Admit a set of requests for one session.
    ///
    /// Check-then-commit: if **any** family would exceed its limit the request
    /// is refused with `RESOURCE_EXHAUSTED` and **no** usage is written, so an
    /// on-air session's numbers are exactly where they were. families are judged
    /// independently — headroom in one does not buy headroom in another.
    pub fn admit(&mut self, session: &str, requests: &[Request]) -> Result<()> {
        if !valid_id(session) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid session reference",
            ));
        }
        let mut wanted: BTreeMap<AdmissionFamily, u64> = BTreeMap::new();
        for request in requests {
            if request.amount == 0 {
                return Err(error(ErrorCode::InvalidArgument, "zero capacity request"));
            }
            let entry = wanted.entry(request.family).or_insert(0);
            *entry = entry.saturating_add(request.amount);
        }
        for (family, amount) in &wanted {
            if self.used_total(*family).saturating_add(*amount) > self.limit(*family) {
                return Err(error(
                    ErrorCode::ResourceExhausted,
                    "family capacity exceeded",
                ));
            }
        }
        for (family, amount) in wanted {
            let entry = self.usage.entry((session.to_owned(), family)).or_insert(0);
            *entry = entry.saturating_add(amount);
        }
        Ok(())
    }

    /// Release everything one session holds. Ending a session frees its share
    /// without touching any other session's numbers.
    pub fn release(&mut self, session: &str) {
        self.usage.retain(|(owner, _), _| owner != session);
    }
}

/// One isolation rule that is currently broken, as a stable token plus its
/// subject. Mirrors the residency audit's shape so reports stay uniform.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IsolationViolation {
    pub token: &'static str,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IsolationAudit {
    pub violations: Vec<IsolationViolation>,
}

impl IsolationAudit {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Audit isolation over plain values.
///
/// Two states are worth naming: a shared lease with no holder (a leak that no
/// behavioural test notices) and two namespaces claiming the same session id (a
/// merge that would silently let two sessions share one epoch). Both are
/// reachable only by a mistake, which is why the audit exists — it turns "the
/// registry cannot do that" into something a host can re-check after a refactor.
pub fn audit_isolation(namespaces: &[SessionNamespace], shared: &SharedPool) -> IsolationAudit {
    let mut violations = Vec::new();
    for (kind, key, holders) in shared.snapshot() {
        if holders.is_empty() {
            violations.push(IsolationViolation {
                token: "shared lease without a holder",
                subject: format!("{}:{key}", kind.as_str()),
            });
        }
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for namespace in namespaces {
        *seen.entry(namespace.session_id.as_str()).or_insert(0) += 1;
    }
    for (session_id, count) in seen {
        if count > 1 {
            violations.push(IsolationViolation {
                token: "two namespaces share a session id",
                subject: session_id.to_owned(),
            });
        }
    }
    violations.sort();
    violations.dedup();
    IsolationAudit { violations }
}

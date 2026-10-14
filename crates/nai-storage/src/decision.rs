//! Append-only hash-chained decision log (V3.6 §5.8, INV-19). Critical entries
//! persist or fail; telemetry may be lost and is buffered with a bound. The
//! playback ACK segment has no producer yet (no output-keeper), so entries of
//! that kind are only defined and stored, never fabricated.
use crate::{Operation, Result, Store, error, error_at, hash, id, utc_ms};
use nai_contracts::v1::ErrorCode;
use rusqlite::{OptionalExtension, TransactionBehavior, params};

pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const TELEMETRY_BUFFER_CAPACITY: usize = 1024;
const MAX_PAYLOAD_BYTES: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Criticality {
    Critical,
    Telemetry,
}
impl Criticality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Telemetry => "telemetry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    Event,
    Observation,
    /// §41.5: a context assembly, logged as its own ingestible fragments (W104).
    Assembly,
    Decision,
    Guard,
    Cue,
    Approval,
    Synthesis,
    PlaybackAck,
    Cancellation,
    Expiry,
    Usage,
    DutyLease,
}
impl DecisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Observation => "observation",
            Self::Assembly => "assembly",
            Self::Decision => "decision",
            Self::Guard => "guard",
            Self::Cue => "cue",
            Self::Approval => "approval",
            Self::Synthesis => "synthesis",
            Self::PlaybackAck => "playback_ack",
            Self::Cancellation => "cancellation",
            Self::Expiry => "expiry",
            Self::Usage => "usage",
            Self::DutyLease => "duty_lease",
        }
    }
    /// Authorization, approval, cue and duty decisions are never dropped; the
    /// remaining chain segments are telemetry (INV-15).
    pub fn criticality(self) -> Criticality {
        match self {
            Self::Cue | Self::Approval | Self::Cancellation | Self::DutyLease => {
                Criticality::Critical
            }
            _ => Criticality::Telemetry,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionAck {
    /// False while the entry sits in the degraded telemetry buffer.
    pub persisted: bool,
    pub entry_hash: Option<String>,
    pub dropped_oldest: bool,
}

/// One persisted decision-log row, as stored (payload still JSON text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionEntryView {
    pub seq: i64,
    pub kind: String,
    pub criticality: String,
    pub session_id: Option<String>,
    pub payload_json: String,
    pub trace_id: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct BufferedDecision {
    workspace_id: String,
    session_id: Option<String>,
    kind: &'static str,
    criticality: &'static str,
    payload_json: String,
    created_at_ms: i64,
    trace_id: String,
}

fn preimage(
    prev_hash: &str,
    kind: &str,
    criticality: &str,
    session: Option<&str>,
    payload_hash: &str,
    created_at_ms: i64,
    trace_id: &str,
) -> String {
    format!(
        "{prev_hash}\n{kind}\n{criticality}\n{}\n{payload_hash}\n{created_at_ms}\n{trace_id}",
        session.unwrap_or("")
    )
}

/// Critical approval state and its chain entry commit in the owner's transaction.
pub(crate) fn approval_entry(
    op: &Operation,
    conn: &rusqlite::Transaction<'_>,
    workspace: &str,
    session: &str,
    kind: DecisionKind,
    payload: &serde_json::Value,
    trace_id: &str,
) -> Result<()> {
    let entry = BufferedDecision {
        workspace_id: workspace.into(),
        session_id: Some(session.into()),
        kind: kind.as_str(),
        criticality: Criticality::Critical.as_str(),
        payload_json: payload.to_string(),
        created_at_ms: utc_ms()?,
        trace_id: trace_id.into(),
    };
    insert_entry(op, conn, &entry).map(|_| ())
}

impl Store {
    /// Append one decision-log entry. Critical kinds either persist or return
    /// the storage error unchanged; telemetry kinds are buffered by the same
    /// single-writer owner when the database write fails.
    pub fn append_decision(
        &mut self,
        op: &Operation,
        workspace: &str,
        session: Option<&str>,
        kind: DecisionKind,
        payload: &serde_json::Value,
        trace_id: &str,
    ) -> Result<DecisionAck> {
        self.start(op)?;
        id(workspace)?;
        id(trace_id)?;
        if let Some(session) = session {
            id(session)?;
        }
        let payload_json = serde_json::to_string(payload).map_err(|_| {
            error(
                ErrorCode::InvalidArgument,
                "decision payload not serializable",
            )
        })?;
        if payload_json.len() > MAX_PAYLOAD_BYTES {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "decision payload too large",
            ));
        }
        let entry = BufferedDecision {
            workspace_id: workspace.into(),
            session_id: session.map(Into::into),
            kind: kind.as_str(),
            criticality: kind.criticality().as_str(),
            payload_json,
            created_at_ms: utc_ms()?,
            trace_id: trace_id.into(),
        };
        if kind.criticality() == Criticality::Critical {
            // Critical writes never spend their deadline flushing telemetry.
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let entry_hash = insert_entry(op, &tx, &entry)?;
            op.check()?;
            tx.commit()?;
            return Ok(DecisionAck {
                persisted: true,
                entry_hash: Some(entry_hash),
                dropped_oldest: false,
            });
        }
        match self.persist_telemetry(op, Some(&entry)) {
            Ok(entry_hash) => Ok(DecisionAck {
                persisted: true,
                entry_hash: Some(entry_hash),
                dropped_oldest: false,
            }),
            Err(failure) => {
                if op.stopped() {
                    return Err(failure);
                }
                let dropped_oldest = self.buffer_decision(entry);
                Ok(DecisionAck {
                    persisted: false,
                    entry_hash: None,
                    dropped_oldest,
                })
            }
        }
    }
    /// Retry buffered telemetry atomically in arrival order. On any failure
    /// all inserts roll back and the entire buffer remains available to retry.
    pub fn flush_decisions(&mut self, op: &Operation) -> Result<()> {
        self.start(op)?;
        if self.decisions.is_empty() {
            return Ok(());
        }
        self.persist_telemetry(op, None).map(|_| ())
    }
    pub fn decision_buffer_stats(&self) -> (usize, u64) {
        (self.decisions.len(), self.dropped_telemetry)
    }
    /// Reads persisted decision entries in chain order, newest last. Read-only:
    /// it never appends, repairs or reorders the chain.
    pub fn decision_entries(
        &self,
        op: &Operation,
        workspace: &str,
        limit: u32,
    ) -> Result<Vec<DecisionEntryView>> {
        self.start(op)?;
        let mut statement = self.conn.prepare(
            "SELECT seq,kind,criticality,session_id,payload_json,trace_id,created_at_ms FROM decision_log WHERE workspace_id=?1 ORDER BY seq DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(rusqlite::params![workspace, limit], |row| {
            Ok(DecisionEntryView {
                seq: row.get(0)?,
                kind: row.get(1)?,
                criticality: row.get(2)?,
                session_id: row.get(3)?,
                payload_json: row.get(4)?,
                trace_id: row.get(5)?,
                created_at_ms: row.get(6)?,
            })
        })?;
        let mut entries = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        entries.reverse();
        op.check()?;
        Ok(entries)
    }
    /// Recompute the chain for one workspace and report the first divergent
    /// sequence number.
    pub fn verify_decision_chain(&self, op: &Operation, workspace: &str) -> Result<()> {
        self.start(op)?;
        let mut statement = self.conn.prepare(
            "SELECT seq,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id FROM decision_log WHERE workspace_id=?1 ORDER BY seq",
        )?;
        let rows = statement.query_map([workspace], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;
        let mut expected_prev = GENESIS_HASH.to_string();
        for row in rows {
            let (
                seq,
                session,
                kind,
                criticality,
                payload_json,
                payload_hash,
                prev_hash,
                entry_hash,
                created_at_ms,
                trace_id,
            ) = row?;
            if payload_hash != hash(payload_json.as_bytes()) || prev_hash != expected_prev {
                return Err(error_at(
                    ErrorCode::Internal,
                    "decision log chain broken",
                    seq,
                ));
            }
            let expected = hash(
                preimage(
                    &prev_hash,
                    &kind,
                    &criticality,
                    session.as_deref(),
                    &payload_hash,
                    created_at_ms,
                    &trace_id,
                )
                .as_bytes(),
            );
            if expected != entry_hash {
                return Err(error_at(
                    ErrorCode::Internal,
                    "decision log entry hash mismatch",
                    seq,
                ));
            }
            expected_prev = entry_hash;
        }
        op.check()?;
        Ok(())
    }
    fn buffer_decision(&mut self, entry: BufferedDecision) -> bool {
        let dropped_oldest = self.decisions.len() >= TELEMETRY_BUFFER_CAPACITY;
        if dropped_oldest {
            self.decisions.pop_front();
            self.dropped_telemetry = self.dropped_telemetry.saturating_add(1);
        }
        self.decisions.push_back(entry);
        dropped_oldest
    }
    fn persist_telemetry(
        &mut self,
        op: &Operation,
        incoming: Option<&BufferedDecision>,
    ) -> Result<String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut last_hash = GENESIS_HASH.to_string();
        for entry in self.decisions.iter().chain(incoming) {
            last_hash = insert_entry(op, &tx, entry)?;
        }
        op.check()?;
        tx.commit()?;
        self.decisions.clear();
        Ok(last_hash)
    }
}

fn insert_entry(
    op: &Operation,
    tx: &rusqlite::Transaction<'_>,
    entry: &BufferedDecision,
) -> Result<String> {
    op.check()?;
    let previous: Option<String> = tx
        .query_row(
            "SELECT entry_hash FROM decision_log WHERE workspace_id=?1 ORDER BY seq DESC LIMIT 1",
            [&entry.workspace_id],
            |row| row.get(0),
        )
        .optional()?;
    let prev_hash = previous.unwrap_or_else(|| GENESIS_HASH.into());
    let payload_hash = hash(entry.payload_json.as_bytes());
    let entry_hash = hash(
        preimage(
            &prev_hash,
            entry.kind,
            entry.criticality,
            entry.session_id.as_deref(),
            &payload_hash,
            entry.created_at_ms,
            &entry.trace_id,
        )
        .as_bytes(),
    );
    tx.execute(
            "INSERT INTO decision_log(workspace_id,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                entry.workspace_id,
                entry.session_id,
                entry.kind,
                entry.criticality,
                entry.payload_json,
                payload_hash,
                prev_hash,
                entry_hash,
                entry.created_at_ms,
                entry.trace_id
            ],
        )?;
    op.check()?;
    Ok(entry_hash)
}

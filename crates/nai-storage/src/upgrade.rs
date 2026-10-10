//! Durable upgrade journal. The upgrade orchestrator decides when a step is
//! reached; this module owns only the record of how far a transaction got, so a
//! crash can be recovered without re-deriving state from the filesystem.
//!
//! `upgrade_journal` is instance-scoped (no `workspace_id`): an upgrade replaces
//! the program and its database, so it cannot belong to the database it replaces.
use crate::{Operation, Result, Store, error};
use nai_contracts::v1::ErrorCode;
use rusqlite::{OptionalExtension, params};

/// Ordered upgrade steps. `RolledBack` is a terminal failure state reachable
/// from any non-terminal step; `Committed` is the terminal success state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeStep {
    Prepared,
    Migrated,
    Checked,
    Committed,
    RolledBack,
}
impl UpgradeStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Migrated => "migrated",
            Self::Checked => "checked",
            Self::Committed => "committed",
            Self::RolledBack => "rolled_back",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "prepared" => Some(Self::Prepared),
            "migrated" => Some(Self::Migrated),
            "checked" => Some(Self::Checked),
            "committed" => Some(Self::Committed),
            "rolled_back" => Some(Self::RolledBack),
            _ => None,
        }
    }
    fn rank(self) -> u8 {
        match self {
            Self::Prepared => 0,
            Self::Migrated => 1,
            Self::Checked => 2,
            Self::Committed => 3,
            Self::RolledBack => 4,
        }
    }
    pub fn terminal(self) -> bool {
        matches!(self, Self::Committed | Self::RolledBack)
    }
    /// Idempotent re-write of the same step is allowed; a terminal step never
    /// moves; otherwise a step may only advance one rank or converge to
    /// `rolled_back`, so a crash cannot leave a transaction quietly regressing.
    fn permits(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        if self.terminal() {
            return false;
        }
        next == Self::RolledBack || next.rank() == self.rank() + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeRecord {
    pub transaction_id: String,
    pub from_manifest: String,
    pub to_manifest: String,
    pub step: UpgradeStep,
    pub backup_ref: Option<String>,
    pub error_code: Option<String>,
}

fn bounded(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        Err(error(ErrorCode::InvalidArgument, "invalid upgrade field"))
    } else {
        Ok(())
    }
}

impl Store {
    /// Advance (or idempotently re-write) the journal for one transaction.
    /// `backup_ref` and `error_code` are preserved once set, so a later step that
    /// does not repeat them cannot erase the evidence an earlier step recorded.
    pub fn record_upgrade(&mut self, op: &Operation, record: &UpgradeRecord) -> Result<()> {
        self.start(op)?;
        bounded(&record.transaction_id)?;
        bounded(&record.from_manifest)?;
        bounded(&record.to_manifest)?;
        if let Some(reference) = &record.backup_ref {
            bounded(reference)?;
        }
        if let Some(code) = &record.error_code {
            bounded(code)?;
        }
        if let Some(current) = self.upgrade_record(op, &record.transaction_id)? {
            if current.from_manifest != record.from_manifest
                || current.to_manifest != record.to_manifest
            {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "upgrade transaction identity changed",
                ));
            }
            if !current.step.permits(record.step) {
                return Err(error(
                    ErrorCode::Conflict,
                    "upgrade step regression or skip",
                ));
            }
        }
        self.conn.execute(
            "INSERT INTO upgrade_journal(transaction_id,from_manifest,to_manifest,step,backup_ref,error_code)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(transaction_id) DO UPDATE SET
               step=excluded.step,
               backup_ref=COALESCE(excluded.backup_ref,upgrade_journal.backup_ref),
               error_code=COALESCE(excluded.error_code,upgrade_journal.error_code)",
            params![
                record.transaction_id,
                record.from_manifest,
                record.to_manifest,
                record.step.as_str(),
                record.backup_ref,
                record.error_code
            ],
        )?;
        op.check()?;
        Ok(())
    }
    pub fn upgrade_record(
        &self,
        op: &Operation,
        transaction_id: &str,
    ) -> Result<Option<UpgradeRecord>> {
        self.start(op)?;
        bounded(transaction_id)?;
        let row = self
            .conn
            .query_row(
                "SELECT transaction_id,from_manifest,to_manifest,step,backup_ref,error_code
                 FROM upgrade_journal WHERE transaction_id=?1",
                params![transaction_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, from, to, step, backup, code)) = row else {
            return Ok(None);
        };
        let step = UpgradeStep::parse(&step)
            .ok_or_else(|| error(ErrorCode::Internal, "unknown recorded upgrade step"))?;
        Ok(Some(UpgradeRecord {
            transaction_id: id,
            from_manifest: from,
            to_manifest: to,
            step,
            backup_ref: backup,
            error_code: code,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataPaths;
    use std::time::Duration;

    fn op() -> Operation {
        Operation::new(Duration::from_secs(30)).unwrap()
    }
    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path()).unwrap();
        let store = Store::open(paths, &op()).unwrap();
        (dir, store)
    }
    fn record(step: UpgradeStep) -> UpgradeRecord {
        UpgradeRecord {
            transaction_id: "tx-1".into(),
            from_manifest: "m-old".into(),
            to_manifest: "m-new".into(),
            step,
            backup_ref: None,
            error_code: None,
        }
    }

    #[test]
    fn steps_advance_and_never_regress() {
        let (_dir, mut store) = store();
        store
            .record_upgrade(&op(), &record(UpgradeStep::Prepared))
            .unwrap();
        let mut migrated = record(UpgradeStep::Migrated);
        migrated.backup_ref = Some("backup-1".into());
        store.record_upgrade(&op(), &migrated).unwrap();
        assert_eq!(
            store.upgrade_record(&op(), "tx-1").unwrap().unwrap().step,
            UpgradeStep::Migrated
        );
        // same step again is idempotent
        store.record_upgrade(&op(), &migrated).unwrap();
        // skipping a step is refused
        assert!(
            store
                .record_upgrade(&op(), &record(UpgradeStep::Committed))
                .is_err()
        );
        // converging to rolled_back is allowed and preserves the backup ref
        let rolled = record(UpgradeStep::RolledBack);
        store.record_upgrade(&op(), &rolled).unwrap();
        let final_record = store.upgrade_record(&op(), "tx-1").unwrap().unwrap();
        assert_eq!(final_record.step, UpgradeStep::RolledBack);
        assert_eq!(final_record.backup_ref.as_deref(), Some("backup-1"));
        // a terminal step never moves again
        assert!(
            store
                .record_upgrade(&op(), &record(UpgradeStep::Committed))
                .is_err()
        );
    }

    #[test]
    fn identity_change_is_refused_and_absent_is_none() {
        let (_dir, mut store) = store();
        assert!(store.upgrade_record(&op(), "tx-1").unwrap().is_none());
        store
            .record_upgrade(&op(), &record(UpgradeStep::Prepared))
            .unwrap();
        let mut changed = record(UpgradeStep::Migrated);
        changed.to_manifest = "other".into();
        assert!(store.record_upgrade(&op(), &changed).is_err());
    }
}

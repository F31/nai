//! Pure recovery decisions. The storage owner must verify facts and perform I/O.
use serde::Deserialize;
use std::sync::OnceLock;

const JOURNAL_SCHEMA: &str =
    include_str!("../../../schemas/config/personal-policy-migration-journal.schema.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepared,
    BackupVerified,
    Committed,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    schema_version: u8,
    migration_id: String,
    source_version: u8,
    target_version: u8,
    phase: Phase,
    backup_id: String,
    backup_manifest_sha256: String,
}

impl Journal {
    pub fn backup_identity(&self) -> (&str, &str) {
        (&self.backup_id, &self.backup_manifest_sha256)
    }
    pub fn migration_id(&self) -> &str {
        &self.migration_id
    }
    pub fn versions(&self) -> (u8, u8, u8) {
        (
            self.schema_version,
            self.source_version,
            self.target_version,
        )
    }
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 4096 {
            return None;
        }
        // Parse the original first to reject duplicate fields.
        let parsed = serde_json::from_slice(bytes).ok()?;
        let value = serde_json::from_slice(bytes).ok()?;
        static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
        let validator = VALIDATOR.get_or_init(|| {
            let source =
                serde_json::from_str(JOURNAL_SCHEMA).expect("embedded schema is valid JSON");
            jsonschema::validator_for(&source).expect("embedded schema compiles")
        });
        validator.is_valid(&value).then_some(parsed)
    }
}

/// Verified by the storage owner against the specific journal backup identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupObservation {
    NotChecked,
    Valid,
    Invalid,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    NoMigration,
    VerifyBackup,
    RecordBackupVerified,
    Activate,
    FinalizeJournal,
    Ready,
    Reject,
}

/// Never instructs restoration of a snapshot: committed newer facts must survive.
pub fn recover(
    journal: Option<&Journal>,
    database_version: u32,
    database_valid: bool,
    backup: BackupObservation,
) -> Recovery {
    if !database_valid {
        return Recovery::Reject;
    }
    let Some(journal) = journal else {
        return if database_version == 2 {
            Recovery::NoMigration
        } else {
            Recovery::Reject
        };
    };
    if !matches!(
        (journal.phase, database_version),
        (Phase::Prepared, 2) | (Phase::BackupVerified, 2 | 3) | (Phase::Committed, 3)
    ) {
        return Recovery::Reject;
    }
    if backup == BackupObservation::Invalid {
        return Recovery::Reject;
    }
    if backup == BackupObservation::NotChecked {
        return Recovery::VerifyBackup;
    }
    match (journal.phase, database_version) {
        (Phase::Prepared, 2) => Recovery::RecordBackupVerified,
        (Phase::BackupVerified, 2) => Recovery::Activate,
        (Phase::BackupVerified, 3) => Recovery::FinalizeJournal,
        (Phase::Committed, 3) => Recovery::Ready,
        _ => Recovery::Reject,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn journal(phase: &str) -> Journal {
        Journal::parse(format!(r#"{{"schema_version":1,"migration_id":"00000000-0000-0000-0000-000000000001","source_version":2,"target_version":3,"phase":"{phase}","backup_id":"00000000-0000-0000-0000-000000000002","backup_manifest_sha256":"{}"}}"#, "a".repeat(64)).as_bytes()).unwrap()
    }
    #[test]
    fn crash_boundaries_preserve_committed_database() {
        let prepared = journal("prepared");
        let verified = journal("backup_verified");
        let committed = journal("committed");
        assert_eq!(
            recover(None, 2, true, BackupObservation::NotChecked),
            Recovery::NoMigration
        );
        assert_eq!(
            recover(Some(&prepared), 2, true, BackupObservation::NotChecked),
            Recovery::VerifyBackup
        );
        assert_eq!(
            recover(Some(&prepared), 2, true, BackupObservation::Valid),
            Recovery::RecordBackupVerified
        );
        assert_eq!(
            recover(Some(&verified), 2, true, BackupObservation::Valid),
            Recovery::Activate
        );
        assert_eq!(
            recover(Some(&verified), 3, true, BackupObservation::Valid),
            Recovery::FinalizeJournal
        );
        assert_eq!(
            recover(Some(&committed), 3, true, BackupObservation::Valid),
            Recovery::Ready
        );
        assert_eq!(
            recover(Some(&committed), 2, true, BackupObservation::Valid),
            Recovery::Reject
        );
        assert_eq!(
            recover(Some(&prepared), 3, true, BackupObservation::Valid),
            Recovery::Reject
        );
        assert_eq!(
            recover(None, 3, true, BackupObservation::Valid),
            Recovery::Reject
        );
        assert_eq!(
            recover(Some(&verified), 3, true, BackupObservation::Invalid),
            Recovery::Reject
        );
        assert_eq!(
            recover(Some(&verified), 3, false, BackupObservation::Valid),
            Recovery::Reject
        );
    }
    #[test]
    fn untrusted_journal_must_match_contract_without_duplicate_fields() {
        assert!(Journal::parse(b"{}").is_none());
        assert!(Journal::parse(&vec![b' '; 4097]).is_none());
        let raw = format!(
            r#"{{"schema_version":1,"migration_id":"00000000-0000-0000-0000-000000000001","source_version":2,"target_version":3,"phase":"prepared","phase":"committed","backup_id":"00000000-0000-0000-0000-000000000002","backup_manifest_sha256":"{}"}}"#,
            "a".repeat(64)
        );
        assert!(Journal::parse(raw.as_bytes()).is_none());
    }
}

//! Isolated transaction primitive; NOT installed in a production database or IPC.
//! A future authenticated owner must verify admission/rights and gate media first.
use super::*;

struct Identity<'a> {
    workspace: &'a str,
    profile: &'a str,
    package: &'a str,
    rights: &'a str,
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate(identity: &Identity<'_>, actor: &str) -> Result<()> {
    uuid(identity.workspace)?;
    uuid(identity.profile)?;
    uuid(actor)?;
    if !valid_digest(identity.package) || !valid_digest(identity.rights) {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}

/// Caller supplies trusted admission evidence, checks the actor, and revokes
/// media before a binding change. This function does not confer authorization.
fn select_in(
    tx: &Transaction<'_>,
    op: &Operation,
    actor: &str,
    identity: &Identity<'_>,
    expected: u64,
    now: i64,
) -> Result<u64> {
    op.check()?;
    validate(identity, actor)?;
    let current: Option<u64> = tx
        .query_row(
            "SELECT revision FROM avatar_bindings WHERE workspace_id=?1",
            [identity.workspace],
            |r| uint(r, 0),
        )
        .optional()
        .wb()?;
    if current.unwrap_or(0) != expected {
        return Err(Error::RevisionConflict);
    }
    let revoked: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM avatar_rights_revocations WHERE workspace_id=?1 AND package_sha256=?2 AND rights_sha256=?3)", params![identity.workspace, identity.package, identity.rights], |r| r.get(0)).wb()?;
    if revoked {
        return Err(Error::PermissionDenied);
    }
    let revision = expected.checked_add(1).ok_or(Error::ResourceExhausted)?;
    tx.execute("INSERT INTO avatar_bindings(workspace_id,profile_id,package_sha256,rights_sha256,revision,active) VALUES(?1,?2,?3,?4,?5,1) ON CONFLICT(workspace_id) DO UPDATE SET profile_id=excluded.profile_id,package_sha256=excluded.package_sha256,rights_sha256=excluded.rights_sha256,revision=excluded.revision,active=1", params![identity.workspace,identity.profile,identity.package,identity.rights,int(revision)?]).wb()?;
    audit(
        tx,
        identity.workspace,
        actor,
        "avatar_select",
        identity.profile,
        now,
    )?;
    op.check()?;
    Ok(revision)
}

/// Caller must close public output before invoking this transaction.
fn revoke_in(
    tx: &Transaction<'_>,
    op: &Operation,
    actor: &str,
    identity: &Identity<'_>,
    expected: u64,
    now: i64,
) -> Result<u64> {
    op.check()?;
    validate(identity, actor)?;
    let current: Option<(String,String,String,u64,bool)> = tx.query_row("SELECT profile_id,package_sha256,rights_sha256,revision,active FROM avatar_bindings WHERE workspace_id=?1", [identity.workspace], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,uint(r,3)?,r.get(4)?))).optional().wb()?;
    let Some((profile, package, rights, revision, active)) = current else {
        return Err(Error::NotFound);
    };
    if revision != expected
        || profile != identity.profile
        || package != identity.package
        || rights != identity.rights
        || !active
    {
        return Err(Error::RevisionConflict);
    }
    let next = revision.checked_add(1).ok_or(Error::ResourceExhausted)?;
    tx.execute("INSERT INTO avatar_rights_revocations(workspace_id,package_sha256,rights_sha256) VALUES(?1,?2,?3)", params![identity.workspace,identity.package,identity.rights]).wb()?;
    tx.execute(
        "UPDATE avatar_bindings SET active=0,revision=?1 WHERE workspace_id=?2",
        params![int(next)?, identity.workspace],
    )
    .wb()?;
    audit(
        tx,
        identity.workspace,
        actor,
        "avatar_revoke",
        identity.profile,
        now,
    )?;
    op.check()?;
    Ok(next)
}

// Own the transaction so an error cannot be followed by an accidental commit
// of the partial write by a caller. This prototype is not an IPC entry point.
fn select(
    conn: &mut Connection,
    op: &Operation,
    actor: &str,
    identity: &Identity<'_>,
    expected: u64,
    now: i64,
) -> Result<u64> {
    let tx = conn.transaction().wb()?;
    let revision = select_in(&tx, op, actor, identity, expected, now)?;
    op.check()?;
    tx.commit().wb()?;
    Ok(revision)
}

fn revoke(
    conn: &mut Connection,
    op: &Operation,
    actor: &str,
    identity: &Identity<'_>,
    expected: u64,
    now: i64,
) -> Result<u64> {
    let tx = conn.transaction().wb()?;
    let revision = revoke_in(&tx, op, actor, identity, expected, now)?;
    op.check()?;
    tx.commit().wb()?;
    Ok(revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    const WS: &str = "00000000-0000-0000-0000-000000000010";
    const ACTOR: &str = "00000000-0000-0000-0000-000000000001";
    const PROFILE: &str = "00000000-0000-0000-0000-000000000020";
    fn initialize(c: &Connection) {
        c.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE workspaces(id TEXT PRIMARY KEY); INSERT INTO workspaces VALUES('00000000-0000-0000-0000-000000000010'); CREATE TABLE audit_events(seq INTEGER PRIMARY KEY,workspace_id TEXT,principal_id TEXT,action TEXT,resource_id TEXT,revision INTEGER,created_at INTEGER,updated_at INTEGER);").unwrap();
        c.execute_batch(include_str!(
            "../../../schemas/storage/avatar-authority.sql"
        ))
        .unwrap();
    }
    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        initialize(&c);
        c
    }
    #[test]
    fn binding_contract_preserves_identity_and_unknown_rights_never_becomes_active() {
        use nai_contracts::workbench::{
            AvatarBindingIdentity, AvatarBindingView, AvatarRightsState,
        };
        let view = AvatarBindingView {
            identity: Some(AvatarBindingIdentity {
                workspace_id: WS.into(),
                profile_id: PROFILE.into(),
                package_sha256: "a".repeat(64),
                rights_sha256: "b".repeat(64),
            }),
            revision: 9_007_199_254_740_993,
            rights_state: 77,
            selected: false,
        };
        let decoded = AvatarBindingView::decode(view.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, view);
        assert!(AvatarRightsState::try_from(decoded.rights_state).is_err());
        assert_ne!(decoded.rights_state, AvatarRightsState::Active as i32);
    }
    #[test]
    fn cas_revocation_and_audit_are_transactional() {
        let mut c = db();
        let package = "a".repeat(64);
        let rights = "b".repeat(64);
        let i = Identity {
            workspace: WS,
            profile: PROFILE,
            package: &package,
            rights: &rights,
        };
        let op = Operation::new(4000).unwrap();
        assert_eq!(select(&mut c, &op, ACTOR, &i, 0, 1).unwrap(), 1);
        assert_eq!(
            select(&mut c, &op, ACTOR, &i, 0, 1),
            Err(Error::RevisionConflict)
        );
        c.execute_batch("CREATE TRIGGER fail_avatar_audit BEFORE INSERT ON audit_events BEGIN SELECT RAISE(ABORT, 'audit'); END;").unwrap();
        assert!(revoke(&mut c, &op, ACTOR, &i, 1, 2).is_err());
        assert_eq!(
            c.query_row("SELECT active FROM avatar_bindings", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM avatar_rights_revocations", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
        c.execute_batch("DROP TRIGGER fail_avatar_audit").unwrap();
        assert_eq!(revoke(&mut c, &op, ACTOR, &i, 1, 2).unwrap(), 2);
        assert_eq!(
            select(&mut c, &op, ACTOR, &i, 2, 3),
            Err(Error::PermissionDenied)
        );
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM audit_events", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        let other_rights = "c".repeat(64);
        let other = Identity {
            rights: &other_rights,
            ..i
        };
        assert_eq!(select(&mut c, &op, ACTOR, &other, 2, 3).unwrap(), 3);
        let old = Identity {
            rights: &rights,
            ..other
        };
        assert_eq!(
            select(&mut c, &op, ACTOR, &old, 3, 4),
            Err(Error::PermissionDenied)
        );
        let cancelled = Operation::new(4000).unwrap();
        cancelled.cancel();
        assert_eq!(
            revoke(&mut c, &cancelled, ACTOR, &other, 3, 4),
            Err(Error::Cancelled)
        );
        assert_eq!(
            c.query_row("SELECT revision FROM avatar_bindings", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }

    #[test]
    fn concurrent_first_selection_commits_only_one_binding_and_audit() {
        use std::sync::{Arc, Barrier};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("authority.sqlite");
        let c = Connection::open(&path).unwrap();
        initialize(&c);
        drop(c);
        let barrier = Arc::new(Barrier::new(2));
        let workers: Vec<_> = ["a", "c"]
            .into_iter()
            .map(|digit| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut conn = Connection::open(path).unwrap();
                    conn.busy_timeout(std::time::Duration::from_secs(1))
                        .unwrap();
                    let package = digit.repeat(64);
                    let rights = "b".repeat(64);
                    let identity = Identity {
                        workspace: WS,
                        profile: PROFILE,
                        package: &package,
                        rights: &rights,
                    };
                    let op = Operation::new(4000).unwrap();
                    barrier.wait();
                    select(&mut conn, &op, ACTOR, &identity, 0, 1)
                })
            })
            .collect();
        let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(
            results
                .iter()
                .any(|r| matches!(r, Err(Error::RevisionConflict | Error::Busy)))
        );
        let c = Connection::open(path).unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM avatar_bindings", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM audit_events", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn sqlite_snapshot_preserves_revocation_and_does_not_overwrite_newer_facts() {
        let temp = tempfile::tempdir().unwrap();
        let source_path = temp.path().join("source.sqlite");
        let backup_path = temp.path().join("backup.sqlite");
        let mut source = Connection::open(&source_path).unwrap();
        initialize(&source);
        let package = "a".repeat(64);
        let rights = "b".repeat(64);
        let identity = Identity {
            workspace: WS,
            profile: PROFILE,
            package: &package,
            rights: &rights,
        };
        let op = Operation::new(4000).unwrap();
        assert_eq!(select(&mut source, &op, ACTOR, &identity, 0, 1).unwrap(), 1);
        assert_eq!(revoke(&mut source, &op, ACTOR, &identity, 1, 2).unwrap(), 2);
        {
            let mut backup = Connection::open(&backup_path).unwrap();
            let copy = rusqlite::backup::Backup::new(&source, &mut backup).unwrap();
            copy.run_to_completion(32, std::time::Duration::from_millis(1), None)
                .unwrap();
        }
        let new_rights = "c".repeat(64);
        let newer = Identity {
            rights: &new_rights,
            ..identity
        };
        assert_eq!(select(&mut source, &op, ACTOR, &newer, 2, 3).unwrap(), 3);
        drop(source);
        let mut saved = Connection::open(&backup_path).unwrap();
        assert_eq!(
            saved
                .query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        assert_eq!(
            saved
                .query_row("SELECT revision FROM avatar_bindings", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
        let original = Identity {
            rights: &rights,
            ..newer
        };
        assert_eq!(
            select(&mut saved, &op, ACTOR, &original, 2, 4),
            Err(Error::PermissionDenied)
        );
        let current = Connection::open(&source_path).unwrap();
        assert_eq!(
            current
                .query_row("SELECT revision FROM avatar_bindings", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            current
                .query_row("SELECT count(*) FROM audit_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
}

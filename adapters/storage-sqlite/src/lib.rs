//! Isolated RF02 writer. No legacy storage, provider, media, Tauri or SDK dependency.
mod assistance;
#[allow(dead_code)] // Isolated authority prototype; not installed or exposed by Store.
mod avatar_authority;
#[allow(dead_code)] // Inventory prototype; no migration or IPC entry point yet.
mod avatar_backup;
mod avatar_draft;
mod browse;
mod budget;
mod delivery;
mod live;
mod llm;
mod manual_run;
mod migration_backup;
mod model_catalog;
mod model_context;
mod photo_draft;
mod video_draft;
mod question_pool;
mod review;
mod review_questions;
mod shared_catalog;
mod shared_model_preview;
pub use migration_backup::ModelMigrationBackup;
mod tts_task;
mod workspace_lifecycle;
mod workspace_purge;
mod workspace_settings;
pub use tts_task::TEST_TEXT as TTS_TEST_TEXT;
pub use tts_task::TtsDispatch;
mod preview;
mod public_media;
mod publication;
pub use public_media::{
    PublicCandidateSource, PublicMediaApprovalRequest, StoredPublicMediaAuthority,
};
mod root;
use nai_contracts::workbench::*;
use nai_runtime_core::foundation::{Operation, StopFence, manual_guard, uuid};
use prost::Message;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
pub type Error = WorkbenchError;
pub type Result<T> = std::result::Result<T, Error>;
trait Sql<T> {
    fn wb(self) -> Result<T>;
}
impl<T> Sql<T> for rusqlite::Result<T> {
    fn wb(self) -> Result<T> {
        self.map_err(|e| match e {
            rusqlite::Error::SqliteFailure(ref x, _)
                if x.code == rusqlite::ErrorCode::OperationInterrupted =>
            {
                Error::Cancelled
            }
            rusqlite::Error::SqliteFailure(ref x, _)
                if matches!(
                    x.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) =>
            {
                Error::Busy
            }
            rusqlite::Error::SqliteFailure(ref x, _)
                if x.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Error::RevisionConflict
            }
            _ => Error::IoFailed,
        })
    }
}
fn uint(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let n: i64 = row.get(index)?;
    u64::try_from(n).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(e),
        )
    })
}
fn uint_opt(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    let n: Option<i64> = row.get(index)?;
    n.map(|v| {
        u64::try_from(v).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Integer,
                Box::new(e),
            )
        })
    })
    .transpose()
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn utc() -> Result<i64> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::IoFailed)?
            .as_millis(),
    )
    .map_err(|_| Error::IoFailed)
}
fn int(n: u64) -> Result<i64> {
    i64::try_from(n).map_err(|_| Error::InvalidArgument)
}
fn role(conn: &Connection, ws: &str, actor: &str) -> Result<(FoundationRole, u64)> {
    workspace_lifecycle::active(conn, ws)?;
    raw_role(conn, ws, actor)
}
fn raw_role(conn: &Connection, ws: &str, actor: &str) -> Result<(FoundationRole, u64)> {
    uuid(ws)?;
    uuid(actor)?;
    let pair: Option<(i32, u64)> = conn
        .query_row(
            "SELECT role,revision FROM roles WHERE workspace_id=?1 AND principal_id=?2",
            params![ws, actor],
            |r| Ok((r.get(0)?, uint(r, 1)?)),
        )
        .optional()
        .wb()?;
    let (r, rev) = pair.ok_or(Error::PermissionDenied)?;
    let r = FoundationRole::try_from(r).map_err(|_| Error::IntegrityFailed)?;
    if r == FoundationRole::None {
        return Err(Error::PermissionDenied);
    }
    Ok((r, rev))
}
fn role_scope(ws: &str, actor: &str, rev: u64) -> String {
    format!("r:{ws}:{actor}:{rev}")
}
fn object_scope(ws: &str, id: &str) -> String {
    format!("o:{ws}:{id}")
}
fn writer(r: FoundationRole) -> Result<()> {
    if matches!(r, FoundationRole::Admin | FoundationRole::Operator) {
        Ok(())
    } else {
        Err(Error::PermissionDenied)
    }
}
fn audit(
    tx: &Transaction<'_>,
    ws: &str,
    actor: &str,
    action: &str,
    id: &str,
    now: i64,
) -> Result<()> {
    tx.execute("INSERT INTO audit_events(workspace_id,principal_id,action,resource_id,revision,created_at,updated_at) VALUES(?1,?2,?3,?4,1,?5,?5)",params![ws,actor,action,id,now]).map_err(|_|Error::IoFailed)?;
    Ok(())
}
fn workspace(cmd: &foundation_command::Operation) -> &str {
    use foundation_command::Operation::*;
    match cmd {
        CreateWorkspace(x) => &x.workspace_id,
        PreviewSharedModels(x) => &x.workspace_id,
        ReadSharedMigration(x) => &x.workspace_id,
        CancelSharedMigration(x) => &x.workspace_id,
        PreviewWorkspacePurge(x) | ReadWorkspacePurge(x) => &x.workspace_id,
        PurgeWorkspace(x) => &x.workspace_id,
        WorkspaceContext(x) => &x.workspace_id,
        MigrateSharedModels(_) => "",
        CreateWorkspaceSettings(x) | EditWorkspaceSettings(x) => &x.workspace_id,
        RenameWorkspace(x) => &x.workspace_id,
        WorkspaceLifecycle(x) => &x.workspace_id,
        Grant(x) => &x.workspace_id,
        PutObject(x) => &x.workspace_id,
        PutPreparation(x) => &x.workspace_id,
        ReadObject(x) => &x.workspace_id,
        CreateSession(x) => &x.workspace_id,
        PrepareDelivery(x) => &x.workspace_id,
        Approve(x) => &x.workspace_id,
        Dispatch(x) => &x.workspace_id,
        ReadDelivery(x) => &x.workspace_id,
        Stop(x) => &x.workspace_id,
        DeleteObject(x) => &x.workspace_id,
        PublicationImpact(x) => &x.workspace_id,
        Publish(x) => &x.workspace_id,
        ReadPublication(x) => &x.workspace_id,
        RevokePublication(x) => &x.workspace_id,
        ListWorkspaces(x) | ListPreparation(x) => &x.workspace_id,
        SaveLlmModel(x) => &x.workspace_id,
        SaveTtsModel(x) => &x.workspace_id,
        DeleteLlmModel(x) | DeleteTtsModel(x) => &x.workspace_id,
        ReadLlmSettings(x) => &x.workspace_id,
        SetLlmBudget(x) => &x.workspace_id,
        StartLlm(x) => &x.workspace_id,
        TestLlm(x) => &x.workspace_id,
        CheckLlm(x) => &x.workspace_id,
        TestTts(x) => &x.workspace_id,
        CheckPreviewSpeech(x) => &x.workspace_id,
        CheckPublicCandidate(x) => &x.workspace_id,
        ConfirmPublicApproval(x) => x.candidate.as_ref().map_or("", |c| &c.workspace_id),
        StopPublicExecution(x) | RevokePublicApproval(x) => &x.workspace_id,
        StartPublicExecution(x) | RenewPublicExecution(x) | PreparePublicWindow(x) => {
            x.candidate.as_ref().map_or("", |c| &c.workspace_id)
        }
        PreparePublicOutputBinding(_) => "",
        SynthesizePreviewSpeech(x) => x.plan.as_ref().map_or("", |p| p.workspace_id.as_str()),
        AuthorizeTtsAudio(x) | RevokeTtsAudio(x) => &x.workspace_id,
        StartTtsAudition(x) => x.audio.as_ref().map_or("", |a| a.workspace_id.as_str()),
        SelectOutputDevice(x) => &x.workspace_id,
        ReadTtsTask(x) | CancelTts(x) | RenewTtsAudition(x) | StopTtsAudition(x) => &x.workspace_id,
        ReadLlmTask(x) | CancelLlm(x) => &x.workspace_id,
        ResolveLlm(x) => &x.workspace_id,
        ReadQuestionPool(x) => &x.workspace_id,
        EnqueueRunQuestion(x) => x.source.as_ref().map_or("", |s| s.workspace_id.as_str()),
        ReadRunQuestionPool(x) => x.source.as_ref().map_or("", |s| s.workspace_id.as_str()),
        EnqueueQuestion(x) => x.scope.as_ref().map_or("", |s| s.workspace_id.as_str()),
        RunQuestion(x) => &x.workspace_id,
        CancelQuestion(x) => &x.workspace_id,
        AskFaq(x) => &x.workspace_id,
        ReadFaq(x) => &x.workspace_id,
        ResolveFaq(x) => &x.workspace_id,
        CreateLiveRun(x) => x.scope.as_ref().map_or("", |s| s.workspace_id.as_str()),
        ReadLiveRun(x) => &x.workspace_id,
        ReadLiveSpeechPlan(x) => &x.workspace_id,
        ReadLiveAuto(x) => &x.workspace_id,
        StartLiveAuto(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        ControlLiveQuestion(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        ControlLiveTakeover(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        ReadReview(x) => &x.workspace_id,
        SaveReviewNote(x) => x.source.as_ref().map_or("", |s| s.workspace_id.as_str()),
        PlanReviewDraft(x) => x.source.as_ref().map_or("", |s| s.workspace_id.as_str()),
        ReadReviewDraft(x) => &x.workspace_id,
        ApplyReviewDraft(x) => &x.workspace_id,
        PrepareLiveAuto(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        StopLiveAuto(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        ConfirmLiveAuto(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        PreviewLiveSentence(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        ControlLiveBatch(x) => x.run.as_ref().map_or("", |r| r.workspace_id.as_str()),
        ControlLiveRun(x) => &x.workspace_id,
        Preview(x) => &x.workspace_id,
        ReadPreview(x) => &x.workspace_id,
        PreviewPresented(x) => &x.workspace_id,
        StopPreview(x) => &x.workspace_id,
    }
}
fn object(conn: &Connection, ws: &str, id: &str, revision: u64) -> Result<(u64, String)> {
    uuid(id)?;
    let current: Option<(u64, bool)> = conn
        .query_row(
            "SELECT revision,deleted FROM objects WHERE workspace_id=?1 AND id=?2",
            params![ws, id],
            |r| Ok((uint(r, 0)?, r.get(1)?)),
        )
        .optional()
        .wb()?;
    let (latest, deleted) = current.ok_or(Error::NotFound)?;
    if deleted {
        return Err(Error::NotFound);
    }
    let wanted = if revision == 0 { latest } else { revision };
    let digest=conn.query_row("SELECT sha256 FROM object_versions WHERE workspace_id=?1 AND object_id=?2 AND revision=?3",params![ws,id,int(wanted)?],|r|r.get(0)).optional().wb()?.ok_or(Error::NotFound)?;
    Ok((latest, digest))
}
fn body(root: &Path, digest: &str) -> Result<String> {
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::IntegrityFailed);
    }
    let bytes = root::read(&root.join("objects").join(digest), 8192)?;
    if hash(&bytes) != digest {
        return Err(Error::IntegrityFailed);
    }
    String::from_utf8(bytes).map_err(|_| Error::IntegrityFailed)
}
fn store_body(root: &Path, bytes: &[u8]) -> Result<String> {
    let digest = hash(bytes);
    let path = root.join("objects").join(&digest);
    root::no_links(&path)?;
    match OpenOptions::new().create_new(true).write(true).open(&path) {
        Ok(mut f) => {
            f.write_all(bytes).map_err(|_| Error::IoFailed)?;
            f.sync_all().map_err(|_| Error::IoFailed)?;
            let directory = root.join("object-ownership");
            root::no_links(&directory)?;
            std::fs::create_dir_all(&directory).map_err(|_| Error::IoFailed)?;
            let owned = directory.join(&digest);
            root::no_links(&owned)?;
            match OpenOptions::new().write(true).create_new(true).open(&owned) {
                Ok(mut marker) => {
                    marker
                        .write_all(b"nai-owned-text-v1")
                        .map_err(|_| Error::IoFailed)?;
                    marker.sync_all().map_err(|_| Error::IoFailed)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if root::read(&owned, 64)? != b"nai-owned-text-v1" {
                        return Err(Error::IntegrityFailed);
                    }
                }
                Err(_) => return Err(Error::IoFailed),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if root::read(&path, 8192)? != bytes {
                return Err(Error::IntegrityFailed);
            }
        }
        Err(_) => return Err(Error::IoFailed),
    }
    Ok(digest)
}
pub struct Store {
    conn: Connection,
    root: PathBuf,
    _owner: File,
    root_owner: String,
    fence: Arc<StopFence>,
    origin: Instant,
    llm_vault: Option<Arc<dyn nai_runtime_core::llm::CredentialVault>>,
    tts_vault: Option<Arc<dyn nai_runtime_core::llm::CredentialVault>>,
}
impl Store {
    pub fn open(
        root: &Path,
        principal: &str,
        initialize: bool,
        fence: Arc<StopFence>,
    ) -> Result<Self> {
        let (mut conn, owner, root_owner) = root::open(root, principal, initialize)?;
        conn.execute_batch(include_str!(
            "../../../schemas/storage/workspace-management.sql"
        ))
        .wb()?;
        conn.execute_batch(include_str!(
            "../../../schemas/storage/shared-model-catalog.sql"
        ))
        .wb()?;
        conn.execute_batch(include_str!(
            "../../../schemas/storage/public-media-approvals.sql"
        ))
        .wb()?;
        root::load_revocations(root, &conn, &fence)?;
        let now = utc()?;
        let tx = conn.transaction().wb()?;
        let public_revoked = tx.execute("UPDATE public_media_approvals SET state='revoked',updated_at=?1 WHERE state='approved'", [now]).wb()?;
        if public_revoked > 0 {
            audit(&tx, "", principal, "public_media_restart_revoke", "", now)?;
        }
        let uncertain = tx
            .execute(
                "UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE state=?3",
                params![
                    DeliveryState::OutcomeUnknown as i32,
                    now,
                    DeliveryState::Committed as i32
                ],
            )
            .wb()?;
        tx.execute("UPDATE outbox SET state='outcome_unknown',revision=revision+1,updated_at=?1 WHERE state='dispatching'",params![now]).wb()?;
        tx.execute("UPDATE budget_reservations SET status='released',revision=revision+1,updated_at=?1 WHERE status='reserved' AND id IN(SELECT resource_id FROM outbox WHERE (event_id LIKE 'llm:%' OR event_id LIKE 'tts:%') AND state='pending')",params![now]).wb()?;
        let invalid=tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE state IN (?3,?4)",params![DeliveryState::Cancelled as i32,now,DeliveryState::AwaitingApproval as i32,DeliveryState::Approved as i32]).wb()?;
        tx.execute("UPDATE outbox SET state='cancelled',revision=revision+1,updated_at=?1 WHERE state='pending'",params![now]).wb()?;
        tx.execute("UPDATE sessions SET stopped=1,epoch=epoch+1,revision=revision+1,updated_at=?1 WHERE stopped=0",params![now]).wb()?;
        tx.execute("DELETE FROM approvals", []).wb()?;
        let uncertain_budget=tx.execute("UPDATE budget_reservations SET status='unknown',revision=revision+1,updated_at=?1 WHERE status='reserved'",params![now]).wb()?;
        if uncertain + invalid + uncertain_budget > 0 {
            audit(
                &tx,
                "",
                principal,
                "recover_revoke_and_preserve_unknown",
                "",
                now,
            )?;
        }
        tx.commit().wb()?;
        let mut store = Self {
            conn,
            root: root.into(),
            _owner: owner,
            root_owner,
            fence,
            origin: Instant::now(),
            llm_vault: None,
            tts_vault: None,
        };
        store.collect_unreferenced()?;
        // A committed purge stays deleted even if owned-file cleanup needs repair.
        let _ = store.cleanup_purge_files(&Operation::new(4000)?, None);
        Ok(store)
    }
    /// Bounded local cleanup; access is denied immediately by tombstone, independent of GC.
    pub fn collect_unreferenced(&mut self) -> Result<usize> {
        let ownership = self.root.join("object-ownership");
        root::no_links(&ownership)?;
        std::fs::create_dir_all(&ownership).map_err(|_| Error::IoFailed)?;
        let mut removed = 0;
        for entry in std::fs::read_dir(&ownership)
            .map_err(|_| Error::IoFailed)?
            .take(128)
        {
            let entry = entry.map_err(|_| Error::IoFailed)?;
            let name = entry
                .file_name()
                .to_str()
                .ok_or(Error::IntegrityFailed)?
                .to_owned();
            if name.len() != 64 || !name.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::IntegrityFailed);
            }
            let queued: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM workspace_purge_files WHERE sha256=?1)",
                    [&name],
                    |r| r.get(0),
                )
                .wb()?;
            if queued {
                continue;
            }
            if root::read(&entry.path(), 64)? != b"nai-owned-text-v1" {
                return Err(Error::IntegrityFailed);
            }
            let used:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM object_versions v JOIN objects o ON o.workspace_id=v.workspace_id AND o.id=v.object_id WHERE v.sha256=?1 AND o.deleted=0) OR EXISTS(SELECT 1 FROM workspace_purge_files WHERE sha256=?1)",params![name],|r|r.get(0)).wb()?;
            if !used {
                let file = self.root.join("objects").join(&name);
                root::no_links(&file)?;
                if file.exists() {
                    if hash(&root::read(&file, 8192)?) != name {
                        return Err(Error::IntegrityFailed);
                    }
                    std::fs::remove_file(&file).map_err(|_| Error::IoFailed)?;
                    removed += 1;
                }
                std::fs::remove_file(entry.path()).map_err(|_| Error::IoFailed)?;
            }
        }
        Ok(removed)
    }
    fn begin(&self, op: &Operation) -> Result<()> {
        op.check()?;
        let check = op.clone();
        self.conn
            .progress_handler(1000, Some(move || check.check().is_err()))
            .wb()?;
        Ok(())
    }
    pub fn tick(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
    pub fn fence(&self) -> Arc<StopFence> {
        self.fence.clone()
    }
    pub fn execute(
        &mut self,
        op: &Operation,
        actor: &str,
        cmd: &FoundationCommand,
    ) -> Result<FoundationResult> {
        self.begin(op)?;
        uuid(actor)?;
        uuid(&cmd.idempotency_key)?;
        if cmd.encoded_len() > 16384 {
            return Err(Error::FrameLimit);
        }
        let action = cmd.operation.as_ref().ok_or(Error::InvalidArgument)?;
        // P0 must precede catalogue/journal reads as well as ordinary SQLite work.
        if matches!(action, foundation_command::Operation::Stop(_)) && actor == self.root_owner {
            self.fence.stop();
        }
        match action {
            ListWorkspaces(x) if x.kind == "cleanup" => return self.pending_purges(actor, x),
            PreviewWorkspacePurge(x) => return self.preview_purge(op, actor, x),
            PurgeWorkspace(x) => return self.purge_workspace(op, actor, x),
            ReadWorkspacePurge(x) => return self.read_purge(op, actor, x),
            WorkspaceContext(x) => return self.workspace_context(op, actor, x),
            _ => (),
        }
        if let foundation_command::Operation::MigrateSharedModels(x) = action {
            return self.migrate_shared(op, actor, x);
        }
        if let foundation_command::Operation::CancelSharedMigration(x) = action {
            return self.cancel_shared(op, actor, x);
        }
        if let foundation_command::Operation::ReadSharedMigration(x) = action {
            if actor != self.root_owner {
                return Err(Error::PermissionDenied);
            }
            if !x.workspace_id.is_empty() {
                return Err(Error::InvalidArgument);
            }
            return self.shared_status();
        }
        if matches!(
            action,
            SaveLlmModel(_) | SaveTtsModel(_) | DeleteLlmModel(_) | DeleteTtsModel(_)
        ) {
            if shared_catalog::pending(&self.conn)? {
                return Err(Error::Busy);
            }
            if shared_catalog::enabled(&self.conn)? && actor != self.root_owner {
                return Err(Error::PermissionDenied);
            }
        }
        if shared_catalog::pending(&self.conn)?
            && matches!(
                action,
                CreateSession(_)
                    | StartLlm(_)
                    | TestLlm(_)
                    | TestTts(_)
                    | SynthesizePreviewSpeech(_)
                    | PrepareLiveAuto(_)
                    | StartLiveAuto(_)
                    | RenameWorkspace(_)
                    | WorkspaceLifecycle(_)
            )
        {
            return Err(Error::Busy);
        }
        let requested_model = match action {
            StartLlm(x) => Some(x.model_id.as_str()),
            StartLiveAuto(x) => Some(x.model_id.as_str()),
            TestLlm(x) => Some(x.model_id.as_str()),
            TestTts(x) => Some(x.model_id.as_str()),
            CheckLlm(x) => Some(x.model_id.as_str()),
            CheckPreviewSpeech(x) => Some(x.model_id.as_str()),
            SynthesizePreviewSpeech(x) => x.plan.as_ref().map(|p| p.model_id.as_str()),
            _ => None,
        };
        if shared_catalog::enabled(&self.conn)?
            && let Some(id) = requested_model
        {
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM shared_models WHERE id=?1)",
                    [id],
                    |r| r.get(0),
                )
                .wb()?;
            if !exists {
                return Err(Error::NotFound);
            }
        }
        if let foundation_command::Operation::PreviewSharedModels(x) = action {
            if actor != self.root_owner {
                return Err(Error::PermissionDenied);
            }
            if !x.workspace_id.is_empty() {
                return Err(Error::InvalidArgument);
            }
            return shared_model_preview::preview(&self.conn, &self.root, op);
        }
        let ws = workspace(action);
        if !matches!(action, ListWorkspaces(_)) {
            uuid(ws)?;
        }
        use foundation_command::Operation::*;
        let now = utc()?;
        let tick = self.tick();
        let origin = self.origin;
        let root = &self.root;
        let fence = &self.fence;
        let root_owner = &self.root_owner;
        let vault = self.llm_vault.as_deref();
        let tx = self.conn.transaction().wb()?;
        let (r, role_rev) = if matches!(action, ListWorkspaces(_)) {
            (FoundationRole::Viewer, 1)
        } else if matches!(action, CreateWorkspace(_) | CreateWorkspaceSettings(_)) {
            if actor != root_owner {
                return Err(Error::PermissionDenied);
            }
            (FoundationRole::Admin, 1)
        } else if matches!(action, WorkspaceLifecycle(_)) {
            raw_role(&tx, ws, actor)?
        } else {
            role(&tx, ws, actor)?
        };
        match action {
            WorkspaceLifecycle(_)
            | EditWorkspaceSettings(_)
            | RenameWorkspace(_)
            | Grant(_)
            | Publish(_)
            | RevokePublication(_)
            | SaveLlmModel(_)
            | SaveTtsModel(_)
            | DeleteLlmModel(_)
            | DeleteTtsModel(_)
            | SetLlmBudget(_) => {
                if r != FoundationRole::Admin {
                    return Err(Error::PermissionDenied);
                }
            }
            ReadObject(_)
            | ReadDelivery(_)
            | ReadPublication(_)
            | ListWorkspaces(_)
            | ListPreparation(_)
            | ReadPreview(_)
            | ReadFaq(_)
            | ReadLiveRun(_)
            | ReadLiveSpeechPlan(_)
            | ReadLiveAuto(_)
            | ReadQuestionPool(_)
            | ReadLlmSettings(_)
            | ReadLlmTask(_)
            | ReadTtsTask(_) => (),
            Stop(_) => {
                if actor != root_owner {
                    return Err(Error::PermissionDenied);
                }
            }
            _ => writer(r)?,
        }
        if !(matches!(action, ListWorkspaces(_))
            || actor == root_owner && matches!(action, Grant(_) | Stop(_)))
        {
            fence.check_scope(&role_scope(ws, actor, role_rev))?;
        }
        let read = matches!(action, WorkspaceLifecycle(x) if x.action == WorkspaceAction::Inspect as i32)
            || matches!(
                action,
                ReadObject(_)
                    | ReadDelivery(_)
                    | ReadPublication(_)
                    | ListWorkspaces(_)
                    | ListPreparation(_)
                    | ReadPreview(_)
                    | ReadFaq(_)
                    | ReadLiveRun(_)
                    | ReadLiveSpeechPlan(_)
                    | ReadLiveAuto(_)
                    | ReadReview(_)
                    | ReadReviewDraft(_)
                    | ReadRunQuestionPool(_)
                    | ReadQuestionPool(_)
                    | ReadLlmSettings(_)
                    | CheckLlm(_)
                    | CheckPreviewSpeech(_)
                    | ReadLlmTask(_)
                    | ReadTtsTask(_)
            );
        let digest = hash(&cmd.encode_to_vec());
        // Replay is checked only after current ACL, so a revoked actor cannot read cached results.
        if !read {
            let prior:Option<(String,Vec<u8>)>=tx.query_row("SELECT command_sha256,result FROM command_results WHERE principal_id=?1 AND idempotency_key=?2",params![actor,cmd.idempotency_key],|r|Ok((r.get(0)?,r.get(1)?))).optional().wb()?;
            if let Some((d, b)) = prior {
                if d != digest {
                    return Err(Error::RevisionConflict);
                }
                let mut result =
                    FoundationResult::decode(b.as_slice()).map_err(|_| Error::IntegrityFailed)?;
                // Results that depend on live authorization are refreshed; never return an old approved state.
                if let SaveReviewNote(x) = action {
                    result = review::read(
                        &tx,
                        root,
                        fence,
                        op,
                        actor,
                        x.source.as_ref().ok_or(Error::InvalidArgument)?,
                        now,
                    )?;
                } else if let PlanReviewDraft(_) = action {
                    result = review::read_job(
                        &tx,
                        root,
                        fence,
                        op,
                        actor,
                        ws,
                        &result.resource_id,
                        now,
                    )?;
                } else if let ApplyReviewDraft(x) = action {
                    result = review::read_job(&tx, root, fence, op, actor, ws, &x.job_id, now)?;
                } else if let EnqueueRunQuestion(x) = action {
                    review_questions::active(
                        &tx,
                        root,
                        fence,
                        op,
                        actor,
                        x.source.as_ref().ok_or(Error::InvalidArgument)?,
                        x.question
                            .as_ref()
                            .and_then(|q| q.scope.as_ref())
                            .ok_or(Error::InvalidArgument)?,
                        x.expected_epoch,
                        now,
                    )?;
                    result = question_pool::refresh(
                        &tx,
                        root,
                        fence,
                        op,
                        actor,
                        ws,
                        &result.resource_id,
                        now,
                    )?;
                } else if matches!(
                    action,
                    EnqueueQuestion(_) | RunQuestion(_) | CancelQuestion(_)
                ) {
                    result = question_pool::refresh(
                        &tx,
                        root,
                        fence,
                        op,
                        actor,
                        ws,
                        &result.resource_id,
                        now,
                    )?;
                } else if matches!(
                    action,
                    CreateLiveRun(_) | ControlLiveRun(_) | ControlLiveBatch(_)
                ) {
                    result = live::read(&tx, root, fence, op, actor, ws, &result.resource_id, now)?;
                } else if let StartLiveAuto(x) = action {
                    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
                    result = live_auto::read(&tx, root, fence, op, actor, ws, &r.run_id, now)?;
                } else if let StopLiveAuto(x) = action {
                    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
                    result = live_auto::read(&tx, root, fence, op, actor, ws, &r.run_id, now)?;
                } else if let PrepareLiveAuto(x) = action {
                    let r = x.run.as_ref().ok_or(Error::InvalidArgument)?;
                    result = tts_task::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                    result.live_auto =
                        live_auto::read(&tx, root, fence, op, actor, ws, &r.run_id, now)?.live_auto;
                } else if let SynthesizePreviewSpeech(x) = action {
                    result = tts_task::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let TestTts(x) = action {
                    result = tts_task::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let CancelTts(x) = action {
                    result = tts_task::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let TestLlm(x) = action {
                    result = llm::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let StartLlm(x) = action {
                    result = llm::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let CancelLlm(x) = action {
                    result = llm::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let ResolveLlm(x) = action {
                    result = llm::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let SaveLlmModel(x) = action {
                    result = llm::settings(&tx, root, ws)?;
                    result.resource_id = x.model_id.clone();
                } else if let SaveTtsModel(x) = action {
                    result = llm::settings(&tx, root, ws)?;
                    result.resource_id = x.model_id.clone();
                } else if let DeleteLlmModel(x) | DeleteTtsModel(x) = action {
                    let revision = result.revision;
                    let sha = result.sha256;
                    result = llm::settings(&tx, root, ws)?;
                    result.resource_id = x.object_id.clone();
                    result.revision = revision;
                    result.sha256 = sha;
                } else if let SetLlmBudget(_) = action {
                    result = llm::settings(&tx, root, ws)?;
                } else if let AskFaq(x) = action {
                    result = assistance::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let ResolveFaq(x) = action {
                    result = assistance::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?;
                } else if let PrepareDelivery(x) = action {
                    result = delivery::read(&tx, root, ws, &x.delivery_id, actor, fence)?;
                } else if let Approve(x) = action {
                    result = delivery::read(&tx, root, ws, &x.delivery_id, actor, fence)?;
                } else if let Dispatch(x) = action {
                    result = delivery::read(&tx, root, ws, &x.delivery_id, actor, fence)?;
                } else if let Publish(x) = action {
                    result = publication::read(&tx, root, fence, op, ws, &x.snapshot_id, now)?;
                } else if let PreviewLiveSentence(x) = action {
                    result =
                        preview::read(&tx, root, fence, op, actor, ws, &x.preview_id, tick, now)?;
                } else if let Preview(x) = action {
                    result =
                        preview::read(&tx, root, fence, op, actor, ws, &x.preview_id, tick, now)?;
                } else if let PreviewPresented(x) = action {
                    result =
                        preview::read(&tx, root, fence, op, actor, ws, &x.preview_id, tick, now)?;
                }
                op.check()?;
                tx.commit().wb()?;
                return Ok(result);
            }
        }
        let result = match action {
            PreviewSharedModels(_)
            | ReadSharedMigration(_)
            | MigrateSharedModels(_)
            | CancelSharedMigration(_)
            | PreviewWorkspacePurge(_)
            | ReadWorkspacePurge(_)
            | PurgeWorkspace(_)
            | WorkspaceContext(_) => return Err(Error::InvalidArgument),
            CreateWorkspaceSettings(x) => workspace_settings::apply(&tx, actor, now, x, true)?,
            EditWorkspaceSettings(x) => workspace_settings::apply(&tx, actor, now, x, false)?,
            CreateWorkspace(x) => {
                if workspace_lifecycle::state(&tx, ws)? == "workspace_delete" {
                    return Err(Error::RevisionConflict);
                }
                if x.name.trim().is_empty()
                    || x.name.len() > 128
                    || x.name.chars().any(char::is_control)
                    || x.budget_limit_minor > 1_000_000_000
                    || x.currency != "CNY"
                {
                    return Err(Error::InvalidArgument);
                }
                tx.execute(
                    "INSERT INTO workspaces VALUES(?1,?2,1,?3,?3,?4,'CNY')",
                    params![ws, x.name, now, int(x.budget_limit_minor)?],
                )
                .wb()?;
                tx.execute(
                    "INSERT INTO roles VALUES(?1,?2,?3,1,?4,?4)",
                    params![ws, actor, FoundationRole::Admin as i32, now],
                )
                .wb()?;
                FoundationResult {
                    resource_id: ws.into(),
                    revision: 1,
                    ..Default::default()
                }
            }
            WorkspaceLifecycle(x) => workspace_lifecycle::apply(&tx, root, fence, actor, now, x)?,
            RenameWorkspace(x) => {
                let name = x.name.trim();
                if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
                    return Err(Error::InvalidArgument);
                }
                if tx.execute("UPDATE workspaces SET name=?1,revision=revision+1,updated_at=?2 WHERE id=?3 AND revision=?4",params![name,now,ws,int(x.expected_revision)?]).wb()? != 1 {
                    return Err(Error::RevisionConflict);
                }
                FoundationResult {
                    resource_id: ws.into(),
                    revision: x.expected_revision + 1,
                    ..Default::default()
                }
            }
            Grant(x) => {
                uuid(&x.principal_id)?;
                let new = FoundationRole::try_from(x.role).map_err(|_| Error::InvalidArgument)?;
                if x.principal_id == *root_owner && new != FoundationRole::Admin {
                    return Err(Error::PermissionDenied);
                }
                let current: Option<u64> = tx
                    .query_row(
                        "SELECT revision FROM roles WHERE workspace_id=?1 AND principal_id=?2",
                        params![ws, x.principal_id],
                        |r| uint(r, 0),
                    )
                    .optional()
                    .wb()?;
                if current.unwrap_or(0) != x.expected_revision {
                    return Err(Error::RevisionConflict);
                }
                if let Some(old) = current {
                    root::revoke(root, fence, &role_scope(ws, &x.principal_id, old))?;
                }
                tx.execute("INSERT INTO roles VALUES(?1,?2,?3,1,?4,?4) ON CONFLICT(workspace_id,principal_id) DO UPDATE SET role=excluded.role,revision=roles.revision+1,updated_at=excluded.updated_at",params![ws,x.principal_id,x.role,now]).wb()?;
                tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE workspace_id=?3 AND recipient=?4 AND state IN (?5,?6)",params![DeliveryState::Cancelled as i32,now,ws,x.principal_id,DeliveryState::AwaitingApproval as i32,DeliveryState::Approved as i32]).wb()?;
                tx.execute("UPDATE outbox SET state='cancelled',revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND state='pending' AND resource_id IN(SELECT id FROM deliveries WHERE workspace_id=?2 AND recipient=?3 AND state=?4)",params![now,ws,x.principal_id,DeliveryState::Cancelled as i32]).wb()?;
                tx.execute(
                    "DELETE FROM approvals WHERE workspace_id=?1 AND principal_id=?2",
                    params![ws, x.principal_id],
                )
                .wb()?;
                FoundationResult {
                    resource_id: x.principal_id.clone(),
                    revision: x.expected_revision + 1,
                    ..Default::default()
                }
            }
            PutObject(x) | PutPreparation(x) => {
                uuid(&x.object_id)?;
                fence.check_scope(&object_scope(ws, &x.object_id))?;
                manual_guard(&x.body)?;
                let draft_kind = if matches!(action, PutPreparation(_)) {
                    Some(nai_runtime_core::preparation::validate(op, &x.body)?)
                } else {
                    if nai_runtime_core::preparation::kind(&x.body).is_some() {
                        return Err(Error::InvalidArgument);
                    }
                    None
                };
                if draft_kind.as_deref() == Some("manual_run_draft") {
                    manual_run::validate(&tx, root, fence, op, x, now)?;
                }
                let current: Option<(u64, bool)> = tx
                    .query_row(
                        "SELECT revision,deleted FROM objects WHERE workspace_id=?1 AND id=?2",
                        params![ws, x.object_id],
                        |r| Ok((uint(r, 0)?, r.get(1)?)),
                    )
                    .optional()
                    .wb()?;
                if x.expected_revision >= 1024 {
                    return Err(Error::ResourceExhausted);
                }
                if current.as_ref().is_some_and(|v| v.1)
                    || current.map(|v| v.0).unwrap_or(0) != x.expected_revision
                {
                    return Err(Error::RevisionConflict);
                }
                if x.expected_revision > 0 {
                    let (_, old_hash) = object(&tx, ws, &x.object_id, x.expected_revision)?;
                    if nai_runtime_core::preparation::kind(&body(root, &old_hash)?) != draft_kind {
                        return Err(Error::RevisionConflict);
                    }
                }
                if draft_kind.as_deref() == Some("avatar_profile_draft") {
                    avatar_draft::prepare(&tx, root, fence, op, actor, x, now)?;
                }
                if draft_kind.as_deref() == Some("avatar_photo_draft") {
                    photo_draft::prepare(&tx, root, op, x)?;
                }
                if draft_kind.as_deref() == Some("avatar_video_draft") {
                    video_draft::prepare(&tx, root, op, x)?;
                }
                let revision = x
                    .expected_revision
                    .checked_add(1)
                    .ok_or(Error::ResourceExhausted)?;
                let hash = store_body(root, x.body.as_bytes())?;
                tx.execute("INSERT INTO objects VALUES(?1,?2,1,0,?3,?3) ON CONFLICT(workspace_id,id) DO UPDATE SET revision=revision+1,updated_at=excluded.updated_at",params![ws,x.object_id,now]).wb()?;
                tx.execute(
                    "INSERT INTO object_versions VALUES(?1,?2,?3,?4,?5,?5)",
                    params![ws, x.object_id, int(revision)?, hash, now],
                )
                .wb()?;
                FoundationResult {
                    resource_id: x.object_id.clone(),
                    revision,
                    sha256: hash,
                    ..Default::default()
                }
            }
            ReadObject(x) => {
                fence
                    .check_scope(&object_scope(ws, &x.object_id))
                    .map_err(|_| Error::NotFound)?;
                let (latest, d) = object(&tx, ws, &x.object_id, x.revision)?;
                let content = body(root, &d)?;
                if nai_runtime_core::preparation::kind(&content)
                    .as_deref()
                    .is_some_and(|k| k.starts_with("publication_review_"))
                {
                    return Err(Error::PermissionDenied);
                }
                if nai_runtime_core::preparation::kind(&content).as_deref()
                    == Some("publication_question_index")
                {
                    return Err(Error::PermissionDenied);
                }
                if nai_runtime_core::preparation::kind(&content)
                    .as_deref()
                    .is_some_and(|k| {
                        matches!(k, "publication_llm_config" | "publication_tts_config")
                    })
                {
                    let mut r = llm::settings(&tx, root, ws)?;
                    r.resource_id = x.object_id.clone();
                    r
                } else if matches!(
                    nai_runtime_core::preparation::kind(&content).as_deref(),
                    Some("publication_tts_test" | "publication_tts_preview")
                ) {
                    tts_task::read(&tx, root, fence, op, actor, ws, &x.object_id, now)?
                } else if matches!(
                    nai_runtime_core::preparation::kind(&content).as_deref(),
                    Some("publication_llm_task" | "publication_llm_test")
                ) {
                    llm::read(&tx, root, fence, op, actor, ws, &x.object_id, now)?
                } else if nai_runtime_core::preparation::kind(&content).as_deref()
                    == Some("publication_live_run")
                {
                    live::read(&tx, root, fence, op, actor, ws, &x.object_id, now)?
                } else if nai_runtime_core::preparation::kind(&content).as_deref()
                    == Some("publication_question_task")
                {
                    question_pool::refresh(&tx, root, fence, op, actor, ws, &x.object_id, now)?
                } else if nai_runtime_core::preparation::kind(&content).as_deref()
                    == Some("publication_faq_task")
                {
                    assistance::read(&tx, root, fence, op, actor, ws, &x.object_id, now)?
                } else if nai_runtime_core::preparation::kind(&content).as_deref()
                    == Some("publication_private_preview")
                {
                    preview::read(&tx, root, fence, op, actor, ws, &x.object_id, tick, now)?
                } else if nai_runtime_core::preparation::kind(&content).as_deref()
                    == Some("publication_snapshot")
                {
                    publication::read(&tx, root, fence, op, ws, &x.object_id, now)?
                } else {
                    FoundationResult {
                        resource_id: x.object_id.clone(),
                        revision: if x.revision == 0 { latest } else { x.revision },
                        body: content,
                        sha256: d,
                        ..Default::default()
                    }
                }
            }
            PublicationImpact(x) => {
                publication::impact(&tx, root, fence, op, actor, role_rev, now, x)?
            }
            Publish(x) => publication::publish(&tx, root, fence, op, actor, role_rev, now, x)?,
            ReadPublication(x) => publication::read(&tx, root, fence, op, ws, &x.snapshot_id, now)?,
            RevokePublication(x) => publication::revoke(&tx, root, fence, actor, now, x)?,
            ListWorkspaces(x) => browse::workspaces(&tx, fence, actor, x)?,
            ListPreparation(x) => browse::preparation(&tx, root, fence, x)?,
            Preview(x) => {
                preview::submit(&tx, root, fence, op, actor, role_rev, tick, now, x, None)?
            }
            PreviewLiveSentence(x) => {
                live::preview_sentence(&tx, root, fence, op, actor, role_rev, tick, now, x)?
            }
            ControlLiveBatch(x) => live::batch(&tx, root, fence, op, actor, x, now)?,
            ReadPreview(x) => {
                preview::read(&tx, root, fence, op, actor, ws, &x.preview_id, tick, now)?
            }
            PreviewPresented(x) => preview::presented(&tx, root, fence, op, actor, tick, now, x)?,
            StopPreview(x) => preview::stop(&tx, actor, now, x)?,
            SaveLlmModel(x) => llm::save(&tx, root, op, vault, actor, now, x)?,
            SaveTtsModel(x) => {
                model_catalog::save_tts(&tx, root, op, self.tts_vault.as_deref(), actor, now, x)?
            }
            DeleteLlmModel(x) => {
                model_catalog::remove(&tx, root, op, now, x, "publication_llm_config")?
            }
            DeleteTtsModel(x) => {
                model_catalog::remove(&tx, root, op, now, x, "publication_tts_config")?
            }
            ReadLlmSettings(_) => llm::settings(&tx, root, ws)?,
            SetLlmBudget(x) => llm::set_budget(&tx, x, now, actor)?,
            CheckLlm(x) => llm::check(&tx, root, fence, op, vault, actor, role_rev, now, x)?,
            AuthorizeTtsAudio(_)
            | RevokeTtsAudio(_)
            | StartTtsAudition(_)
            | RenewTtsAudition(_)
            | StopTtsAudition(_)
            | SelectOutputDevice(_) => return Err(Error::CapabilityUnavailable),
            CheckPublicCandidate(_)
            | ConfirmPublicApproval(_)
            | RevokePublicApproval(_)
            | StartPublicExecution(_)
            | RenewPublicExecution(_)
            | StopPublicExecution(_)
            | PreparePublicWindow(_) => {
                return Err(Error::CapabilityUnavailable);
            }
            PreparePublicOutputBinding(_) => return Err(Error::CapabilityUnavailable),
            ConfirmLiveAuto(_) => return Err(Error::CapabilityUnavailable),
            ControlLiveTakeover(x) => live_takeover::control(&tx, root, fence, op, actor, x, now)?,
            ReadReview(x) => review::read(&tx, root, fence, op, actor, x, now)?,
            SaveReviewNote(x) => review::save_note(&tx, root, fence, op, actor, x, now)?,
            PlanReviewDraft(x) => review::plan(&tx, root, fence, op, actor, x, now)?,
            ReadReviewDraft(x) => {
                review::read_job(&tx, root, fence, op, actor, ws, &x.job_id, now)?
            }
            ApplyReviewDraft(x) => review::apply(&tx, root, fence, op, actor, x, now)?,
            ControlLiveQuestion(x) => live_question::control(&tx, root, fence, op, actor, x, now)?,
            StartLiveAuto(x) => live_auto::start(&tx, root, fence, op, actor, x, now)?,
            PrepareLiveAuto(x) => {
                live_auto::prepare(&tx, root, fence, op, actor, role_rev, tick, x, now)?
            }
            StopLiveAuto(x) => live_auto::stop(&tx, root, fence, op, actor, x, now)?,
            ReadLiveAuto(x) => live_auto::read(&tx, root, fence, op, actor, ws, &x.run_id, now)?,
            CheckPreviewSpeech(x) => FoundationResult {
                speech_plan: Some(tts_task::speech_plan(
                    &tx, root, fence, op, actor, x, now, true,
                )?),
                ..Default::default()
            },
            SynthesizePreviewSpeech(x) => {
                let p = x.plan.as_ref().ok_or(Error::InvalidArgument)?;
                let request = FoundationTtsTest {
                    workspace_id: p.workspace_id.clone(),
                    task_id: x.task_id.clone(),
                    model_id: p.model_id.clone(),
                    model_revision: p.model_revision,
                };
                tts_task::start(
                    &tx,
                    root,
                    fence,
                    op,
                    actor,
                    role_rev,
                    now,
                    &request,
                    Some(x),
                    None,
                )?
            }
            TestTts(x) => {
                tts_task::start(&tx, root, fence, op, actor, role_rev, now, x, None, None)?
            }
            ReadTtsTask(x) => tts_task::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?,
            CancelTts(x) => tts_task::cancel(&tx, root, fence, actor, now, x)?,
            TestLlm(x) => llm::test(&tx, root, fence, op, actor, role_rev, now, x)?,
            StartLlm(x) => llm::start(&tx, root, fence, op, actor, role_rev, now, x)?,
            ReadLlmTask(x) => llm::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?,
            CancelLlm(x) => {
                llm::cancel(&tx, root, actor, fence, now, x)?;
                llm::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?
            }
            ResolveLlm(x) => llm::resolve(&tx, root, fence, op, actor, now, x)?,
            CreateLiveRun(x) => live::create(&tx, root, fence, op, actor, role_rev, x, now)?,
            ReadLiveRun(x) => live::read(&tx, root, fence, op, actor, ws, &x.run_id, now)?,
            ReadLiveSpeechPlan(x) => {
                live::speech_plan(&tx, root, fence, op, actor, ws, &x.run_id, now)?
            }
            ControlLiveRun(x) => live::control(&tx, root, fence, op, actor, x, now)?,
            ReadQuestionPool(x) => question_pool::read(&tx, root, fence, op, actor, x, now)?,
            EnqueueQuestion(x) => question_pool::enqueue(&tx, root, fence, op, actor, x, now)?,
            EnqueueRunQuestion(x) => {
                question_pool::enqueue_run(&tx, root, fence, op, actor, x, now)?
            }
            ReadRunQuestionPool(x) => question_pool::read_run(&tx, root, fence, op, actor, x, now)?,
            RunQuestion(x) => question_pool::run(&tx, root, fence, op, actor, x, now)?,
            CancelQuestion(x) => question_pool::cancel(&tx, root, fence, op, actor, x, now)?,
            AskFaq(x) => assistance::ask(&tx, root, fence, op, actor, role_rev, now, x)?,
            ReadFaq(x) => assistance::read(&tx, root, fence, op, actor, ws, &x.task_id, now)?,
            ResolveFaq(x) => assistance::resolve(&tx, root, fence, op, actor, now, x)?,
            CreateSession(x) => {
                uuid(&x.session_id)?;
                fence.check(fence.generation())?;
                tx.execute(
                    "INSERT INTO sessions VALUES(?1,?2,?3,1,0,1,?4,?4)",
                    params![ws, x.session_id, actor, now],
                )
                .wb()?;
                FoundationResult {
                    resource_id: x.session_id.clone(),
                    revision: 1,
                    epoch: 1,
                    ..Default::default()
                }
            }
            PrepareDelivery(x) => {
                delivery::prepare(&tx, root, fence, tick, now, actor, role_rev, x)?
            }
            Approve(x) => delivery::approve(&tx, root, fence, tick, now, actor, x)?,
            Dispatch(x) => delivery::claim(&tx, root, fence, tick, now, actor, x)?,
            ReadDelivery(x) => delivery::read(&tx, root, ws, &x.delivery_id, actor, fence)?,
            Stop(x) => {
                uuid(&x.session_id)?;
                let cancelled: u32 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM deliveries WHERE state IN (?1,?2)",
                        params![
                            DeliveryState::AwaitingApproval as i32,
                            DeliveryState::Approved as i32
                        ],
                        |r| r.get(0),
                    )
                    .wb()?;
                let submitted: u32 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM deliveries WHERE state=?1",
                        params![DeliveryState::Delivering as i32],
                        |r| r.get(0),
                    )
                    .wb()?;
                let in_flight: u32 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM deliveries WHERE state=?1",
                        params![DeliveryState::Committed as i32],
                        |r| r.get(0),
                    )
                    .wb()?;
                let unknown: u32 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM deliveries WHERE state=?1",
                        params![DeliveryState::OutcomeUnknown as i32],
                        |r| r.get(0),
                    )
                    .wb()?;
                tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE state=?3",params![DeliveryState::OutcomeUnknown as i32,now,DeliveryState::Committed as i32]).wb()?;
                tx.execute("UPDATE outbox SET state='outcome_unknown',revision=revision+1,updated_at=?1 WHERE state='dispatching'",params![now]).wb()?;
                tx.execute("UPDATE sessions SET stopped=1,epoch=epoch+1,revision=revision+1,updated_at=?1 WHERE stopped=0",params![now]).wb()?;
                tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE state IN (?3,?4)",params![DeliveryState::Cancelled as i32,now,DeliveryState::AwaitingApproval as i32,DeliveryState::Approved as i32]).wb()?;
                tx.execute("UPDATE outbox SET state='cancelled',revision=revision+1,updated_at=?1 WHERE state='pending'",params![now]).wb()?;
                tx.execute("DELETE FROM approvals", []).wb()?;
                FoundationResult {
                    resource_id: x.session_id.clone(),
                    cancelled_pending: cancelled,
                    already_submitted: submitted,
                    outcome_unknown_count: unknown,
                    dispatch_in_flight: in_flight,
                    ..Default::default()
                }
            }
            DeleteObject(x) => {
                uuid(&x.object_id)?;
                let (latest, sha) = object(&tx, ws, &x.object_id, 0)?;
                if nai_runtime_core::preparation::kind(&body(root, &sha)?).is_some_and(|kind| {
                    kind.starts_with("publication_")
                        || kind == "avatar_photo_draft"
                        || kind == "avatar_video_draft"
                }) {
                    return Err(Error::PermissionDenied);
                }
                if latest != x.expected_revision {
                    return Err(Error::RevisionConflict);
                }
                root::revoke(root, fence, &object_scope(ws, &x.object_id))?;
                let changed=tx.execute("UPDATE objects SET deleted=1,revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND id=?3 AND revision=?4 AND deleted=0",params![now,ws,x.object_id,int(x.expected_revision)?]).wb()?;
                if changed != 1 {
                    return Err(Error::RevisionConflict);
                }
                tx.execute("UPDATE deliveries SET state=?1,revision=revision+1,updated_at=?2 WHERE workspace_id=?3 AND object_id=?4 AND state IN (?5,?6)",params![DeliveryState::Cancelled as i32,now,ws,x.object_id,DeliveryState::AwaitingApproval as i32,DeliveryState::Approved as i32]).wb()?;
                tx.execute("UPDATE outbox SET state='cancelled',revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND resource_id IN(SELECT id FROM deliveries WHERE workspace_id=?2 AND object_id=?3) AND state='pending'",params![now,ws,x.object_id]).wb()?;
                FoundationResult {
                    resource_id: x.object_id.clone(),
                    revision: x.expected_revision + 1,
                    ..Default::default()
                }
            }
        };
        if matches!(action, RunQuestion(_) | CancelQuestion(_))
            && let Some(e) = result
                .question_pool
                .as_ref()
                .and_then(|p| p.entries.first())
        {
            review_questions::progress(&tx, root, actor, ws, e, now)?;
        }
        if let ResolveFaq(x) = action {
            review_questions::resolution(&tx, root, actor, x, now)?;
        }
        if !read {
            audit(
                &tx,
                ws,
                actor,
                match action {
                    CreateWorkspace(_) => "create_workspace",
                    CreateWorkspaceSettings(_) => "create_workspace_settings",
                    EditWorkspaceSettings(_) => "edit_workspace_settings",
                    RenameWorkspace(_) => "rename_workspace",
                    WorkspaceLifecycle(_) => "workspace_lifecycle",
                    Grant(_) => "grant",
                    PutObject(_) => "put_object",
                    PutPreparation(_) => "put_preparation",
                    PublicationImpact(_) => "publication_impact",
                    Publish(_) => "publish_private_content",
                    RevokePublication(_) => "revoke_product_fact_version",
                    Preview(_) => "preview_private_submit",
                    PreviewPresented(_) => "preview_frontend_presented",
                    StopPreview(_) => "stop_private_preview",
                    SaveLlmModel(_) => "llm_model_save",
                    SaveTtsModel(_) => "tts_model_save",
                    DeleteLlmModel(_) => "llm_model_remove",
                    DeleteTtsModel(_) => "tts_model_remove",
                    SetLlmBudget(_) => "llm_budget_set",
                    StartLlm(_) => "llm_task_reserve",
                    TestLlm(_) => "llm_test_reserve",
                    TestTts(_) => "tts_test_reserve",
                    SynthesizePreviewSpeech(_) => "tts_preview_approve_reserve",
                    CancelTts(_) => "tts_task_cancel",
                    CancelLlm(_) => "llm_task_cancel",
                    ResolveLlm(_) => "llm_task_resolve",
                    CreateLiveRun(_) => "live_run_create",
                    ControlLiveQuestion(_) => "live_question_control",
                    ControlLiveTakeover(_) => "live_takeover_control",
                    SaveReviewNote(_) => "review_note_save",
                    PlanReviewDraft(_) => "review_draft_plan",
                    ApplyReviewDraft(_) => "review_draft_apply",
                    StartLiveAuto(_) => "live_auto_start",
                    PrepareLiveAuto(_) => "live_auto_prepare",
                    StopLiveAuto(_) => "live_auto_stop",
                    PreviewLiveSentence(_) => "live_sentence_preview",
                    ControlLiveBatch(_) => "live_batch_control",
                    ControlLiveRun(_) => "live_run_control",
                    EnqueueRunQuestion(_) => "question_enqueue_for_run",
                    EnqueueQuestion(_) => "question_enqueued",
                    RunQuestion(_) => "question_run",
                    CancelQuestion(_) => "question_cancel",
                    AskFaq(_) => "faq_lookup",
                    ResolveFaq(_) => "faq_resolve",
                    CreateSession(_) => "create_session",
                    PrepareDelivery(_) => "prepare_delivery",
                    Approve(_) => "approve",
                    Dispatch(_) => "dispatch_intent",
                    Stop(_) => "stop_all",
                    DeleteObject(_) => "delete_object",
                    _ => unreachable!(),
                },
                &result.resource_id,
                now,
            )?;
            tx.execute(
                "INSERT INTO command_results VALUES(?1,?2,?3,?4,1,?5,?5)",
                params![
                    actor,
                    cmd.idempotency_key,
                    digest,
                    result.encode_to_vec(),
                    now
                ],
            )
            .wb()?;
        }
        op.check()?;
        let preview_id = match action {
            Preview(x) => Some(&x.preview_id),
            PreviewLiveSentence(x) => Some(&x.preview_id),
            PreviewPresented(x) => Some(&x.preview_id),
            _ => None,
        };
        if let Some(id) = preview_id {
            let current = preview::read(
                &tx,
                root,
                fence,
                op,
                actor,
                ws,
                id,
                u64::try_from(origin.elapsed().as_millis()).map_err(|_| Error::DeadlineExceeded)?,
                utc()?,
            )?;
            if current.state != DeliveryState::Delivering as i32 {
                return Err(Error::Cancelled);
            }
        }
        if matches!(
            action,
            EnqueueQuestion(_) | EnqueueRunQuestion(_) | RunQuestion(_)
        ) {
            question_pool::commit_gate(
                &tx,
                root,
                fence,
                op,
                actor,
                ws,
                &result.resource_id,
                utc()?,
            )?;
        }
        if let RunQuestion(x) = action {
            assistance::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id, utc()?)?;
        }
        if let AskFaq(x) = action {
            assistance::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id, utc()?)?;
        }
        if let TestTts(x) = action {
            tts_task::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id)?;
        }
        if let SynthesizePreviewSpeech(x) = action {
            tts_task::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id)?;
        }
        if let TestLlm(x) = action {
            llm::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id, utc()?)?;
        }
        if let StartLlm(x) = action {
            llm::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id, utc()?)?;
        }
        if let ResolveLlm(x) = action
            && matches!(
                FaqDecision::try_from(x.decision),
                Ok(FaqDecision::Accept | FaqDecision::Edit)
            )
        {
            llm::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id, utc()?)?;
        }
        if let ResolveFaq(x) = action
            && matches!(
                FaqDecision::try_from(x.decision),
                Ok(FaqDecision::Accept | FaqDecision::Edit)
            )
        {
            assistance::commit_gate(&tx, root, fence, op, actor, ws, &x.task_id, utc()?)?;
        }
        if let Publish(x) = action {
            let commit_time = utc()?;
            publication::report_time(&tx, root, ws, &x.report_id, commit_time)?;
            let current = publication::read(&tx, root, fence, op, ws, &x.snapshot_id, commit_time)?;
            if current.publication_status != PublicationStatus::PrivateContent as i32 {
                return Err(Error::CapabilityUnavailable);
            }
        }
        if matches!(action, PrepareDelivery(_) | Approve(_) | Dispatch(_)) {
            fence.check(fence.generation())?;
            let id = match action {
                PrepareDelivery(x) => &x.delivery_id,
                Approve(x) => &x.delivery_id,
                Dispatch(x) => &x.delivery_id,
                _ => unreachable!(),
            };
            delivery::validate_current(
                &tx,
                root,
                fence,
                u64::try_from(origin.elapsed().as_millis()).map_err(|_| Error::DeadlineExceeded)?,
                actor,
                ws,
                id,
                !matches!(action, PrepareDelivery(_)),
            )?;
        }
        tx.commit().wb()?;
        if matches!(action, DeleteObject(_)) {
            self.collect_unreferenced()?;
        }
        Ok(result)
    }
    /// Apply the real local private-card read model, only after a committed dispatch intent.
    /// It is Submitted, not a UI-visible or user-read completion receipt.
    pub fn project_private_card(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
    ) -> Result<FoundationResult> {
        self.begin(op)?;
        let tick = self.tick();
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        let result = delivery::project(&tx, &self.root, &self.fence, tick, now, actor, ws, id)?;
        op.check()?;
        delivery::validate_current(
            &tx,
            &self.root,
            &self.fence,
            u64::try_from(self.origin.elapsed().as_millis())
                .map_err(|_| Error::DeadlineExceeded)?,
            actor,
            ws,
            id,
            true,
        )?;
        tx.commit().wb()?;
        Ok(result)
    }
}
mod live_auto;
mod live_question;
mod live_takeover;
#[cfg(test)]
mod tests;

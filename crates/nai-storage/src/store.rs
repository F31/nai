use crate::{DataPaths, Operation, Result, error, hash, id, migration, sql_int, utc_ms};
use nai_contracts::v1::{
    Approval, AuthoritativeFact, ErrorCode, MemoryRecord, PerformanceCue, SessionSnapshot,
    authoritative_fact,
};
use prost::Message;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    fs::{File, OpenOptions},
    time::Duration,
};

pub struct Store {
    pub(crate) conn: Connection,
    pub(crate) paths: DataPaths,
    pub(crate) decisions: std::collections::VecDeque<crate::decision::BufferedDecision>,
    pub(crate) dropped_telemetry: u64,
    _owner: File,
}
#[derive(Clone, Copy)]
pub enum ConfigKind {
    Runtime,
    Provider,
}

/// 一条记忆记录在库里的样子：契约九字段 **加上** 撤销时刻。
///
/// 撤销时刻是**存储态**，不是契约字段 ⇒ 它 MUST NOT 被塞进 `MemoryRecord`
/// （那会给契约九字段第二处定义）。分开返回，是为了让「记录是什么」与
/// 「这条记录在库里处于什么状态」保持**两件不同的事**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMemoryRecord {
    /// 契约九字段，逐列还原。
    pub record: MemoryRecord,
    /// §42.4: 该行被阻断读取的时刻。`None` = 未被撤销。
    pub revoked_at_ms: Option<i64>,
}
pub use nai_contracts::candidate_hash;
pub(crate) fn validate_json(kind: ConfigKind, value: &serde_json::Value) -> Result<()> {
    let source = match kind {
        ConfigKind::Runtime => include_str!("../../../schemas/config/runtime.schema.json"),
        ConfigKind::Provider => include_str!("../../../schemas/config/provider.schema.json"),
    };
    let schema: serde_json::Value = serde_json::from_str(source)
        .map_err(|_| error(ErrorCode::Internal, "invalid embedded schema"))?;
    if !jsonschema::validator_for(&schema)
        .map_err(|_| error(ErrorCode::Internal, "schema compile failed"))?
        .is_valid(value)
    {
        return Err(error(
            ErrorCode::InvalidArgument,
            "configuration violates schema",
        ));
    }
    match kind {
        ConfigKind::Runtime => {
            if value["media"]["lease_expire_ms"].as_u64()
                <= value["media"]["lease_renew_ms"].as_u64()
                || value["media"]["startup_buffer_ms"].as_u64()
                    > value["media"]["max_buffer_ms"].as_u64()
            {
                return Err(error(ErrorCode::InvalidArgument, "invalid media intervals"));
            }
            value["budgets"]["per_session_limit_minor"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| error(ErrorCode::InvalidArgument, "budget range"))?;
        }
        ConfigKind::Provider => {
            let endpoint = value["endpoint"].as_str().unwrap_or_default();
            let authority = endpoint
                .split_once("://")
                .map(|(_, s)| s.split(['/', '?', '#']).next().unwrap_or_default())
                .unwrap_or_default();
            if authority.contains('@')
                || endpoint.split_once('?').is_some_and(|(_, query)| {
                    query.split('&').any(|pair| {
                        let key = pair
                            .split('=')
                            .next()
                            .unwrap_or_default()
                            .to_ascii_lowercase()
                            .replace(['_', '-'], "");
                        ["token", "secret", "apikey", "credential"]
                            .iter()
                            .any(|word| key.contains(word))
                    })
                })
            {
                return Err(error(ErrorCode::InvalidArgument, "credentials in endpoint"));
            }
        }
    }
    Ok(())
}

pub(crate) fn access(
    conn: &Connection,
    workspace: &str,
    actor: &str,
    permission: &str,
    resource: &str,
) -> Result<()> {
    let granted:i64=conn.query_row("SELECT COUNT(*) FROM permission_grants WHERE workspace_id=?1 AND actor_id=?2 AND permission=?3 AND resource_scope IN('*',?4) AND revoked_at_ms IS NULL",params![workspace,actor,permission,resource],|r|r.get(0))?;
    if granted == 0 {
        Err(error(
            ErrorCode::PermissionDenied,
            "scope permission missing",
        ))
    } else {
        Ok(())
    }
}
pub(crate) fn audit(
    conn: &Connection,
    workspace: &str,
    actor: &str,
    action: &str,
    object: &str,
    versions: &serde_json::Value,
) -> Result<()> {
    conn.execute("INSERT INTO audit_records(workspace_id,actor_id,action,object_hash,versions_json,created_at_ms,trace_id) VALUES(?1,?2,?3,?4,?5,?6,'storage-owner')",params![workspace,actor,action,hash(object.as_bytes()),versions.to_string(),utc_ms()?])?;
    Ok(())
}
pub(crate) fn recover(conn: &mut Connection, op: &Operation, restore: bool) -> Result<()> {
    op.check()?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = utc_ms()?;
    tx.execute("UPDATE sessions SET generation_epoch=generation_epoch+1,state_revision=state_revision+1,lifecycle=7,mode=2,ended_at_ms=?1 WHERE lifecycle<>7 OR ?2",params![now,restore])?;
    tx.execute(
        "UPDATE approvals SET revoked_at_ms=?1 WHERE revoked_at_ms IS NULL",
        [now],
    )?;
    tx.execute(
        "UPDATE utterances SET status=12 WHERE status NOT IN(11,12,13,14,15)",
        [],
    )?;
    tx.execute("UPDATE tts_cache SET valid=0", [])?;
    tx.execute("UPDATE session_snapshots SET binding_ref=NULL", [])?;
    if restore {
        tx.execute(
            "UPDATE permission_grants SET revoked_at_ms=?1 WHERE revoked_at_ms IS NULL",
            [now],
        )?;
        tx.execute(
            "UPDATE devices SET revoked_at_ms=?1 WHERE revoked_at_ms IS NULL",
            [now],
        )?;
    }
    op.check()?;
    tx.commit()?;
    Ok(())
}

impl Store {
    pub fn open(paths: DataPaths, op: &Operation) -> Result<Self> {
        op.check()?;
        paths.initialize()?;
        let lock_path = paths.root.join("db/owner.lock");
        crate::paths::no_links(&lock_path)?;
        let owner = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        owner.try_lock().map_err(|_| {
            error(
                ErrorCode::Conflict,
                "storage owner already present or lock unavailable",
            )
        })?;
        crate::paths::no_links(&paths.database())?;
        let mut conn = Connection::open(paths.database())?;
        conn.busy_timeout(Duration::from_millis(50))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "wal_autocheckpoint", 1000)?;
        let call = op.clone();
        conn.progress_handler(1000, Some(move || call.stopped()))?;
        migration::migrate(&mut conn, op)?;
        if conn
            .prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some()
        {
            return Err(error(ErrorCode::Conflict, "foreign key corruption"));
        }
        recover(&mut conn, op, false)?;
        Ok(Self {
            conn,
            paths,
            decisions: std::collections::VecDeque::new(),
            dropped_telemetry: 0,
            _owner: owner,
        })
    }
    pub(crate) fn start(&self, op: &Operation) -> Result<()> {
        op.check()?;
        let call = op.clone();
        self.conn
            .progress_handler(1000, Some(move || call.stopped()))?;
        Ok(())
    }
    pub fn create_workspace(&mut self, op: &Operation, workspace: &str, name: &str) -> Result<()> {
        self.start(op)?;
        id(workspace)?;
        id(name)?;
        self.conn.execute(
            "INSERT INTO workspaces(workspace_id,name) VALUES(?1,?2)",
            params![workspace, name],
        )?;
        Ok(())
    }
    /// Trusted owner administration, not an endpoint exposed to models/plugins.
    pub fn set_permission(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        permission: &str,
        resource: &str,
        granted: bool,
    ) -> Result<()> {
        self.start(op)?;
        for key in [workspace, actor, permission, resource] {
            id(key)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            "UPDATE workspaces SET permission_revision=permission_revision+1 WHERE workspace_id=?1",
            [workspace],
        )?;
        if changed == 0 {
            return Err(error(ErrorCode::InvalidArgument, "workspace absent"));
        }
        let revision: i64 = tx.query_row(
            "SELECT permission_revision FROM workspaces WHERE workspace_id=?1",
            [workspace],
            |r| r.get(0),
        )?;
        let revoked = if granted { None } else { Some(utc_ms()?) };
        tx.execute("INSERT INTO permission_grants VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(workspace_id,actor_id,permission,resource_scope) DO UPDATE SET revision=excluded.revision,revoked_at_ms=excluded.revoked_at_ms",params![workspace,actor,permission,resource,revision,revoked])?;
        if !granted {
            tx.execute("UPDATE approvals SET revoked_at_ms=?2 WHERE workspace_id=?1 AND revoked_at_ms IS NULL",params![workspace,utc_ms()?])?;
            tx.execute("UPDATE utterances SET status=12 WHERE workspace_id=?1 AND status NOT IN(11,12,13,14,15)",[workspace])?;
            tx.execute(
                "UPDATE tts_cache SET valid=0 WHERE workspace_id=?1",
                [workspace],
            )?;
            tx.execute("UPDATE sessions SET generation_epoch=generation_epoch+1,state_revision=state_revision+1 WHERE workspace_id=?1",[workspace])?;
            tx.execute("INSERT INTO invalidation_outbox(workspace_id,reason,resource_id,old_revision,new_revision,created_at_ms) VALUES(?1,'permission_revoked',?2,?3,?4,?5)",params![workspace,resource,(revision-1).to_string(),revision.to_string(),utc_ms()?])?;
        }
        audit(
            &tx,
            workspace,
            actor,
            if granted {
                "permission.granted"
            } else {
                "permission.revoked"
            },
            resource,
            &serde_json::json!({"revision":revision}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(())
    }
    pub fn save_config(
        &mut self,
        op: &Operation,
        scope: (&str, &str),
        kind: ConfigKind,
        name: &str,
        value: &serde_json::Value,
        expected: i64,
    ) -> Result<i64> {
        let (workspace, actor) = scope;
        self.start(op)?;
        id(name)?;
        validate_json(kind, value)?;
        let text = value.to_string();
        if text.len() > 65536 {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "configuration too large",
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, workspace, actor, "config.write", name)?;
        let (table, key) = match kind {
            ConfigKind::Runtime => ("runtime_profiles", "profile_id"),
            ConfigKind::Provider => ("provider_profiles", "provider_id"),
        };
        let current: i64 = tx
            .query_row(
                &format!("SELECT config_revision FROM {table} WHERE workspace_id=?1 AND {key}=?2"),
                params![workspace, name],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if current != expected {
            return Err(error(ErrorCode::Conflict, "config revision mismatch"));
        }
        let next = current
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "revision exhausted"))?;
        match kind {
            ConfigKind::Runtime => {
                if value["workspace_id"].as_str() != Some(workspace) {
                    return Err(error(
                        ErrorCode::InvalidArgument,
                        "configuration workspace mismatch",
                    ));
                }
                tx.execute("INSERT INTO runtime_profiles VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(workspace_id,profile_id) DO UPDATE SET edition=excluded.edition,deployment=excluded.deployment,config_revision=excluded.config_revision,config_json=excluded.config_json",params![workspace,name,value["edition"].as_str(),value["deployment"].as_str(),next,text])?;
            }
            ConfigKind::Provider => {
                if value["id"].as_str() != Some(name) {
                    return Err(error(ErrorCode::InvalidArgument, "provider id mismatch"));
                }
                tx.execute("INSERT INTO provider_profiles VALUES(?1,?2,?3,?4,?5,?6,'{}',?7,?8,?9) ON CONFLICT(workspace_id,provider_id) DO UPDATE SET protocol=excluded.protocol,endpoint=excluded.endpoint,model=excluded.model,credential_ref=excluded.credential_ref,price_revision=excluded.price_revision,config_revision=excluded.config_revision,config_json=excluded.config_json",params![workspace,name,value["protocol"].as_str(),value["endpoint"].as_str(),value["model"].as_str(),value["credential_ref"].as_str(),value["price_revision"].as_str(),next,text])?;
            }
        }
        audit(
            &tx,
            workspace,
            actor,
            "config.saved",
            name,
            &serde_json::json!({"revision":next}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(next)
    }
    pub fn load_config(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        kind: ConfigKind,
        name: &str,
    ) -> Result<serde_json::Value> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "config.read", name)?;
        let (table, key) = match kind {
            ConfigKind::Runtime => ("runtime_profiles", "profile_id"),
            ConfigKind::Provider => ("provider_profiles", "provider_id"),
        };
        let text: String = self.conn.query_row(
            &format!("SELECT config_json FROM {table} WHERE workspace_id=?1 AND {key}=?2"),
            params![workspace, name],
            |r| r.get(0),
        )?;
        let value = serde_json::from_str(&text)
            .map_err(|_| error(ErrorCode::Internal, "corrupt configuration"))?;
        validate_json(kind, &value)?;
        op.check()?;
        Ok(value)
    }
    pub fn create_session(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        session: &str,
    ) -> Result<()> {
        self.start(op)?;
        id(session)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        self.conn.execute(
            "INSERT INTO sessions(session_id,workspace_id) VALUES(?1,?2)",
            params![session, workspace],
        )?;
        Ok(())
    }
    pub fn session(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        session: &str,
    ) -> Result<SessionSnapshot> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let mut value=self.conn.query_row("SELECT lifecycle,mode,generation_epoch,state_revision FROM sessions WHERE workspace_id=?1 AND session_id=?2",params![workspace,session],|r|Ok(SessionSnapshot{session_id:session.into(),workspace_id:workspace.into(),lifecycle:r.get(0)?,mode:r.get(1)?,media_state:0,generation_epoch:r.get::<_,i64>(2)? as u64,state_revision:r.get::<_,i64>(3)? as u64,..Default::default()}))?;
        value.knowledge_snapshot_ref=self.conn.query_row("SELECT snapshot_id FROM session_knowledge_bindings WHERE workspace_id=?1 AND session_id=?2",params![workspace,session],|row|row.get(0)).optional()?.unwrap_or_default();
        op.check()?;
        Ok(value)
    }
    /// Persist revocation after local media mute. CAS failure never authorizes output.
    pub fn checkpoint_revocation(
        &mut self,
        op: &Operation,
        actor: &str,
        previous: &SessionSnapshot,
        revoked: &SessionSnapshot,
    ) -> Result<()> {
        self.start(op)?;
        if previous.session_id != revoked.session_id
            || previous.workspace_id != revoked.workspace_id
            || revoked.generation_epoch <= previous.generation_epoch
            || revoked.state_revision <= previous.state_revision
            || revoked.media_state != nai_contracts::v1::MediaState::Muted as i32
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid revocation checkpoint",
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, &revoked.workspace_id, actor, "session.manage", "*")?;
        let changed = tx.execute(
            "UPDATE sessions SET generation_epoch=?1,state_revision=?2 WHERE workspace_id=?3 AND session_id=?4 AND generation_epoch=?5 AND state_revision=?6",
            params![sql_int(revoked.generation_epoch)?, sql_int(revoked.state_revision)?, revoked.workspace_id, revoked.session_id, sql_int(previous.generation_epoch)?, sql_int(previous.state_revision)?],
        )?;
        if changed != 1 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "session checkpoint CAS conflict",
            ));
        }
        let now = utc_ms()?;
        tx.execute("UPDATE approvals SET revoked_at_ms=?1 WHERE workspace_id=?2 AND session_id=?3 AND revoked_at_ms IS NULL", params![now,revoked.workspace_id,revoked.session_id])?;
        tx.execute("UPDATE utterances SET status=12 WHERE workspace_id=?1 AND session_id=?2 AND status NOT IN(11,12,13,14,15)", params![revoked.workspace_id,revoked.session_id])?;
        audit(
            &tx,
            &revoked.workspace_id,
            actor,
            "session.revoked",
            &revoked.session_id,
            &serde_json::json!({"epoch":revoked.generation_epoch.to_string()}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(())
    }
    pub fn publish_fact(
        &mut self,
        op: &Operation,
        actor: &str,
        input: &AuthoritativeFact,
        expected: Option<&str>,
    ) -> Result<AuthoritativeFact> {
        self.start(op)?;
        for key in [
            &input.workspace_id,
            &input.product_id,
            &input.field_key,
            &input.authority_ref,
        ] {
            id(key)?;
        }
        validate_fact(input)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(
            &tx,
            &input.workspace_id,
            actor,
            "knowledge.publish",
            &input.product_id,
        )?;
        let current:Option<String>=tx.query_row("SELECT revision FROM facts WHERE workspace_id=?1 AND product_id=?2 AND field_key=?3",params![input.workspace_id,input.product_id,input.field_key],|r|r.get(0)).optional()?;
        if current.as_deref() != expected {
            return Err(error(ErrorCode::Conflict, "fact revision mismatch"));
        }
        let next = current
            .as_deref()
            .unwrap_or("0")
            .parse::<i64>()
            .ok()
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "fact revision exhausted"))?;
        let permission_revision: i64 = tx.query_row(
            "SELECT permission_revision FROM workspaces WHERE workspace_id=?1",
            [&input.workspace_id],
            |r| r.get(0),
        )?;
        let mut fact = input.clone();
        fact.revision = next.to_string();
        fact.publisher_ref = actor.into();
        fact.permission_revision = permission_revision.to_string();
        let bytes = fact.encode_to_vec();
        if bytes.len() > 65536 {
            return Err(error(ErrorCode::ResourceExhausted, "fact too large"));
        }
        tx.execute("INSERT INTO facts VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(workspace_id,product_id,field_key) DO UPDATE SET revision=excluded.revision,fact_proto=excluded.fact_proto,valid_from_ms=excluded.valid_from_ms,valid_until_ms=excluded.valid_until_ms,authority_ref=excluded.authority_ref,permission_revision=excluded.permission_revision",params![fact.workspace_id,fact.product_id,fact.field_key,fact.revision,bytes,fact.valid_from_utc_ms.map(sql_int).transpose()?,fact.valid_until_utc_ms.map(sql_int).transpose()?,fact.authority_ref,permission_revision])?;
        tx.execute(
            "INSERT INTO fact_revisions VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                fact.workspace_id,
                fact.product_id,
                fact.field_key,
                fact.revision,
                bytes,
                utc_ms()?
            ],
        )?;
        if let Some(old) = current {
            tx.execute("UPDATE utterances SET status=12 WHERE workspace_id=?1 AND utterance_id IN(SELECT utterance_id FROM evidence_links WHERE workspace_id=?1 AND product_id=?2 AND field_key=?3 AND fact_revision=?4) AND status NOT IN(11,12,13,14,15)",params![fact.workspace_id,fact.product_id,fact.field_key,old])?;
            tx.execute("UPDATE approvals SET revoked_at_ms=?2 WHERE workspace_id=?1 AND candidate_hash IN(SELECT text_hash FROM utterances WHERE workspace_id=?1 AND status=12) AND revoked_at_ms IS NULL",params![fact.workspace_id,utc_ms()?])?;
            // Conservative workspace invalidation until the D09 cache dependency index exists.
            tx.execute(
                "UPDATE tts_cache SET valid=0 WHERE workspace_id=?1",
                [&fact.workspace_id],
            )?;
            tx.execute("INSERT INTO invalidation_outbox(workspace_id,reason,resource_id,old_revision,new_revision,created_at_ms) VALUES(?1,'fact_changed',?2,?3,?4,?5)",params![fact.workspace_id,fact.product_id,old,fact.revision,utc_ms()?])?;
        }
        audit(
            &tx,
            &fact.workspace_id,
            actor,
            "fact.published",
            &format!("{}:{}", fact.product_id, fact.field_key),
            &serde_json::json!({"revision":fact.revision}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(fact)
    }
    pub fn fact(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        product: &str,
        field: &str,
    ) -> Result<AuthoritativeFact> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "knowledge.read", product)?;
        let bytes:Option<Vec<u8>>=self.conn.query_row("SELECT fact_proto FROM facts WHERE workspace_id=?1 AND product_id=?2 AND field_key=?3 AND (valid_from_ms IS NULL OR valid_from_ms<=?4) AND (valid_until_ms IS NULL OR valid_until_ms>?4)",params![workspace,product,field,utc_ms()?],|r|r.get(0)).optional()?;
        let fact = AuthoritativeFact::decode(
            bytes
                .ok_or_else(|| error(ErrorCode::EvidenceExpired, "fact absent or expired"))?
                .as_slice(),
        )
        .map_err(|_| error(ErrorCode::Internal, "corrupt fact"))?;
        op.check()?;
        Ok(fact)
    }
    pub fn record_candidate(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        cue: &PerformanceCue,
        dependencies: &[(&str, &str, &str)],
    ) -> Result<String> {
        self.start(op)?;
        if dependencies.len() > 32 || cue.encode_to_vec().len() > 65536 {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "candidate dependency limit",
            ));
        }
        if cue.expires_at_utc_ms <= utc_ms()? as u64 {
            return Err(error(ErrorCode::DeadlineExceeded, "candidate expired"));
        }
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let epoch: i64 = self.conn.query_row(
            "SELECT generation_epoch FROM sessions WHERE workspace_id=?1 AND session_id=?2",
            params![workspace, cue.session_id],
            |r| r.get(0),
        )?;
        if sql_int(cue.generation_epoch)? != epoch {
            return Err(error(ErrorCode::StaleEpoch, "candidate epoch mismatch"));
        }
        let pending: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM utterances u WHERE u.workspace_id=?1 AND u.session_id=?2 AND u.generation_epoch=?3 AND u.expires_at_ms>?4 AND u.status BETWEEN 1 AND 5 AND NOT EXISTS(SELECT 1 FROM approvals a WHERE a.workspace_id=u.workspace_id AND a.session_id=u.session_id AND a.generation_epoch=u.generation_epoch AND a.candidate_hash=u.text_hash)",
            params![workspace, cue.session_id, epoch, utc_ms()?], |r| r.get(0))?;
        if pending >= 200 {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "session pending approval capacity",
            ));
        }
        for (product, field, revision) in dependencies {
            if self.fact(op, workspace, actor, product, field)?.revision != *revision {
                return Err(error(
                    ErrorCode::EvidenceExpired,
                    "candidate fact revision mismatch",
                ));
            }
        }
        let candidate_hash = candidate_hash(cue);
        let document_dependencies =
            self.knowledge_dependencies(op, workspace, actor, cue, dependencies)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // display_text is stored alongside the hash: the approval binds to the hash,
        // but the operator has to approve what would actually be broadcast.
        tx.execute(
            "INSERT INTO utterances(utterance_id,workspace_id,session_id,generation_epoch,status,text_hash,expires_at_ms,actual_played,display_text) VALUES(?1,?2,?3,?4,1,?5,?6,0,?7)",
            params![
                cue.utterance_id,
                workspace,
                cue.session_id,
                epoch,
                candidate_hash,
                sql_int(cue.expires_at_utc_ms)?,
                cue.display_text
            ],
        )?;
        for (index, (product, field, revision)) in dependencies.iter().enumerate() {
            tx.execute(
                "INSERT INTO evidence_links VALUES(?1,?2,?3,NULL,?4,?5,?6)",
                params![
                    workspace,
                    cue.utterance_id,
                    format!("fact-{index}"),
                    product,
                    field,
                    revision
                ],
            )?;
        }
        for (evidence, revision) in document_dependencies {
            tx.execute("INSERT INTO evidence_links(workspace_id,utterance_id,evidence_id,doc_revision) VALUES(?1,?2,?3,?4)",params![workspace,cue.utterance_id,evidence,revision])?;
        }
        op.check()?;
        tx.commit()?;
        Ok(candidate_hash)
    }

    /// §42.2: persists one **already judged** memory record.
    ///
    /// The write gate is the kernel's `admit_write`; this method is the transport,
    /// so it MUST NOT re-run that judgement — one gate, one answering point (W88).
    /// All it adds is its own layer's work: validate the scope identifiers, check
    /// the session permission, write the nine columns.
    ///
    /// `MemoryRecord`'s nine fields map one-to-one onto the table's nine columns
    /// (`viewer_scoped_id` → `viewer_scope`, `expires_at_utc_ms` → `expires_at_ms`),
    /// which is why this needs **no migration**. The gate also judges `source`,
    /// `claim`, `source_ref`, `written_at_utc_ms` and `publish_ref`, but §42.2 makes
    /// those *gate inputs*, not stored columns, and the table has no column for
    /// them — so they are deliberately **not** persisted. Inventing columns to look
    /// complete would give this table a second definition.
    ///
    /// No expiry check here on purpose: §42.3 puts the freshness judgement on the
    /// **read** face, and a write-face rule the contract never stated would make
    /// the gate and the store answer to two different criteria.
    pub fn record_memory(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        record: &MemoryRecord,
    ) -> Result<()> {
        self.start(op)?;
        for key in [&record.workspace_id, &record.record_id, &record.session_id] {
            id(key)?;
        }
        if record.workspace_id != workspace {
            return Err(error(
                ErrorCode::InvalidArgument,
                "memory record workspace mismatch",
            ));
        }
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let existing: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memory_records WHERE workspace_id=?1 AND record_id=?2",
            params![record.workspace_id, record.record_id],
            |r| r.get(0),
        )?;
        if existing != 0 {
            return Err(error(ErrorCode::Conflict, "memory record exists"));
        }
        let expires = sql_int(record.expires_at_utc_ms)?;
        self.conn.execute(
            "INSERT INTO memory_records(workspace_id,record_id,session_id,viewer_scope,kind,purpose,authorization_ref,content,expires_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                record.workspace_id,
                record.record_id,
                record.session_id,
                record.viewer_scoped_id,
                record.kind,
                record.purpose,
                record.authorization_ref,
                record.content,
                expires
            ],
        )?;
        op.check()?;
        Ok(())
    }

    /// The memory records of one session, read back column by column in a
    /// deterministic order.
    ///
    /// Read-only, and deliberately **not** the §42.3 read face: it applies no
    /// scope-crossing, expiry, revocation, permission, substitution or budget
    /// judgement. `memory_records` carries **no revocation column**, so a caller
    /// cannot honestly derive the `revoked` input `read_records` needs — asserting
    /// `false` here would fabricate a judgement input (W94's shape). The read
    /// judgement's wiring is a registered open question, not something this
    /// accessor may be read as having answered.
    pub fn memory_records(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        session: &str,
    ) -> Result<Vec<MemoryRecord>> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let mut statement = self.conn.prepare(
            "SELECT record_id,workspace_id,session_id,viewer_scope,kind,purpose,authorization_ref,content,expires_at_ms FROM memory_records WHERE workspace_id=?1 AND session_id=?2 ORDER BY record_id",
        )?;
        let rows = statement.query_map(params![workspace, session], |r| {
            Ok(MemoryRecord {
                record_id: r.get(0)?,
                workspace_id: r.get(1)?,
                session_id: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                viewer_scoped_id: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                kind: r.get(4)?,
                purpose: r.get(5)?,
                authorization_ref: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                content: r.get(7)?,
                expires_at_utc_ms: r.get::<_, i64>(8)? as u64,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        op.check()?;
        Ok(records)
    }

    /// §42.3: 一个工作区里的**全部**记忆记录，连同每条的撤销时刻，且**不施加** §42.3 的任何判定。
    ///
    /// 唯一的过滤是工作区——那是硬边界，不是 §42.3 的判定对象。
    /// **刻意不按会话过滤**：`read_records` 的越作用域拒绝需要**有对象可拒**；
    /// 若取数阶段就按会话把别的作用域筛掉，那条守卫还在、但它守的东西已经被上游先拿走了
    /// （**假阴性**，是最危险的方向）。既有 `memory_records` 保留它的会话过滤不动，
    /// 本入口存在的理由正是让读取面**有东西可判**。
    ///
    /// 撤销时刻是**存储态**、不是契约字段，故单列返回，MUST NOT 被塞进 `MemoryRecord`
    /// （那会给契约九字段第二处定义）。
    pub fn memory_scoped(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
    ) -> Result<Vec<StoredMemoryRecord>> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let mut statement = self.conn.prepare(
            "SELECT record_id,workspace_id,session_id,viewer_scope,kind,purpose,authorization_ref,content,expires_at_ms,revoked_at_ms FROM memory_records WHERE workspace_id=?1 ORDER BY record_id",
        )?;
        let rows = statement.query_map(params![workspace], |r| {
            Ok(StoredMemoryRecord {
                record: MemoryRecord {
                    record_id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    session_id: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    viewer_scoped_id: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    kind: r.get(4)?,
                    purpose: r.get(5)?,
                    authorization_ref: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    content: r.get(7)?,
                    expires_at_utc_ms: r.get::<_, i64>(8)? as u64,
                },
                revoked_at_ms: r.get::<_, Option<i64>>(9)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        op.check()?;
        Ok(records)
    }

    /// §42.4: 遗忘链的**阻断**落点——把一条记录标为已撤销（读取侧先失效）。
    ///
    /// 返回**生效中的**撤销时刻：若该行本就已被撤销，返回它**原有**的时刻，
    /// MUST NOT 覆盖——撤销时刻是一条事实，重写它等于篡改。
    /// §42.4 要求「**先**阻断检索与读取，**再**异步清除副本」，故阻断 MUST NOT
    /// 用删除行来表达：那样「已阻断」与「已清除」将无法区分。清除侧不在此处。
    pub fn block_memory_read(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        record_id: &str,
    ) -> Result<i64> {
        self.start(op)?;
        id(record_id)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT revoked_at_ms FROM memory_records WHERE workspace_id=?1 AND record_id=?2",
                params![workspace, record_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "memory record missing"))?;
        if let Some(revoked) = existing {
            op.check()?;
            return Ok(revoked);
        }
        let now = utc_ms()?;
        self.conn.execute(
            "UPDATE memory_records SET revoked_at_ms=?1 WHERE workspace_id=?2 AND record_id=?3 AND revoked_at_ms IS NULL",
            params![now, workspace, record_id],
        )?;
        op.check()?;
        Ok(now)
    }

    /// W111: §42.4 清理侧的**执行体**——真正把载荷从库里清掉。
    ///
    /// 与 [`Self::block_memory_read`] 同形的三条刻意的取舍：
    /// * **只置一次、不覆盖**：已清过就返回**同一个** `purged_at_ms`。若每次都刷新，
    ///   「什么时候清的」会被后来的一次改写，而那正是「可验证」要钉的东西。
    /// * **保留该行**：§24.4「删除先发布 tombstone」⇒ 被清的是 `content`，不是行。
    ///   行还在，「这条记录被遗忘过」才有凭据。
    /// * **已撤销照样能清**：读取侧已阻断**不是**拒绝清除的理由——那会让一条被遗忘的记录
    ///   因为「已经不能读了」而永远留在库里。
    ///
    /// ⚠️ **这不是介质级擦除**：SQLite 的页与 WAL 可能仍留有旧字节。故上层 MUST NOT
    /// 据此宣称「已完全擦除」（`nai_kernel` 的 `body_purged` 只支撑**逻辑**口径）。
    pub fn purge_memory_body(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        record_id: &str,
    ) -> Result<i64> {
        self.start(op)?;
        id(record_id)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT purged_at_ms FROM memory_records WHERE workspace_id=?1 AND record_id=?2",
                params![workspace, record_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "memory record missing"))?;
        if let Some(purged) = existing {
            op.check()?;
            return Ok(purged);
        }
        let now = utc_ms()?;
        self.conn.execute(
            "UPDATE memory_records SET content='' WHERE workspace_id=?1 AND record_id=?2 AND purged_at_ms IS NULL",
            params![workspace, record_id],
        )?;
        self.conn.execute(
            "UPDATE memory_records SET purged_at_ms=?1 WHERE workspace_id=?2 AND record_id=?3 AND purged_at_ms IS NULL",
            params![now, workspace, record_id],
        )?;
        op.check()?;
        Ok(now)
    }

    /// W111: 载荷**是否真的被清过**——由存储派生，不是调用方的说法。
    ///
    /// §42.4 的判定需要这个事实，而调用方给的 `purge_state` 只是陈述。
    /// 读列（而不是收参数）是 [`Self::block_memory_read`] 之外的第二个「事实派生源」。
    pub fn memory_body_purged_at(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        record_id: &str,
    ) -> Result<Option<i64>> {
        self.start(op)?;
        id(record_id)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let purged: Option<i64> = self
            .conn
            .query_row(
                "SELECT purged_at_ms FROM memory_records WHERE workspace_id=?1 AND record_id=?2",
                params![workspace, record_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "memory record missing"))?;
        op.check()?;
        Ok(purged)
    }

    pub fn approve(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        input: &Approval,
    ) -> Result<()> {
        self.start(op)?;
        id(&input.approval_id)?;
        access(&self.conn, workspace, actor, "approval.write", "*")?;
        let now = utc_ms()?;
        if !crate::publication::candidate_documents_valid(
            &self.conn,
            workspace,
            &input.session_id,
            &input.candidate_hash,
            now,
        )? {
            return Err(error(
                ErrorCode::EvidenceExpired,
                "candidate document expired or revoked",
            ));
        }
        if sql_int(input.expires_at_utc_ms)? <= now || input.hash_algorithm != "sha256-canonical-v1"
        {
            return Err(error(
                ErrorCode::ApprovalExpired,
                "approval expiry or hash algorithm invalid",
            ));
        }
        let valid:i64=self.conn.query_row("SELECT COUNT(*) FROM utterances u JOIN sessions s ON s.session_id=u.session_id WHERE u.workspace_id=?1 AND u.session_id=?2 AND u.generation_epoch=?3 AND s.generation_epoch=u.generation_epoch AND u.text_hash=?4 AND u.expires_at_ms>?5 AND u.status NOT IN(12,13,14,15) AND NOT EXISTS(SELECT 1 FROM evidence_links e LEFT JOIN facts f ON f.workspace_id=e.workspace_id AND f.product_id=e.product_id AND f.field_key=e.field_key WHERE e.workspace_id=u.workspace_id AND e.utterance_id=u.utterance_id AND e.product_id IS NOT NULL AND (f.revision IS NULL OR f.revision<>e.fact_revision OR f.valid_from_ms>?5 OR f.valid_until_ms<=?5))",params![workspace,input.session_id,sql_int(input.generation_epoch)?,input.candidate_hash,now],|r|r.get(0))?;
        if valid == 0 {
            return Err(error(
                ErrorCode::ApprovalExpired,
                "candidate not approvable",
            ));
        }
        let approved: i64 = self.conn.query_row("SELECT COUNT(*) FROM approvals WHERE workspace_id=?1 AND session_id=?2 AND generation_epoch=?3 AND candidate_hash=?4", params![workspace, input.session_id, sql_int(input.generation_epoch)?, input.candidate_hash], |r| r.get(0))?;
        if approved != 0 {
            return Err(error(ErrorCode::Conflict, "candidate already approved"));
        }
        let mut approval = input.clone();
        approval.actor_id = actor.into();
        approval.actor_role = "approver".into();
        approval.revoked_at_utc_ms = None;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO approvals VALUES(?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9)",
            params![
                approval.approval_id,
                workspace,
                approval.session_id,
                sql_int(approval.generation_epoch)?,
                approval.candidate_hash,
                actor,
                approval.action_scope,
                sql_int(approval.expires_at_utc_ms)?,
                approval.encode_to_vec()
            ],
        )?;
        tx.execute("UPDATE utterances SET status=6 WHERE workspace_id=?1 AND session_id=?2 AND generation_epoch=?3 AND text_hash=?4", params![workspace, approval.session_id, sql_int(approval.generation_epoch)?, approval.candidate_hash])?;
        audit(
            &tx,
            workspace,
            actor,
            "approval.recorded",
            &approval.candidate_hash,
            &serde_json::json!({"epoch":approval.generation_epoch.to_string()}),
        )?;
        crate::decision::approval_entry(
            op,
            &tx,
            workspace,
            &approval.session_id,
            crate::decision::DecisionKind::Approval,
            &serde_json::json!({"candidate_hash":approval.candidate_hash,"approval_id":approval.approval_id,"epoch":approval.generation_epoch.to_string(),"actor_id":actor}),
            &approval.approval_id,
        )?;
        op.check()?;
        tx.commit()?;
        Ok(())
    }
    pub fn approval_valid(
        &self,
        op: &Operation,
        workspace: &str,
        session: &str,
        epoch: u64,
        candidate: &str,
    ) -> Result<bool> {
        self.start(op)?;
        if !crate::publication::candidate_documents_valid(
            &self.conn,
            workspace,
            session,
            candidate,
            utc_ms()?,
        )? {
            return Ok(false);
        }
        let count:i64=self.conn.query_row("SELECT COUNT(*) FROM approvals a JOIN sessions s ON s.session_id=a.session_id JOIN utterances u ON u.session_id=a.session_id AND u.text_hash=a.candidate_hash WHERE a.workspace_id=?1 AND a.session_id=?2 AND a.generation_epoch=?3 AND s.generation_epoch=a.generation_epoch AND a.candidate_hash=?4 AND a.revoked_at_ms IS NULL AND a.expires_at_ms>?5 AND u.expires_at_ms>?5 AND u.status NOT IN(12,13,14,15) AND NOT EXISTS(SELECT 1 FROM evidence_links e LEFT JOIN facts f ON f.workspace_id=e.workspace_id AND f.product_id=e.product_id AND f.field_key=e.field_key WHERE e.workspace_id=u.workspace_id AND e.utterance_id=u.utterance_id AND e.product_id IS NOT NULL AND (f.revision IS NULL OR f.revision<>e.fact_revision OR f.valid_from_ms>?5 OR f.valid_until_ms<=?5)) AND EXISTS(SELECT 1 FROM permission_grants p WHERE p.workspace_id=a.workspace_id AND p.actor_id=a.actor_id AND p.permission='approval.write' AND p.resource_scope='*' AND p.revoked_at_ms IS NULL)",params![workspace,session,sql_int(epoch)?,candidate,utc_ms()?],|r|r.get(0))?;
        op.check()?;
        Ok(count > 0)
    }
    /// Bounded snapshot of approval-tracked candidates for the given scope.
    /// Each item carries candidate_hash, the current approval id (if any),
    /// expiry, utterance status and evidence count. Stale/revoked/approved
    /// candidates are filtered by the same validity rules as approval_valid.
    pub fn list_pending_approvals(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        session: &str,
        epoch: u64,
        limit: u32,
    ) -> Result<Vec<PendingApproval>> {
        self.start(op)?;
        if !(1..=200).contains(&limit) {
            return Err(error(
                ErrorCode::InvalidArgument,
                "approval list limit out of range",
            ));
        }
        access(&self.conn, workspace, actor, "approval.read", "*")?;
        let now = utc_ms()?;
        let mut statement = self.conn.prepare(
            "SELECT u.text_hash, a.approval_id, u.expires_at_ms, u.status,
                (SELECT COUNT(*) FROM evidence_links e WHERE e.workspace_id=u.workspace_id AND e.utterance_id=u.utterance_id),
                u.display_text
             FROM utterances u JOIN sessions s ON s.session_id=u.session_id
             LEFT JOIN approvals a ON a.workspace_id=u.workspace_id AND a.session_id=u.session_id AND a.candidate_hash=u.text_hash AND a.generation_epoch=?6 AND a.revoked_at_ms IS NULL AND a.expires_at_ms>?5
             WHERE u.workspace_id=?1 AND u.session_id=?2 AND u.generation_epoch=?3 AND s.generation_epoch=u.generation_epoch
                AND u.expires_at_ms>?5 AND u.status BETWEEN 1 AND 5 AND a.approval_id IS NULL
                AND NOT EXISTS(SELECT 1 FROM approvals previous WHERE previous.workspace_id=u.workspace_id AND previous.session_id=u.session_id AND previous.generation_epoch=u.generation_epoch AND previous.candidate_hash=u.text_hash)
               AND NOT EXISTS(SELECT 1 FROM evidence_links e LEFT JOIN facts f ON f.workspace_id=e.workspace_id AND f.product_id=e.product_id AND f.field_key=e.field_key WHERE e.workspace_id=u.workspace_id AND e.utterance_id=u.utterance_id AND e.product_id IS NOT NULL AND (f.revision IS NULL OR f.revision<>e.fact_revision OR f.valid_from_ms>?5 OR f.valid_until_ms<=?5))
             ORDER BY u.rowid LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![
                workspace,
                session,
                sql_int(epoch)?,
                limit,
                now,
                sql_int(epoch)?
            ],
            |r| {
                Ok(PendingApproval {
                    candidate_hash: r.get(0)?,
                    approval_id: r.get(1)?,
                    expires_at_utc_ms: r.get::<_, i64>(2)? as u64,
                    status: r.get(3)?,
                    evidence_count: r.get::<_, i64>(4)? as u64,
                    display_text: r.get(5)?,
                })
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            let item = row?;
            if crate::publication::candidate_documents_valid(
                &self.conn,
                workspace,
                session,
                &item.candidate_hash,
                now,
            )? {
                out.push(item);
            }
        }
        op.check()?;
        Ok(out)
    }
    /// Read a specific approval-tracked candidate. Returns a readable failure
    /// when the candidate is not in the scope or has been revoked/expired.
    pub fn read_approval(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        session: &str,
        epoch: u64,
        candidate: &str,
    ) -> Result<PendingApproval> {
        self.start(op)?;
        id(candidate)?;
        access(&self.conn, workspace, actor, "approval.read", "*")?;
        let now = utc_ms()?;
        let record=self.conn.query_row(
            "SELECT u.text_hash, a.approval_id, u.expires_at_ms, u.status,
                    (SELECT COUNT(*) FROM evidence_links e WHERE e.workspace_id=u.workspace_id AND e.utterance_id=u.utterance_id),
                    u.display_text
             FROM utterances u JOIN sessions s ON s.session_id=u.session_id
             LEFT JOIN approvals a ON a.workspace_id=u.workspace_id AND a.session_id=u.session_id AND a.candidate_hash=u.text_hash AND a.generation_epoch=?4 AND a.revoked_at_ms IS NULL AND a.expires_at_ms>?3
             WHERE u.workspace_id=?1 AND u.session_id=?2 AND u.generation_epoch=?4 AND s.generation_epoch=u.generation_epoch AND u.text_hash=?5 AND u.expires_at_ms>?3 AND u.status NOT IN(12,13,14,15)",
            params![workspace, session, now, sql_int(epoch)?, candidate],
            |r| Ok(PendingApproval { candidate_hash: r.get(0)?, approval_id: r.get(1)?, expires_at_utc_ms: r.get::<_,i64>(2)? as u64, status: r.get(3)?, evidence_count: r.get::<_,i64>(4)? as u64, display_text: r.get(5)? }),
        ).optional()?.ok_or_else(|| error(ErrorCode::DeadlineExceeded, "approval not found or expired"))?;
        if !crate::publication::candidate_documents_valid(
            &self.conn, workspace, session, candidate, now,
        )? {
            return Err(error(
                ErrorCode::EvidenceExpired,
                "candidate document expired or revoked",
            ));
        }
        op.check()?;
        Ok(record)
    }
    /// Reject an awaiting candidate: actor needs approval.write; approved
    /// candidates are refused; the candidate is moved to Blocked and auditing
    /// and decision-log evidence are recorded. Returns the Blocked state.
    pub fn reject_candidate(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        session: &str,
        epoch: u64,
        candidate: &str,
    ) -> Result<()> {
        self.start(op)?;
        id(candidate)?;
        access(&self.conn, workspace, actor, "approval.write", "*")?;
        let now = utc_ms()?;
        let approved:i64=self.conn.query_row("SELECT COUNT(*) FROM approvals a WHERE a.workspace_id=?1 AND a.session_id=?2 AND a.generation_epoch=?3 AND a.candidate_hash=?4 AND a.revoked_at_ms IS NULL AND a.expires_at_ms>?5",params![workspace,session,sql_int(epoch)?,candidate,now],|r|r.get(0))?;
        if approved > 0 {
            return Err(error(
                ErrorCode::Conflict,
                "cannot reject approved candidate",
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute(
            "UPDATE utterances SET status=14 WHERE workspace_id=?1 AND session_id=?2 AND generation_epoch=?3 AND text_hash=?4 AND expires_at_ms>?5 AND status BETWEEN 1 AND 5 AND EXISTS(SELECT 1 FROM sessions s WHERE s.workspace_id=utterances.workspace_id AND s.session_id=utterances.session_id AND s.generation_epoch=utterances.generation_epoch)",
            params![workspace, session, sql_int(epoch)?, candidate, now],
        )?;
        if updated == 0 {
            return Err(error(
                ErrorCode::DeadlineExceeded,
                "candidate missing, expired or already blocked",
            ));
        }
        audit(
            &tx,
            workspace,
            actor,
            "approval.rejected",
            candidate,
            &serde_json::json!({"epoch":epoch.to_string()}),
        )?;
        tx.execute("UPDATE approvals SET revoked_at_ms=?5 WHERE workspace_id=?1 AND session_id=?2 AND generation_epoch=?3 AND candidate_hash=?4 AND revoked_at_ms IS NULL", params![workspace, session, sql_int(epoch)?, candidate, now])?;
        crate::decision::approval_entry(
            op,
            &tx,
            workspace,
            session,
            crate::decision::DecisionKind::Cancellation,
            &serde_json::json!({"candidate_hash":candidate,"reason":"operator_reject","epoch":epoch.to_string(),"actor_id":actor}),
            "storage-reject",
        )?;
        op.check()?;
        tx.commit()?;
        Ok(())
    }
}

/// Non-payload approval view: candidate identity, current approval id (if any),
/// expiry, utterance status and evidence count. Never carries candidate payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub candidate_hash: String,
    pub approval_id: Option<String>,
    pub expires_at_utc_ms: u64,
    pub status: i32,
    pub evidence_count: u64,
    /// The candidate's own display text, as admitted. Empty means the row predates
    /// the column (migration 0006), not that the candidate had no text.
    pub display_text: String,
}
fn validate_fact(fact: &AuthoritativeFact) -> Result<()> {
    if let Some(from) = fact.valid_from_utc_ms {
        sql_int(from)?;
    }
    if let Some(until) = fact.valid_until_utc_ms {
        sql_int(until)?;
        if fact.valid_from_utc_ms.is_some_and(|from| until <= from) {
            return Err(error(ErrorCode::InvalidArgument, "fact validity range"));
        }
    }
    match fact.value.as_ref() {
        Some(authoritative_fact::Value::Money(m))
            if m.currency.len() == 3
                && m.currency.bytes().all(|b| b.is_ascii_uppercase())
                && m.scale <= 9 => {}
        Some(authoritative_fact::Value::Decimal(s))
            if s.len() <= 64 && {
                let unsigned = s.strip_prefix('-').unwrap_or(s);
                let parts: Vec<_> = unsigned.split('.').collect();
                parts.len() <= 2
                    && parts
                        .iter()
                        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            } => {}
        Some(
            authoritative_fact::Value::Text(_)
            | authoritative_fact::Value::Integer(_)
            | authoritative_fact::Value::Flag(_),
        ) => {}
        _ => {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid exact fact value",
            ));
        }
    }
    Ok(())
}

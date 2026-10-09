//! Immutable snapshot copies and a single transactional visibility switch.
use crate::{
    Operation, Result, SnapshotPublish, Store, error, hash, id,
    store::{access, audit},
    utc_ms,
};
use nai_contracts::v1::{ErrorCode, SessionSnapshot};
use rusqlite::{
    Connection, OptionalExtension, Params, TransactionBehavior, params, types::ValueRef,
};
use sha2::{Digest, Sha256};

const MAX_CHUNKS: i64 = 4096;
const MAX_BYTES: i64 = 16 * 1024 * 1024;

fn digest_rows<P: Params>(
    conn: &Connection,
    op: &Operation,
    sql: &str,
    parameters: P,
) -> Result<String> {
    let mut statement = conn.prepare(sql)?;
    let columns = statement.column_count();
    let mut rows = statement.query(parameters)?;
    let mut digest = Sha256::new();
    digest.update(b"nai-knowledge-rows-v1");
    while let Some(row) = rows.next()? {
        op.check()?;
        digest.update(b"row");
        for column in 0..columns {
            match row.get_ref(column)? {
                ValueRef::Null => digest.update([0]),
                ValueRef::Integer(value) => {
                    digest.update([1]);
                    digest.update(value.to_le_bytes());
                }
                ValueRef::Real(_) => {
                    return Err(error(
                        ErrorCode::Internal,
                        "unexpected floating snapshot field",
                    ));
                }
                ValueRef::Text(value) | ValueRef::Blob(value) => {
                    digest.update([if matches!(row.get_ref(column)?, ValueRef::Text(_)) {
                        2
                    } else {
                        3
                    }]);
                    digest.update((value.len() as u64).to_le_bytes());
                    digest.update(value);
                }
            }
        }
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn permission_revision(conn: &Connection, workspace: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT permission_revision FROM workspaces WHERE workspace_id=?1",
        [workspace],
        |row| row.get(0),
    )?)
}

fn fact_revision(conn: &Connection, op: &Operation, workspace: &str) -> Result<String> {
    let (count, bytes): (i64, i64) = conn.query_row(
        "SELECT COUNT(*),COALESCE(SUM(length(fact_proto)),0) FROM facts WHERE workspace_id=?1",
        [workspace],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if count > MAX_CHUNKS || bytes > MAX_BYTES {
        return Err(error(
            ErrorCode::ResourceExhausted,
            "snapshot facts exceed bound",
        ));
    }
    let digest = digest_rows(
        conn,
        op,
        "SELECT product_id,field_key,revision,fact_proto FROM facts WHERE workspace_id=?1 ORDER BY product_id,field_key",
        [workspace],
    )?;
    Ok(hash(
        &serde_json::to_vec(&(workspace, permission_revision(conn, workspace)?, digest))
            .map_err(|_| error(ErrorCode::Internal, "invalid fact fingerprint"))?,
    ))
}

fn active(conn: &Connection, workspace: &str, kb: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT snapshot_id FROM knowledge_active_snapshots WHERE workspace_id=?1 AND kb_scope=?2",params![workspace,kb],|row|row.get(0)).optional()?)
}

fn descriptor(conn: &Connection, workspace: &str, snapshot: &str) -> Result<SnapshotPublish> {
    conn.query_row("SELECT kb_scope,manifest_hash,fact_revision,index_ref FROM knowledge_snapshots WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot],|row|Ok(SnapshotPublish { workspace_id:workspace.into(),snapshot_id:snapshot.into(),kb_scope:row.get(0)?,manifest_hash:row.get(1)?,fact_revision:row.get(2)?,index_ref:row.get(3)? })).optional()?.ok_or_else(||error(ErrorCode::InvalidArgument,"snapshot absent"))
}

fn manifest(conn: &Connection, op: &Operation, workspace: &str, snapshot: &str) -> Result<String> {
    let mut components = Vec::new();
    for sql in [
        "SELECT workspace_id,snapshot_id,kb_scope,fact_revision,index_ref FROM knowledge_snapshots WHERE workspace_id=?1 AND snapshot_id=?2",
        "SELECT document_id,revision,source,parser_version,product_scope,acl_ref,original_artifact_ref,valid_from_ms,valid_until_ms FROM knowledge_snapshot_documents WHERE workspace_id=?1 AND snapshot_id=?2 ORDER BY document_id",
        "SELECT chunk_id,document_id,doc_revision,content,source_span,product_scope,acl_ref,tokens FROM knowledge_snapshot_chunks WHERE workspace_id=?1 AND snapshot_id=?2 ORDER BY chunk_id",
        "SELECT chunk_id,tokens FROM knowledge_snapshot_fts WHERE workspace_id=?1 AND snapshot_id=?2 ORDER BY chunk_id",
        "SELECT index_id,embedding_provider,model_version,dimension,normalization,chunker_version,content_hash FROM knowledge_snapshot_indexes WHERE workspace_id=?1 AND snapshot_id=?2",
        "SELECT chunk_id,vector FROM knowledge_snapshot_vectors WHERE workspace_id=?1 AND snapshot_id=?2 ORDER BY chunk_id",
        "SELECT product_id,field_key,revision,fact_proto FROM knowledge_snapshot_facts WHERE workspace_id=?1 AND snapshot_id=?2 ORDER BY product_id,field_key",
    ] {
        components.push(digest_rows(conn, op, sql, params![workspace, snapshot])?);
    }
    Ok(hash(
        &serde_json::to_vec(&("nai-knowledge-manifest-v1", components))
            .map_err(|_| error(ErrorCode::Internal, "invalid manifest"))?,
    ))
}

impl Store {
    /// Capture reviewed documents and current facts into an invisible immutable
    /// build. The caller explicitly supplies the active ID it reviewed.
    #[allow(clippy::too_many_arguments)]
    pub fn stage_snapshot(
        &mut self,
        op: &Operation,
        actor: &str,
        workspace: &str,
        snapshot: &str,
        kb: &str,
        documents: &[String],
        index_ref: Option<&str>,
        expected_active: Option<&str>,
    ) -> Result<()> {
        self.start(op)?;
        for value in [workspace, snapshot, kb] {
            id(value)?;
        }
        if documents.is_empty() || documents.len() > 256 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid snapshot documents",
            ));
        }
        for document in documents {
            id(document)?;
        }
        let selected = serde_json::to_string(documents)
            .map_err(|_| error(ErrorCode::InvalidArgument, "invalid document selection"))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, workspace, actor, "knowledge.import", kb)?;
        if active(&tx, workspace, kb)?.as_deref() != expected_active {
            return Err(error(ErrorCode::Conflict, "active snapshot changed"));
        }
        let count:i64=tx.query_row("SELECT COUNT(*) FROM documents WHERE workspace_id=?1 AND status!='revoked' AND document_id IN(SELECT value FROM json_each(?2))",params![workspace,selected],|row|row.get(0))?;
        if count != documents.len() as i64 {
            return Err(error(
                ErrorCode::InvalidArgument,
                "snapshot document absent or duplicate",
            ));
        }
        let (chunks,bytes):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(content AS BLOB))),0) FROM chunks WHERE workspace_id=?1 AND document_id IN(SELECT value FROM json_each(?2))",params![workspace,selected],|row|Ok((row.get(0)?,row.get(1)?)))?;
        if chunks == 0 || chunks > MAX_CHUNKS || bytes > MAX_BYTES {
            return Err(error(ErrorCode::ResourceExhausted, "snapshot chunk bound"));
        }
        let (lexical_count,token_bytes):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(f.tokens AS BLOB))),0) FROM knowledge_fts f JOIN chunks c ON c.workspace_id=f.workspace_id AND c.chunk_id=f.chunk_id WHERE c.workspace_id=?1 AND c.document_id IN(SELECT value FROM json_each(?2))",params![workspace,selected],|row|Ok((row.get(0)?,row.get(1)?)))?;
        if lexical_count != chunks || token_bytes > MAX_BYTES {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "snapshot lexical index incomplete or oversized",
            ));
        }
        let facts = fact_revision(&tx, op, workspace)?;
        tx.execute(
            "INSERT INTO knowledge_snapshots VALUES(?1,?2,?3,'',?4,?5,'staged')",
            params![workspace, snapshot, kb, facts, index_ref],
        )?;
        tx.execute(
            "INSERT INTO knowledge_snapshot_builds VALUES(?1,?2,?3,?4)",
            params![
                workspace,
                snapshot,
                expected_active,
                permission_revision(&tx, workspace)?
            ],
        )?;
        tx.execute("INSERT INTO knowledge_snapshot_documents SELECT d.workspace_id,?2,d.document_id,r.revision,d.source,r.parser_version,d.product_scope,d.acl_ref,d.original_artifact_ref,r.valid_from_ms,r.valid_until_ms FROM documents d JOIN document_revisions r ON r.workspace_id=d.workspace_id AND r.document_id=d.document_id AND r.revision=d.current_revision WHERE d.workspace_id=?1 AND d.document_id IN(SELECT value FROM json_each(?3))",params![workspace,snapshot,selected])?;
        let copied:i64=tx.query_row("SELECT COUNT(*) FROM knowledge_snapshot_documents WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot],|row|row.get(0))?;
        if copied != count {
            return Err(error(ErrorCode::Conflict, "document revision incomplete"));
        }
        tx.execute("INSERT INTO knowledge_snapshot_chunks SELECT c.workspace_id,?2,c.chunk_id,c.document_id,c.doc_revision,c.content,c.source_span,c.product_scope,c.acl_ref,f.tokens FROM chunks c JOIN knowledge_fts f ON f.workspace_id=c.workspace_id AND f.chunk_id=c.chunk_id JOIN knowledge_snapshot_documents d ON d.workspace_id=c.workspace_id AND d.snapshot_id=?2 AND d.document_id=c.document_id AND d.revision=c.doc_revision WHERE c.workspace_id=?1",params![workspace,snapshot])?;
        let copied:i64=tx.query_row("SELECT COUNT(*) FROM knowledge_snapshot_chunks WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot],|row|row.get(0))?;
        if copied != chunks {
            return Err(error(ErrorCode::Conflict, "document index incomplete"));
        }
        tx.execute("INSERT INTO knowledge_snapshot_facts SELECT workspace_id,?2,product_id,field_key,revision,fact_proto FROM facts WHERE workspace_id=?1",params![workspace,snapshot])?;
        audit(
            &tx,
            workspace,
            actor,
            "knowledge.staged",
            snapshot,
            &serde_json::json!({"fact_revision":facts}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(())
    }

    /// Copy a complete matching vector index (or finish lexical-only indexing),
    /// seal the server-computed manifest and leave the snapshot invisible.
    pub fn index_snapshot(
        &mut self,
        op: &Operation,
        actor: &str,
        workspace: &str,
        snapshot: &str,
    ) -> Result<SnapshotPublish> {
        self.start(op)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let metadata = descriptor(&tx, workspace, snapshot)?;
        access(
            &tx,
            workspace,
            actor,
            "knowledge.import",
            &metadata.kb_scope,
        )?;
        let state: String = tx.query_row(
            "SELECT state FROM knowledge_snapshots WHERE workspace_id=?1 AND snapshot_id=?2",
            params![workspace, snapshot],
            |row| row.get(0),
        )?;
        if state != "staged" {
            return Err(error(ErrorCode::Conflict, "snapshot is not staging"));
        }
        tx.execute("INSERT INTO knowledge_snapshot_fts SELECT workspace_id,snapshot_id,chunk_id,tokens FROM knowledge_snapshot_chunks WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot])?;
        if let Some(index) = &metadata.index_ref {
            let copied=tx.execute("INSERT INTO knowledge_snapshot_indexes SELECT workspace_id,?2,index_id,embedding_provider,model_version,dimension,normalization,chunker_version,content_hash FROM vector_index_manifests WHERE workspace_id=?1 AND index_id=?3",params![workspace,snapshot,index])?;
            if copied != 1 {
                return Err(error(ErrorCode::Conflict, "snapshot vector index absent"));
            }
            let bytes:i64=tx.query_row("SELECT COALESCE(SUM(length(v.vector)),0) FROM knowledge_vectors v JOIN knowledge_snapshot_chunks c ON c.workspace_id=v.workspace_id AND c.snapshot_id=?2 AND c.chunk_id=v.chunk_id WHERE v.workspace_id=?1 AND v.index_id=?3",params![workspace,snapshot,index],|row|row.get(0))?;
            if bytes > MAX_BYTES {
                return Err(error(
                    ErrorCode::ResourceExhausted,
                    "snapshot vector byte bound",
                ));
            }
            tx.execute("INSERT INTO knowledge_snapshot_vectors SELECT v.workspace_id,?2,v.chunk_id,v.vector FROM knowledge_vectors v JOIN chunks c ON c.workspace_id=v.workspace_id AND c.chunk_id=v.chunk_id JOIN knowledge_snapshot_chunks s ON s.workspace_id=c.workspace_id AND s.snapshot_id=?2 AND s.chunk_id=c.chunk_id AND s.doc_revision=c.doc_revision AND s.content=c.content WHERE v.workspace_id=?1 AND v.index_id=?3",params![workspace,snapshot,index])?;
            let (count,bytes):(i64,i64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(length(vector)),0) FROM knowledge_snapshot_vectors WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot],|row|Ok((row.get(0)?,row.get(1)?)))?;
            let chunks:i64=tx.query_row("SELECT COUNT(*) FROM knowledge_snapshot_chunks WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot],|row|row.get(0))?;
            if count != chunks || bytes > MAX_BYTES {
                return Err(error(
                    ErrorCode::Conflict,
                    "snapshot vectors incomplete or oversized",
                ));
            }
        }
        let digest = manifest(&tx, op, workspace, snapshot)?;
        tx.execute("UPDATE knowledge_snapshots SET state='indexed',manifest_hash=?3 WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot,digest])?;
        audit(
            &tx,
            workspace,
            actor,
            "knowledge.snapshot_indexed",
            snapshot,
            &serde_json::json!({"manifest_hash":digest}),
        )?;
        let result = descriptor(&tx, workspace, snapshot)?;
        op.check()?;
        tx.commit()?;
        Ok(result)
    }

    pub fn bind_session_knowledge(
        &mut self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        session: &str,
        kb: &str,
        snapshot: &str,
    ) -> Result<SessionSnapshot> {
        self.start(op)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, workspace, actor, "session.manage", "*")?;
        access(&tx, workspace, actor, "knowledge.read", kb)?;
        let existing:Option<(String,String)>=tx.query_row("SELECT snapshot_id,kb_scope FROM session_knowledge_bindings WHERE workspace_id=?1 AND session_id=?2",params![workspace,session],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
        if let Some((existing, scope)) = existing {
            if existing != snapshot || scope != kb {
                return Err(error(
                    ErrorCode::Conflict,
                    "session knowledge binding is frozen",
                ));
            }
        } else {
            let published:i64=tx.query_row("SELECT COUNT(*) FROM knowledge_active_snapshots a JOIN knowledge_snapshots s ON s.workspace_id=a.workspace_id AND s.snapshot_id=a.snapshot_id WHERE a.workspace_id=?1 AND a.kb_scope=?2 AND a.snapshot_id=?3 AND s.state='published'",params![workspace,kb,snapshot],|row|row.get(0))?;
            if published != 1 {
                return Err(error(
                    ErrorCode::Conflict,
                    "binding requires active published snapshot",
                ));
            }
            tx.execute(
                "INSERT INTO session_knowledge_bindings VALUES(?1,?2,?3,?4)",
                params![workspace, session, kb, snapshot],
            )?;
            audit(
                &tx,
                workspace,
                actor,
                "knowledge.session_bound",
                session,
                &serde_json::json!({"snapshot_id":snapshot}),
            )?;
        }
        op.check()?;
        tx.commit()?;
        self.session(op, workspace, actor, session)
    }
}

pub(crate) fn publish(
    store: &mut Store,
    op: &Operation,
    actor: &str,
    request: &SnapshotPublish,
) -> Result<()> {
    store.start(op)?;
    let workspace = &request.workspace_id;
    let snapshot = &request.snapshot_id;
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let stored = descriptor(&tx, workspace, snapshot)?;
    access(&tx, workspace, actor, "knowledge.publish", &stored.kb_scope)?;
    if request.kb_scope != stored.kb_scope
        || request.manifest_hash != stored.manifest_hash
        || request.fact_revision != stored.fact_revision
        || request.index_ref != stored.index_ref
        || request.manifest_hash != manifest(&tx, op, workspace, snapshot)?
    {
        return Err(error(ErrorCode::Conflict, "snapshot manifest mismatch"));
    }
    let state: String = tx.query_row(
        "SELECT state FROM knowledge_snapshots WHERE workspace_id=?1 AND snapshot_id=?2",
        params![workspace, snapshot],
        |row| row.get(0),
    )?;
    if state == "published" {
        op.check()?;
        return Ok(());
    }
    if state != "indexed" {
        return Err(error(ErrorCode::Conflict, "snapshot index not sealed"));
    }
    let (expected,permission):(Option<String>,i64)=tx.query_row("SELECT expected_active,permission_revision FROM knowledge_snapshot_builds WHERE workspace_id=?1 AND snapshot_id=?2",params![workspace,snapshot],|row|Ok((row.get(0)?,row.get(1)?)))?;
    if permission_revision(&tx, workspace)? != permission
        || stored.fact_revision != fact_revision(&tx, op, workspace)?
        || active(&tx, workspace, &stored.kb_scope)? != expected
    {
        return Err(error(
            ErrorCode::Conflict,
            "snapshot publication CAS conflict",
        ));
    }
    let invalid:i64=tx.query_row("SELECT COUNT(*) FROM knowledge_snapshot_documents s LEFT JOIN documents d ON d.workspace_id=s.workspace_id AND d.document_id=s.document_id WHERE s.workspace_id=?1 AND s.snapshot_id=?2 AND (d.current_revision IS NULL OR d.current_revision!=s.revision OR d.status='revoked' OR EXISTS(SELECT 1 FROM knowledge_document_revocations r WHERE r.workspace_id=s.workspace_id AND r.document_id=s.document_id AND r.revision=s.revision AND r.kb_scope IN('*',?3)))",params![workspace,snapshot,stored.kb_scope],|row|row.get(0))?;
    if invalid != 0 {
        return Err(error(
            ErrorCode::Conflict,
            "staged document changed or revoked",
        ));
    }
    if let Some(previous) = expected {
        let mut statement=tx.prepare("SELECT d.document_id,d.revision FROM knowledge_snapshot_documents d LEFT JOIN knowledge_snapshot_documents n ON n.workspace_id=d.workspace_id AND n.snapshot_id=?3 AND n.document_id=d.document_id WHERE d.workspace_id=?1 AND d.snapshot_id=?2 AND (n.revision IS NULL OR n.revision!=d.revision)")?;
        let replaced = statement
            .query_map(params![workspace, previous, snapshot], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        for (document, revision) in replaced {
            revoke_revision(
                &tx,
                workspace,
                &document,
                &revision,
                &stored.kb_scope,
                "document_replaced",
            )?;
        }
    }
    tx.execute(
        "UPDATE knowledge_snapshots SET state='published' WHERE workspace_id=?1 AND snapshot_id=?2",
        params![workspace, snapshot],
    )?;
    tx.execute("INSERT INTO knowledge_active_snapshots VALUES(?1,?2,?3) ON CONFLICT(workspace_id,kb_scope) DO UPDATE SET snapshot_id=excluded.snapshot_id",params![workspace,stored.kb_scope,snapshot])?;
    audit(
        &tx,
        workspace,
        actor,
        "knowledge.published",
        snapshot,
        &serde_json::json!({"manifest_hash":stored.manifest_hash,"fact_revision":stored.fact_revision}),
    )?;
    op.check()?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn revoke_revision(
    conn: &Connection,
    workspace: &str,
    document: &str,
    revision: &str,
    kb: &str,
    reason: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO knowledge_document_revocations VALUES(?1,?2,?3,?4,?5,?6)",
        params![workspace, document, revision, kb, reason, utc_ms()?],
    )?;
    conn.execute("UPDATE utterances SET status=12 WHERE workspace_id=?1 AND utterance_id IN(SELECT e.utterance_id FROM evidence_links e JOIN utterances u ON u.workspace_id=e.workspace_id AND u.utterance_id=e.utterance_id LEFT JOIN session_knowledge_bindings b ON b.workspace_id=u.workspace_id AND b.session_id=u.session_id JOIN knowledge_snapshot_chunks c ON c.workspace_id=e.workspace_id AND e.evidence_id='chunk:'||c.chunk_id AND (e.doc_revision IS NULL OR e.doc_revision=c.doc_revision) WHERE c.workspace_id=?1 AND c.document_id=?2 AND c.doc_revision=?3 AND (?4='*' OR b.kb_scope=?4)) AND status NOT IN(11,12,13,14,15)",params![workspace,document,revision,kb])?;
    conn.execute("UPDATE approvals SET revoked_at_ms=?2 WHERE workspace_id=?1 AND candidate_hash IN(SELECT text_hash FROM utterances WHERE workspace_id=?1 AND status=12) AND revoked_at_ms IS NULL",params![workspace,utc_ms()?])?;
    conn.execute("INSERT INTO invalidation_outbox(workspace_id,reason,resource_id,old_revision,new_revision,created_at_ms) VALUES(?1,?2,?3,?4,'revoked',?5)",params![workspace,reason,document,revision,utc_ms()?])?;
    conn.execute(
        "UPDATE tts_cache SET valid=0 WHERE workspace_id=?1",
        [workspace],
    )?;
    Ok(())
}

pub(crate) fn candidate_documents_valid(
    conn: &Connection,
    workspace: &str,
    session: &str,
    candidate: &str,
    now: i64,
) -> Result<bool> {
    let invalid:i64=conn.query_row("SELECT COUNT(*) FROM evidence_links e JOIN utterances u ON u.workspace_id=e.workspace_id AND u.utterance_id=e.utterance_id LEFT JOIN session_knowledge_bindings b ON b.workspace_id=u.workspace_id AND b.session_id=u.session_id LEFT JOIN knowledge_snapshot_chunks c ON c.workspace_id=e.workspace_id AND c.snapshot_id=b.snapshot_id AND e.evidence_id='chunk:'||c.chunk_id AND e.doc_revision=c.doc_revision LEFT JOIN knowledge_snapshot_documents d ON d.workspace_id=c.workspace_id AND d.snapshot_id=c.snapshot_id AND d.document_id=c.document_id WHERE u.workspace_id=?1 AND u.session_id=?2 AND u.text_hash=?3 AND e.doc_revision IS NOT NULL AND (d.document_id IS NULL OR d.valid_from_ms>?4 OR d.valid_until_ms<=?4 OR EXISTS(SELECT 1 FROM knowledge_document_revocations r WHERE r.workspace_id=d.workspace_id AND r.document_id=d.document_id AND r.revision=d.revision AND r.kb_scope IN('*',b.kb_scope)))",params![workspace,session,candidate,now],|row|row.get(0))?;
    Ok(invalid == 0)
}

//! Knowledge pipeline persistence: bounded import, FTS5 lexical index, vector
//! rows with index identity, atomic snapshot publication, revocation and
//! filtered local retrieval. Media/embeddings generation is upstream (D08).
use crate::{
    Operation, Result, Store, error, hash, id, sql_int,
    store::{access, audit},
    utc_ms,
};
use nai_contracts::v1::{
    AuthoritativeFact, ErrorCode, Evidence, RetrievalStatus, authoritative_fact,
};
use prost::Message;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_CONTENT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_CHUNKS: usize = 4096;
// R08 名册中由本模块承载的两条。
/// 一条候选可以引用多少条证据（事实引用与块引用合起来算）。
pub(crate) const MAX_EVIDENCE_REFS: usize = 32;
/// 一次撤销可以触碰多少个**不重复的** `(document, revision)`。它是「批次」，不是
/// 「本次选了多少份文档」——同一份文档的多个版本各算一个。
pub(crate) const MAX_REVOCATION_REVISIONS: usize = 4_096;
const MAX_TOKENS_PER_CHUNK: usize = 4096;
const MAX_DIMENSION: usize = 8192;

#[derive(Clone, Debug)]
pub struct ImportChunk {
    pub chunk_id: String,
    pub content: String,
    pub source_span: String,
    pub product_scope: String,
    pub acl_ref: String,
    pub tokens: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ImportedDocument {
    pub workspace_id: String,
    pub document_id: String,
    pub source: String,
    pub parser_version: String,
    pub product_scope: String,
    pub acl_ref: String,
    pub original_artifact_ref: Option<String>,
    pub valid_from_ms: Option<i64>,
    pub valid_until_ms: Option<i64>,
    pub content: String,
    pub chunks: Vec<ImportChunk>,
}

#[derive(Clone, Debug)]
pub struct VectorManifest {
    pub index_id: String,
    pub embedding_provider: String,
    pub model_version: String,
    pub dimension: usize,
    pub normalization: String,
    pub chunker_version: String,
    pub content_hash: String,
}

#[derive(Clone, Debug)]
pub struct SnapshotPublish {
    pub workspace_id: String,
    pub snapshot_id: String,
    pub kb_scope: String,
    pub manifest_hash: String,
    pub fact_revision: String,
    pub index_ref: Option<String>,
}

fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn document_revision(doc: &ImportedDocument, content_hash: &str) -> Result<String> {
    let chunks: BTreeMap<_, _> = doc
        .chunks
        .iter()
        .map(|chunk| {
            (
                &chunk.chunk_id,
                (
                    &chunk.content,
                    &chunk.source_span,
                    &chunk.product_scope,
                    &chunk.acl_ref,
                    &chunk.tokens,
                ),
            )
        })
        .collect();
    Ok(hash(
        &serde_json::to_vec(&(
            "nai-document-revision-v1",
            content_hash,
            &doc.source,
            &doc.parser_version,
            &doc.product_scope,
            &doc.acl_ref,
            &doc.original_artifact_ref,
            doc.valid_from_ms,
            doc.valid_until_ms,
            chunks,
        ))
        .map_err(|_| error(ErrorCode::InvalidArgument, "invalid document revision"))?,
    ))
}

impl Store {
    pub(crate) fn knowledge_dependencies(
        &self,
        op: &Operation,
        workspace: &str,
        actor: &str,
        cue: &nai_contracts::v1::PerformanceCue,
        facts: &[(&str, &str, &str)],
    ) -> Result<Vec<(String, String)>> {
        if cue.evidence_refs.len() > MAX_EVIDENCE_REFS {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "candidate evidence bound",
            ));
        }
        let snapshot = &cue.knowledge_snapshot_ref;
        if !snapshot.is_empty() {
            let bound:i64=self.conn.query_row("SELECT COUNT(*) FROM session_knowledge_bindings b JOIN knowledge_snapshots s ON s.workspace_id=b.workspace_id AND s.snapshot_id=b.snapshot_id WHERE b.workspace_id=?1 AND b.session_id=?2 AND b.snapshot_id=?3 AND s.state='published'",params![workspace,cue.session_id,snapshot],|row|row.get(0))?;
            if bound != 1 {
                return Err(error(
                    ErrorCode::EvidenceExpired,
                    "candidate knowledge is not frozen binding",
                ));
            }
            let kb:String=self.conn.query_row("SELECT kb_scope FROM session_knowledge_bindings WHERE workspace_id=?1 AND session_id=?2",params![workspace,cue.session_id],|row|row.get(0))?;
            access(&self.conn, workspace, actor, "knowledge.read", &kb)?;
            for (product, field, revision) in facts {
                let captured:i64=self.conn.query_row("SELECT COUNT(*) FROM knowledge_snapshot_facts WHERE workspace_id=?1 AND snapshot_id=?2 AND product_id=?3 AND field_key=?4 AND revision=?5",params![workspace,snapshot,product,field,revision],|row|row.get(0))?;
                if captured != 1 {
                    return Err(error(
                        ErrorCode::EvidenceExpired,
                        "candidate fact is outside frozen snapshot",
                    ));
                }
            }
        }
        let permission = self.conn.query_row(
            "SELECT permission_revision FROM workspaces WHERE workspace_id=?1",
            [workspace],
            |row| row.get(0),
        )?;
        let mut documents = Vec::new();
        let mut seen = BTreeSet::new();
        for reference in &cue.evidence_refs {
            op.check()?;
            if !seen.insert(reference) {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "duplicate evidence reference",
                ));
            }
            if facts.iter().any(|(product, field, revision)| {
                reference == &format!("fact:{product}:{field}:{revision}")
            }) {
                continue;
            }
            let chunk = reference
                .strip_prefix("chunk:")
                .ok_or_else(|| error(ErrorCode::EvidenceExpired, "unknown candidate evidence"))?;
            if snapshot.is_empty() {
                return Err(error(
                    ErrorCode::EvidenceExpired,
                    "candidate snapshot missing",
                ));
            }
            let evidence = self
                .knowledge_evidence(
                    workspace,
                    actor,
                    chunk,
                    &[],
                    utc_ms()?,
                    permission,
                    snapshot,
                )?
                .ok_or_else(|| {
                    error(
                        ErrorCode::EvidenceExpired,
                        "candidate document inaccessible",
                    )
                })?;
            documents.push((reference.clone(), evidence.revision));
        }
        Ok(documents)
    }

    /// Bounded, transactional import. Empty/oversized input is rejected; the
    /// computed content hash is authoritative, never caller-claimed.
    #[allow(clippy::too_many_arguments)]
    pub fn import_document(
        &mut self,
        op: &Operation,
        actor: &str,
        doc: &ImportedDocument,
    ) -> Result<()> {
        self.start(op)?;
        id(&doc.workspace_id)?;
        id(&doc.document_id)?;
        id(&doc.source)?;
        id(&doc.parser_version)?;
        id(&doc.product_scope)?;
        id(&doc.acl_ref)?;
        if doc.content.len() > MAX_CONTENT_BYTES {
            return Err(error(
                ErrorCode::InvalidArgument,
                "document exceeds import bound",
            ));
        }
        if doc.content.trim().is_empty() {
            return Err(error(
                ErrorCode::InvalidArgument,
                "empty document cannot be indexed",
            ));
        }
        if doc.chunks.is_empty() || doc.chunks.len() > MAX_CHUNKS {
            return Err(error(ErrorCode::InvalidArgument, "invalid chunk count"));
        }
        if doc
            .valid_from_ms
            .zip(doc.valid_until_ms)
            .is_some_and(|(from, until)| from >= until)
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid document validity interval",
            ));
        }
        let content_hash = hash(doc.content.as_bytes());
        let mut seen = BTreeSet::new();
        let mut chunk_bytes = 0usize;
        let mut token_bytes = 0usize;
        for chunk in &doc.chunks {
            id(&chunk.chunk_id)?;
            id(&chunk.product_scope)?;
            id(&chunk.acl_ref)?;
            chunk_bytes = chunk_bytes.saturating_add(chunk.content.len());
            token_bytes = chunk.tokens.iter().fold(token_bytes, |total, token| {
                total.saturating_add(token.len())
            });
            if chunk_bytes > MAX_CONTENT_BYTES
                || token_bytes > MAX_CONTENT_BYTES
                || chunk.content.trim().is_empty()
                || chunk.source_span.is_empty()
                || chunk.source_span.len() > 256
                || chunk.tokens.len() > MAX_TOKENS_PER_CHUNK
                || chunk.tokens.iter().any(|t| t.is_empty() || t.len() > 64)
                || !seen.insert(chunk.chunk_id.as_str())
            {
                return Err(error(ErrorCode::InvalidArgument, "invalid chunk or token"));
            }
        }
        let revision = document_revision(doc, &content_hash)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, &doc.workspace_id, actor, "knowledge.import", "*")?;
        tx.execute("DELETE FROM knowledge_vectors WHERE workspace_id=?1 AND chunk_id IN(SELECT chunk_id FROM chunks WHERE workspace_id=?1 AND document_id=?2)", params![doc.workspace_id,doc.document_id])?;
        tx.execute("DELETE FROM knowledge_fts WHERE workspace_id=?1 AND chunk_id IN(SELECT chunk_id FROM chunks WHERE workspace_id=?1 AND document_id=?2)", params![doc.workspace_id,doc.document_id])?;
        tx.execute(
            "DELETE FROM chunks WHERE workspace_id=?1 AND document_id=?2",
            params![doc.workspace_id, doc.document_id],
        )?;
        tx.execute(
            "INSERT INTO documents(workspace_id,document_id,source,content_hash,product_scope,status,acl_ref,original_artifact_ref,current_revision) VALUES(?1,?2,?3,?4,?5,'Staged',?6,?7,?8)
              ON CONFLICT(workspace_id,document_id) DO UPDATE SET content_hash=excluded.content_hash,status='Staged',source=excluded.source,product_scope=excluded.product_scope,acl_ref=excluded.acl_ref,original_artifact_ref=excluded.original_artifact_ref,current_revision=excluded.current_revision",
             params![doc.workspace_id, doc.document_id, doc.source, content_hash, doc.product_scope, doc.acl_ref, doc.original_artifact_ref,revision],
        )?;
        tx.execute(
            "INSERT INTO document_revisions(workspace_id,document_id,revision,parser_version,published_at_ms,valid_from_ms,valid_until_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)
              ON CONFLICT(workspace_id,document_id,revision) DO UPDATE SET parser_version=excluded.parser_version,published_at_ms=excluded.published_at_ms,valid_from_ms=excluded.valid_from_ms,valid_until_ms=excluded.valid_until_ms",
             params![doc.workspace_id, doc.document_id, revision, doc.parser_version, utc_ms()?, doc.valid_from_ms, doc.valid_until_ms],
        )?;
        for chunk in &doc.chunks {
            tx.execute(
                "INSERT INTO chunks(workspace_id,chunk_id,document_id,doc_revision,content,source_span,product_scope,acl_ref) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                 params![doc.workspace_id, chunk.chunk_id, doc.document_id, revision, chunk.content, chunk.source_span, chunk.product_scope, chunk.acl_ref],
            )?;
            tx.execute(
                "DELETE FROM knowledge_fts WHERE workspace_id=?1 AND chunk_id=?2",
                params![doc.workspace_id, chunk.chunk_id],
            )?;
            tx.execute(
                "INSERT INTO knowledge_fts(workspace_id,chunk_id,content,tokens) VALUES(?1,?2,?3,?4)",
                params![doc.workspace_id, chunk.chunk_id, chunk.content, chunk.tokens.join(" ")],
            )?;
        }
        audit(
            &tx,
            &doc.workspace_id,
            actor,
            "knowledge.imported",
            &doc.document_id,
            &serde_json::json!({"content_hash": content_hash}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(())
    }

    /// Index identity isolation: rows are scoped by index_id so different
    /// embedding models never mix. Vectors must match the manifest dimension.
    pub fn index_vectors(
        &mut self,
        op: &Operation,
        actor: &str,
        workspace: &str,
        manifest: &VectorManifest,
        vectors: &BTreeMap<String, Vec<f32>>,
    ) -> Result<()> {
        self.start(op)?;
        id(&manifest.index_id)?;
        id(&manifest.embedding_provider)?;
        id(&manifest.model_version)?;
        if manifest.dimension == 0
            || manifest.dimension > MAX_DIMENSION
            || !matches!(manifest.normalization.as_str(), "l2" | "cosine")
            || !is_hash(&manifest.content_hash)
            || vectors.is_empty()
            || vectors.len() > MAX_CHUNKS
        {
            return Err(error(ErrorCode::InvalidArgument, "invalid vector manifest"));
        }
        for (chunk_id, values) in vectors {
            id(chunk_id)?;
            if values.len() != manifest.dimension
                || values.iter().any(|value| !value.is_finite())
                || values.iter().all(|value| *value == 0.0)
            {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "vector dimension mismatch",
                ));
            }
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, workspace, actor, "knowledge.import", "*")?;
        let existing: Option<(String,String,i64,String,String,String)> = tx.query_row("SELECT embedding_provider,model_version,dimension,normalization,chunker_version,content_hash FROM vector_index_manifests WHERE workspace_id=?1 AND index_id=?2",params![workspace,manifest.index_id],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).optional()?;
        let identity = (
            manifest.embedding_provider.clone(),
            manifest.model_version.clone(),
            manifest.dimension as i64,
            manifest.normalization.clone(),
            manifest.chunker_version.clone(),
            manifest.content_hash.clone(),
        );
        if existing.is_some_and(|stored| stored != identity) {
            return Err(error(ErrorCode::Conflict, "vector index identity changed"));
        }
        tx.execute(
            "INSERT INTO vector_index_manifests(workspace_id,index_id,embedding_provider,model_version,dimension,normalization,chunker_version,content_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(workspace_id,index_id) DO UPDATE SET embedding_provider=excluded.embedding_provider,model_version=excluded.model_version,dimension=excluded.dimension,normalization=excluded.normalization,chunker_version=excluded.chunker_version,content_hash=excluded.content_hash",
            params![workspace, manifest.index_id, manifest.embedding_provider, manifest.model_version, sql_int(manifest.dimension as u64)?, manifest.normalization, manifest.chunker_version, manifest.content_hash],
        )?;
        tx.execute(
            "DELETE FROM knowledge_vectors WHERE workspace_id=?1 AND index_id=?2",
            params![workspace, manifest.index_id],
        )?;
        for (chunk_id, values) in vectors {
            let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
            tx.execute(
                "INSERT INTO knowledge_vectors(workspace_id,index_id,chunk_id,vector) VALUES(?1,?2,?3,?4) ON CONFLICT(workspace_id,index_id,chunk_id) DO UPDATE SET vector=excluded.vector",
                params![workspace, manifest.index_id, chunk_id, bytes],
            )?;
        }
        audit(
            &tx,
            workspace,
            actor,
            "knowledge.indexed",
            &manifest.index_id,
            &serde_json::json!({"dimension": manifest.dimension}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(())
    }

    /// Verify the sealed snapshot, fact/permission/active-pointer CAS and commit
    /// the visibility switch together with dependent output invalidation.
    pub fn publish_snapshot(
        &mut self,
        op: &Operation,
        actor: &str,
        publish: &SnapshotPublish,
    ) -> Result<()> {
        crate::publication::publish(self, op, actor, publish)
    }

    /// Revocation removes local index rows and marks documents revoked; remote
    /// deletion is outside local guarantees. Returns revoked chunk count.
    pub fn revoke_knowledge(
        &mut self,
        op: &Operation,
        actor: &str,
        workspace: &str,
        product_scope: Option<&str>,
        document_id: Option<&str>,
    ) -> Result<usize> {
        self.start(op)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        access(&tx, workspace, actor, "knowledge.revoke", "*")?;
        // 读窗口是「上限 + 1」：比上限多读出来的那一行就是「后面还有」的证据。
        let window = MAX_REVOCATION_REVISIONS + 1;
        let revisions = tx.prepare(&format!("SELECT DISTINCT document_id,revision FROM knowledge_snapshot_documents WHERE workspace_id=?1 AND (?2 IS NULL OR product_scope=?2) AND (?3 IS NULL OR document_id=?3) LIMIT {window}"))?.query_map(params![workspace,product_scope,document_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
        if revisions.len() > MAX_REVOCATION_REVISIONS {
            return Err(error(
                ErrorCode::ResourceExhausted,
                "revocation revision bound",
            ));
        }
        for (document, revision) in revisions {
            crate::publication::revoke_revision(
                &tx,
                workspace,
                &document,
                &revision,
                "*",
                "knowledge_revoked",
            )?;
            tx.execute("DELETE FROM knowledge_snapshot_fts WHERE workspace_id=?1 AND (snapshot_id,chunk_id) IN(SELECT snapshot_id,chunk_id FROM knowledge_snapshot_chunks WHERE workspace_id=?1 AND document_id=?2 AND doc_revision=?3)",params![workspace,document,revision])?;
            tx.execute("DELETE FROM knowledge_snapshot_vectors WHERE workspace_id=?1 AND (snapshot_id,chunk_id) IN(SELECT snapshot_id,chunk_id FROM knowledge_snapshot_chunks WHERE workspace_id=?1 AND document_id=?2 AND doc_revision=?3)",params![workspace,document,revision])?;
        }
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(workspace)];
        let mut sql = String::from("SELECT chunk_id FROM chunks WHERE workspace_id=?1");
        if let Some(scope) = product_scope {
            sql.push_str(" AND product_scope=?");
            params.push(Box::new(scope.to_string()));
        }
        if let Some(document) = document_id {
            sql.push_str(" AND document_id=?");
            params.push(Box::new(document.to_string()));
        }
        let chunk_ids: Vec<String> = tx
            .prepare(&sql)?
            .query_map(rusqlite::params_from_iter(params), |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        for chunk in &chunk_ids {
            tx.execute(
                "DELETE FROM knowledge_fts WHERE workspace_id=?1 AND chunk_id=?2",
                params![workspace, chunk],
            )?;
            tx.execute(
                "DELETE FROM knowledge_vectors WHERE workspace_id=?1 AND chunk_id=?2",
                params![workspace, chunk],
            )?;
        }
        let mut clauses = String::from("1=1");
        let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(workspace.to_string())];
        if let Some(scope) = product_scope {
            clauses.push_str(" AND product_scope=?");
            sql_params.push(Box::new(scope.to_string()));
        }
        if let Some(document) = document_id {
            clauses.push_str(" AND document_id=?");
            sql_params.push(Box::new(document.to_string()));
        }
        tx.execute(
            &format!("UPDATE documents SET status='revoked' WHERE workspace_id=?1 AND {clauses}"),
            rusqlite::params_from_iter(sql_params),
        )?;
        for chunk in &chunk_ids {
            tx.execute("UPDATE utterances SET status=12 WHERE workspace_id=?1 AND utterance_id IN (SELECT utterance_id FROM evidence_links WHERE workspace_id=?1 AND evidence_id IN(?2,?3)) AND status NOT IN(11,12,13,14,15)", params![workspace, format!("chunk:{chunk}"), format!("vector:{chunk}")])?;
        }
        tx.execute("UPDATE approvals SET revoked_at_ms=?2 WHERE workspace_id=?1 AND candidate_hash IN (SELECT text_hash FROM utterances WHERE workspace_id=?1 AND status=12) AND revoked_at_ms IS NULL", params![workspace, utc_ms()?])?;
        tx.execute("INSERT INTO invalidation_outbox(workspace_id,reason,resource_id,old_revision,new_revision,created_at_ms) VALUES(?1,'knowledge_revoked',?2,'','revoked',?3)", params![workspace, document_id.or(product_scope).unwrap_or("*"), utc_ms()?])?;
        audit(
            &tx,
            workspace,
            actor,
            "knowledge.revoked",
            product_scope.unwrap_or("*"),
            &serde_json::json!({"chunks": chunk_ids.len()}),
        )?;
        op.check()?;
        tx.commit()?;
        Ok(chunk_ids.len())
    }

    /// Revoked revisions a composition root may submit to a remote adapter after
    /// the local tombstone. Remote failure must not roll back the local one.
    pub fn remote_delete_targets(
        &self,
        op: &Operation,
        workspace: &str,
        kb_scope: &str,
        actor: &str,
    ) -> Result<Vec<(String, String)>> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "knowledge.revoke", "*")?;
        let mut statement = self.conn.prepare(
            "SELECT document_id,revision FROM knowledge_document_revocations WHERE workspace_id=?1 AND (kb_scope=?2 OR kb_scope='*') ORDER BY created_at_ms LIMIT 16",
        )?;
        let rows = statement.query_map(params![workspace, kb_scope], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut targets = Vec::new();
        for row in rows {
            op.check()?;
            targets.push(row?);
            if targets.len() >= 16 {
                break;
            }
        }
        Ok(targets)
    }

    /// Filtered local retrieval: ACL/product/time first, then exact facts,
    /// Chinese lexical (FTS5) and optional vector candidates, fused with exact
    /// facts leading. Never returns misleading "empty" for a timeout.
    #[allow(clippy::too_many_arguments)]
    pub fn retrieve_local(
        &mut self,
        op: &Operation,
        actor: &str,
        workspace: &str,
        kb_scope: &str,
        product_filters: &[String],
        query_tokens: &[String],
        vector: Option<(&str, Vec<f32>)>,
        top_k: usize,
    ) -> Result<(RetrievalStatus, Vec<Evidence>)> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "knowledge.read", kb_scope)?;
        let snapshot:Option<String>=self.conn.query_row("SELECT snapshot_id FROM knowledge_active_snapshots WHERE workspace_id=?1 AND kb_scope=?2",params![workspace,kb_scope],|row|row.get(0)).optional()?;
        let Some(snapshot) = snapshot else {
            return Ok((RetrievalStatus::RetrievalUnavailable, vec![]));
        };
        self.retrieve_snapshot(
            op,
            actor,
            workspace,
            kb_scope,
            &snapshot,
            product_filters,
            query_tokens,
            vector,
            top_k,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn retrieve_session_knowledge(
        &mut self,
        op: &Operation,
        actor: &str,
        workspace: &str,
        session: &str,
        product_filters: &[String],
        query_tokens: &[String],
        vector: Option<(&str, Vec<f32>)>,
        top_k: usize,
    ) -> Result<(RetrievalStatus, Vec<Evidence>)> {
        self.start(op)?;
        access(&self.conn, workspace, actor, "session.manage", "*")?;
        let binding:Option<(String,String)>=self.conn.query_row("SELECT kb_scope,snapshot_id FROM session_knowledge_bindings WHERE workspace_id=?1 AND session_id=?2",params![workspace,session],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
        let Some((kb, snapshot)) = binding else {
            return Ok((RetrievalStatus::RetrievalUnavailable, vec![]));
        };
        self.retrieve_snapshot(
            op,
            actor,
            workspace,
            &kb,
            &snapshot,
            product_filters,
            query_tokens,
            vector,
            top_k,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn retrieve_snapshot(
        &mut self,
        op: &Operation,
        actor: &str,
        workspace: &str,
        kb_scope: &str,
        snapshot: &str,
        product_filters: &[String],
        query_tokens: &[String],
        vector: Option<(&str, Vec<f32>)>,
        top_k: usize,
    ) -> Result<(RetrievalStatus, Vec<Evidence>)> {
        self.start(op)?;
        if top_k == 0
            || top_k > 64
            || query_tokens.is_empty() && vector.is_none()
            || query_tokens.len() > MAX_TOKENS_PER_CHUNK
            || query_tokens
                .iter()
                .any(|token| token.is_empty() || token.len() > 64)
            || product_filters.len() > 64
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "invalid retrieval request",
            ));
        }
        access(&self.conn, workspace, actor, "knowledge.read", kb_scope)?;
        let published:i64=self.conn.query_row("SELECT COUNT(*) FROM knowledge_snapshots WHERE workspace_id=?1 AND snapshot_id=?2 AND kb_scope=?3 AND state='published'",params![workspace,snapshot,kb_scope],|row|row.get(0))?;
        if published != 1 {
            return Ok((RetrievalStatus::RetrievalUnavailable, vec![]));
        }
        let now = utc_ms()?;
        let permission_revision: i64 = self.conn.query_row(
            "SELECT permission_revision FROM workspaces WHERE workspace_id=?1",
            [workspace],
            |row| row.get(0),
        )?;
        let scope = |value: &str| {
            product_filters.is_empty()
                || value.is_empty()
                || product_filters.iter().any(|filter| filter == value)
        };
        let mut facts = Vec::new();
        if !product_filters.is_empty() {
            let mut statement = self.conn.prepare(
                "SELECT s.product_id,s.field_key,s.revision,s.fact_proto FROM knowledge_snapshot_facts s JOIN facts f ON f.workspace_id=s.workspace_id AND f.product_id=s.product_id AND f.field_key=s.field_key AND f.revision=s.revision WHERE s.workspace_id=?1 AND s.snapshot_id=?4 AND (f.valid_from_ms IS NULL OR f.valid_from_ms<=?2) AND (f.valid_until_ms IS NULL OR f.valid_until_ms>?2) AND f.permission_revision=?3 LIMIT 4096",
            )?;
            let rows = statement.query_map(
                params![workspace, now, permission_revision, snapshot],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                },
            )?;
            for row in rows {
                op.check()?;
                let (product, field, revision, blob) = row?;
                if !scope(&product) {
                    continue;
                }
                if let Err(denied) =
                    access(&self.conn, workspace, actor, "knowledge.read", &product)
                {
                    if denied.code == ErrorCode::PermissionDenied {
                        continue;
                    }
                    return Err(denied);
                }
                let fact = AuthoritativeFact::decode(blob.as_slice())
                    .map_err(|_| error(ErrorCode::Internal, "invalid stored fact"))?;
                facts.push(evidence_from_fact(
                    product,
                    field,
                    revision,
                    fact,
                    permission_revision,
                ));
            }
        }
        let mut lexical = Vec::new();
        if !query_tokens.is_empty() {
            let match_query = query_tokens
                .iter()
                .map(|token| format!("\"{}\"", token.replace('"', "")))
                .collect::<Vec<_>>()
                .join(" OR ");
            let mut statement = self.conn.prepare(
                "SELECT f.chunk_id,c.content FROM knowledge_snapshot_fts f JOIN knowledge_snapshot_chunks c ON c.workspace_id=f.workspace_id AND c.snapshot_id=f.snapshot_id AND c.chunk_id=f.chunk_id
                 JOIN knowledge_snapshot_documents d ON d.workspace_id=c.workspace_id AND d.snapshot_id=c.snapshot_id AND d.document_id=c.document_id
                 WHERE f.workspace_id=?1 AND f.snapshot_id=?7 AND f.tokens MATCH ?2 AND (d.valid_from_ms IS NULL OR d.valid_from_ms<=?3) AND (d.valid_until_ms IS NULL OR d.valid_until_ms>?3)
                 AND NOT EXISTS(SELECT 1 FROM knowledge_document_revocations r WHERE r.workspace_id=d.workspace_id AND r.document_id=d.document_id AND r.revision=d.revision AND r.kb_scope IN('*',?8))
                 AND (?5='[]' OR c.product_scope='' OR c.product_scope IN(SELECT value FROM json_each(?5)))
                 AND EXISTS(SELECT 1 FROM permission_grants p WHERE p.workspace_id=c.workspace_id AND p.actor_id=?6 AND p.permission='knowledge.read' AND p.revoked_at_ms IS NULL AND p.resource_scope IN('*',c.acl_ref))
                 AND EXISTS(SELECT 1 FROM permission_grants p WHERE p.workspace_id=d.workspace_id AND p.actor_id=?6 AND p.permission='knowledge.read' AND p.revoked_at_ms IS NULL AND p.resource_scope IN('*',d.acl_ref)) LIMIT ?4",
            )?;
            let rows = statement.query_map(
                params![
                    workspace,
                    match_query,
                    now,
                    (top_k * 2) as i64,
                    serde_json::to_string(product_filters).map_err(|_| error(
                        ErrorCode::InvalidArgument,
                        "invalid product filters"
                    ))?,
                    actor,
                    snapshot,
                    kb_scope
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            for row in rows {
                op.check()?;
                let (chunk_id, content) = row?;
                let _ = content;
                if let Some(evidence) = self.knowledge_evidence(
                    workspace,
                    actor,
                    &chunk_id,
                    product_filters,
                    now,
                    permission_revision,
                    snapshot,
                )? {
                    lexical.push(evidence);
                }
            }
        }
        let mut vec_evidence = Vec::new();
        let mut truncated = false;
        if let Some((index_id, query)) = vector {
            let identity = self.conn.query_row("SELECT embedding_provider,model_version,dimension,normalization,chunker_version FROM knowledge_snapshot_indexes WHERE workspace_id=?1 AND index_id=?2 AND snapshot_id=?3",params![workspace,index_id,snapshot],|row| Ok(nai_knowledge_core::vector::VectorIdentity { embedding_provider:row.get(0)?,model_version:row.get(1)?,dimension:row.get::<_,u32>(2)? as usize,normalization:row.get(3)?,chunker_version:row.get(4)? })).optional()?;
            let Some(identity) = identity else {
                let completed = nai_knowledge_core::rank::fuse(facts, lexical, vec![], top_k);
                let status = if completed.is_empty() {
                    RetrievalStatus::RetrievalUnavailable
                } else {
                    RetrievalStatus::RetrievalPartial
                };
                return Ok((status, completed));
            };
            if query.len() != identity.dimension {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "vector query dimension mismatch",
                ));
            }
            let mut statement = self.conn.prepare(
                "SELECT chunk_id, vector FROM knowledge_snapshot_vectors WHERE workspace_id=?1 AND snapshot_id=?2 LIMIT 4097",
            )?;
            let rows = statement.query_map(params![workspace, snapshot], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?;
            let mut space = nai_knowledge_core::vector::VectorSpace::new(identity);
            for (scanned, row) in rows.enumerate() {
                op.check()?;
                if scanned >= 4096 {
                    truncated = true;
                    break;
                }
                let (chunk_id, bytes) = row?;
                if bytes.len() != query.len() * 4 {
                    continue;
                }
                let values = bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect::<Vec<_>>();
                if self
                    .knowledge_evidence(
                        workspace,
                        actor,
                        &chunk_id,
                        product_filters,
                        now,
                        permission_revision,
                        snapshot,
                    )?
                    .is_some()
                {
                    space
                        .insert(&chunk_id, values)
                        .map_err(|_| error(ErrorCode::Internal, "invalid stored vector"))?;
                }
            }
            for (chunk_id, score) in space
                .top_k(&query, top_k.min(32))
                .map_err(|_| error(ErrorCode::Internal, "vector retrieval failed"))?
            {
                let _ = score;
                if let Some(evidence) = self.knowledge_evidence(
                    workspace,
                    actor,
                    &chunk_id,
                    product_filters,
                    now,
                    permission_revision,
                    snapshot,
                )? {
                    vec_evidence.push(evidence);
                }
            }
        }
        let fused = nai_knowledge_core::rank::fuse(facts, lexical, vec_evidence, top_k);
        op.check()?;
        let status = if truncated {
            RetrievalStatus::RetrievalPartial
        } else if fused.is_empty() {
            RetrievalStatus::RetrievalEmpty
        } else {
            RetrievalStatus::RetrievalComplete
        };
        Ok((status, fused))
    }

    #[allow(clippy::too_many_arguments)]
    fn knowledge_evidence(
        &self,
        workspace: &str,
        actor: &str,
        chunk: &str,
        products: &[String],
        now: i64,
        permission_revision: i64,
        snapshot: &str,
    ) -> Result<Option<Evidence>> {
        let row = self.conn.query_row("SELECT c.document_id,c.doc_revision,c.content,c.source_span,c.product_scope,c.acl_ref,d.valid_until_ms,d.source FROM knowledge_snapshot_chunks c JOIN knowledge_snapshot_documents d ON d.workspace_id=c.workspace_id AND d.snapshot_id=c.snapshot_id AND d.document_id=c.document_id JOIN knowledge_snapshots s ON s.workspace_id=d.workspace_id AND s.snapshot_id=d.snapshot_id WHERE c.workspace_id=?1 AND c.chunk_id=?2 AND c.snapshot_id=?4 AND (d.valid_from_ms IS NULL OR d.valid_from_ms<=?3) AND (d.valid_until_ms IS NULL OR d.valid_until_ms>?3) AND NOT EXISTS(SELECT 1 FROM knowledge_document_revocations r WHERE r.workspace_id=d.workspace_id AND r.document_id=d.document_id AND r.revision=d.revision AND r.kb_scope IN('*',s.kb_scope))", params![workspace,chunk,now,snapshot], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?,row.get::<_,Option<i64>>(6)?,row.get::<_,String>(7)?))).optional()?;
        let Some((document_id, revision, text, source_span, product, acl, valid_until, source)) =
            row
        else {
            return Ok(None);
        };
        if !products.is_empty() && !product.is_empty() && !products.contains(&product) {
            return Ok(None);
        }
        let (document_acl,valid_from): (String,Option<i64>) = self.conn.query_row(
            "SELECT acl_ref,valid_from_ms FROM knowledge_snapshot_documents WHERE workspace_id=?1 AND document_id=?2 AND snapshot_id=?3",
            params![workspace, document_id,snapshot],
            |row| Ok((row.get(0)?,row.get(1)?)),
        )?;
        if let Err(denied) = access(
            &self.conn,
            workspace,
            actor,
            "knowledge.read",
            &document_acl,
        ) {
            if denied.code == ErrorCode::PermissionDenied {
                return Ok(None);
            }
            return Err(denied);
        }
        if let Err(denied) = access(&self.conn, workspace, actor, "knowledge.read", &acl) {
            if denied.code == ErrorCode::PermissionDenied {
                return Ok(None);
            }
            return Err(denied);
        }
        Ok(Some(Evidence {
            evidence_id: format!("chunk:{chunk}"),
            document_id,
            revision,
            chunk_id: chunk.into(),
            text,
            source_span,
            product_id: product,
            source_ref: source,
            valid_until_utc_ms: valid_until.and_then(|value| u64::try_from(value).ok()),
            valid_from_utc_ms: valid_from.and_then(|value| u64::try_from(value).ok()),
            permission_revision: permission_revision.to_string(),
            original_verifiable: true,
            ..Default::default()
        }))
    }
}

fn evidence_from_fact(
    product: String,
    field: String,
    revision: String,
    fact: AuthoritativeFact,
    permission_revision: i64,
) -> Evidence {
    let versions = vec![nai_contracts::v1::RevisionRef {
        object_id: format!("{product}:{field}"),
        revision: revision.clone(),
    }];
    let text = match fact.value {
        Some(authoritative_fact::Value::Text(value)) => value,
        Some(authoritative_fact::Value::Integer(value)) => value.to_string(),
        Some(authoritative_fact::Value::Decimal(value)) => value,
        Some(authoritative_fact::Value::Flag(value)) => value.to_string(),
        Some(authoritative_fact::Value::Money(value)) => {
            format!("{} {}", value.minor_units, value.currency)
        }
        None => String::new(),
    };
    Evidence {
        evidence_id: format!("fact:{product}:{field}:{revision}"),
        document_id: product.clone(),
        revision,
        text,
        source_ref: format!("fact:{product}:{field}"),
        authority: fact.authority_ref,
        permission_revision: permission_revision.to_string(),
        original_verifiable: true,
        valid_from_utc_ms: fact.valid_from_utc_ms,
        valid_until_utc_ms: fact.valid_until_utc_ms,
        fact_versions: versions,
        ..Default::default()
    }
}

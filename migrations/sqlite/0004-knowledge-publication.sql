ALTER TABLE documents ADD COLUMN current_revision TEXT;
UPDATE documents SET current_revision=content_hash;
CREATE TABLE knowledge_snapshot_builds(
  workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL,
  expected_active TEXT, permission_revision INTEGER NOT NULL,
  PRIMARY KEY(workspace_id,snapshot_id),
  FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,snapshot_id)
) STRICT;
CREATE TABLE knowledge_active_snapshots(
  workspace_id TEXT NOT NULL, kb_scope TEXT NOT NULL, snapshot_id TEXT NOT NULL,
  PRIMARY KEY(workspace_id,kb_scope),
  FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,snapshot_id)
) STRICT;
CREATE TABLE knowledge_snapshot_documents(
  workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, document_id TEXT NOT NULL,
  revision TEXT NOT NULL, source TEXT NOT NULL, parser_version TEXT NOT NULL,
  product_scope TEXT NOT NULL, acl_ref TEXT NOT NULL, original_artifact_ref TEXT,
  valid_from_ms INTEGER, valid_until_ms INTEGER,
  PRIMARY KEY(workspace_id,snapshot_id,document_id),
  FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,snapshot_id)
) STRICT;
CREATE TABLE knowledge_snapshot_chunks(
  workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, chunk_id TEXT NOT NULL,
  document_id TEXT NOT NULL, doc_revision TEXT NOT NULL, content TEXT NOT NULL,
  source_span TEXT NOT NULL, product_scope TEXT NOT NULL, acl_ref TEXT NOT NULL, tokens TEXT NOT NULL,
  PRIMARY KEY(workspace_id,snapshot_id,chunk_id),
  FOREIGN KEY(workspace_id,snapshot_id,document_id) REFERENCES knowledge_snapshot_documents(workspace_id,snapshot_id,document_id)
) STRICT;
CREATE VIRTUAL TABLE knowledge_snapshot_fts USING fts5(
  workspace_id UNINDEXED, snapshot_id UNINDEXED, chunk_id UNINDEXED, tokens, tokenize='unicode61'
);
CREATE TABLE knowledge_snapshot_indexes(
  workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, index_id TEXT NOT NULL,
  embedding_provider TEXT NOT NULL, model_version TEXT NOT NULL, dimension INTEGER NOT NULL,
  normalization TEXT NOT NULL, chunker_version TEXT NOT NULL, content_hash TEXT NOT NULL,
  PRIMARY KEY(workspace_id,snapshot_id),
  FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,snapshot_id)
) STRICT;
CREATE TABLE knowledge_snapshot_vectors(
  workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, chunk_id TEXT NOT NULL, vector BLOB NOT NULL,
  PRIMARY KEY(workspace_id,snapshot_id,chunk_id),
  FOREIGN KEY(workspace_id,snapshot_id,chunk_id) REFERENCES knowledge_snapshot_chunks(workspace_id,snapshot_id,chunk_id),
  FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshot_indexes(workspace_id,snapshot_id)
) STRICT;
CREATE TABLE knowledge_snapshot_facts(
  workspace_id TEXT NOT NULL, snapshot_id TEXT NOT NULL, product_id TEXT NOT NULL,
  field_key TEXT NOT NULL, revision TEXT NOT NULL, fact_proto BLOB NOT NULL,
  PRIMARY KEY(workspace_id,snapshot_id,product_id,field_key),
  FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,snapshot_id)
) STRICT;
CREATE TABLE knowledge_document_revocations(
  workspace_id TEXT NOT NULL REFERENCES workspaces, document_id TEXT NOT NULL,
  revision TEXT NOT NULL, kb_scope TEXT NOT NULL, reason TEXT NOT NULL, created_at_ms INTEGER NOT NULL,
  PRIMARY KEY(workspace_id,document_id,revision,kb_scope)
) STRICT;
CREATE TABLE session_knowledge_bindings(
  workspace_id TEXT NOT NULL, session_id TEXT NOT NULL, kb_scope TEXT NOT NULL, snapshot_id TEXT NOT NULL,
  PRIMARY KEY(workspace_id,session_id),
  FOREIGN KEY(workspace_id,session_id) REFERENCES sessions(workspace_id,session_id),
  FOREIGN KEY(workspace_id,snapshot_id) REFERENCES knowledge_snapshots(workspace_id,snapshot_id)
) STRICT;

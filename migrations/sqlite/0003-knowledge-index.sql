CREATE VIRTUAL TABLE knowledge_fts USING fts5(
  workspace_id UNINDEXED, chunk_id UNINDEXED, content, tokens, tokenize='unicode61'
);
CREATE TABLE knowledge_vectors(
  workspace_id TEXT NOT NULL,
  index_id TEXT NOT NULL,
  chunk_id TEXT NOT NULL,
  vector BLOB NOT NULL,
  PRIMARY KEY(workspace_id,index_id,chunk_id),
  FOREIGN KEY(workspace_id,index_id) REFERENCES vector_index_manifests(workspace_id,index_id),
  FOREIGN KEY(workspace_id,chunk_id) REFERENCES chunks(workspace_id,chunk_id)
) STRICT;
CREATE INDEX knowledge_vectors_chunk ON knowledge_vectors(workspace_id, chunk_id);

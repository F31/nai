-- Additive single-owner extension. Existing business rows are never migrated or replaced.
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS public_media_approvals(
 workspace_id TEXT NOT NULL,
 approval_id TEXT NOT NULL,
 session_id TEXT NOT NULL,
 epoch INTEGER NOT NULL CHECK(epoch>0),
 principal_id TEXT NOT NULL,
 role_revision INTEGER NOT NULL,
 bundle_sha256 TEXT NOT NULL CHECK(length(bundle_sha256)=64),
 bundle BLOB NOT NULL CHECK(length(bundle)<=8192),
 state TEXT NOT NULL CHECK(state IN('approved','revoked')),
 expires_tick INTEGER NOT NULL,
 expires_at INTEGER NOT NULL,
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 PRIMARY KEY(workspace_id,approval_id),
 FOREIGN KEY(workspace_id,session_id) REFERENCES sessions(workspace_id,id)
);
CREATE INDEX IF NOT EXISTS public_media_approvals_active ON public_media_approvals(workspace_id,state,expires_tick);
COMMIT;

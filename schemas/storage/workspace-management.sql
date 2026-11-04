-- Single-owner workspace context, retained financial facts and resumable cleanup.
CREATE TABLE IF NOT EXISTS workspace_selection(principal_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS workspace_retained_budget(workspace_id TEXT NOT NULL, id TEXT NOT NULL, principal_id TEXT NOT NULL, reserved_minor INTEGER NOT NULL, actual_minor INTEGER, status TEXT NOT NULL, currency TEXT NOT NULL, revision INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY(workspace_id,id));
CREATE TABLE IF NOT EXISTS workspace_purges(workspace_id TEXT PRIMARY KEY, revision INTEGER NOT NULL, plan_sha256 TEXT NOT NULL, plan BLOB NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS workspace_purge_files(workspace_id TEXT NOT NULL REFERENCES workspace_purges(workspace_id), sha256 TEXT NOT NULL, PRIMARY KEY(workspace_id,sha256));

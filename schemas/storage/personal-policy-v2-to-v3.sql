-- Execute only inside the verified owner's migration transaction.
-- Workspaces and all financial / approval tables remain untouched.
CREATE TABLE personal_live_preferences (
    workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    body TEXT NOT NULL CHECK(length(CAST(body AS BLOB)) BETWEEN 1 AND 4096),
    revision INTEGER NOT NULL CHECK(revision BETWEEN 1 AND 9223372036854775807),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
PRAGMA user_version = 3;

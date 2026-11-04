-- Proposed isolated authority tables. Not installed in existing databases until
-- the backup/journal migration and trusted owner integration are implemented.
CREATE TABLE avatar_bindings (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
  profile_id TEXT NOT NULL,
  package_sha256 TEXT NOT NULL,
  rights_sha256 TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK(revision > 0),
  active INTEGER NOT NULL CHECK(active IN (0,1))
);
CREATE TABLE avatar_rights_revocations (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  package_sha256 TEXT NOT NULL,
  rights_sha256 TEXT NOT NULL,
  PRIMARY KEY(workspace_id, package_sha256, rights_sha256)
);

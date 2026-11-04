-- Desktop-owned database, separate from the engine Store. Never stores plaintext keys.
CREATE TABLE IF NOT EXISTS provider_settings (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  scene_json TEXT NOT NULL CHECK (length(scene_json) <= 1048576)
);
CREATE TABLE IF NOT EXISTS provider_credentials (
  provider_id TEXT PRIMARY KEY,
  identity TEXT NOT NULL,
  nonce BLOB NOT NULL CHECK (length(nonce) = 12),
  ciphertext BLOB NOT NULL CHECK (length(ciphertext) BETWEEN 17 AND 2576)
);
PRAGMA user_version = 1;

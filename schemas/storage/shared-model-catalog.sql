-- Additive, single-owner catalogue extension. No business-workspace foreign key.
CREATE TABLE IF NOT EXISTS shared_model_migration(id TEXT PRIMARY KEY, request BLOB NOT NULL, plan_sha256 TEXT NOT NULL, stage TEXT NOT NULL CHECK(stage IN ('preparing','rewrapping','committed','aborted')), backup_attempts INTEGER NOT NULL DEFAULT 0 CHECK(backup_attempts BETWEEN 0 AND 8), backup_id TEXT NOT NULL DEFAULT '', backup_manifest_sha256 TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS shared_model_staging(migration_id TEXT NOT NULL REFERENCES shared_model_migration(id), model_id TEXT NOT NULL, capability TEXT NOT NULL CHECK(capability IN ('llm','tts')), attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 3), body TEXT, PRIMARY KEY(migration_id,model_id));
CREATE TABLE IF NOT EXISTS shared_models(id TEXT PRIMARY KEY, revision INTEGER NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('publication_llm_config','publication_tts_config')));
CREATE TABLE IF NOT EXISTS shared_model_versions(model_id TEXT NOT NULL REFERENCES shared_models(id), revision INTEGER NOT NULL, sha256 TEXT NOT NULL, body TEXT NOT NULL CHECK(length(body)<=8192), PRIMARY KEY(model_id,revision));



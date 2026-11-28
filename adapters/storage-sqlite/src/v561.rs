//! V5.6.1 single data root and minimal SQLite schema.
//!
//! This module intentionally does not inherit the V4.8 workspace / role /
//! approval / budget / ledger / outbox schema. It is the storage seam for the
//! single-operator, single-room V5.6.1 baseline.

use nai_runtime_core::validate_data_root_path;
use rusqlite::{Connection, OpenFlags, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    error::Error as StdError,
    fmt,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const SCHEMA_VERSION: i64 = 2;

#[derive(Debug)]
pub enum V561StoreError {
    Io(std::io::Error),
    Sql(rusqlite::Error),
    Json(serde_json::Error),
    UnsafePath(String),
    MissingParent(PathBuf),
    InvalidConfig(String),
    IncompatibleDatabase,
}

impl fmt::Display for V561StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Sql(e) => write!(f, "sql: {e}"),
            Self::Json(e) => write!(f, "json: {e}"),
            Self::UnsafePath(p) => write!(f, "unsafe path: {p}"),
            Self::MissingParent(p) => write!(f, "missing parent: {}", p.display()),
            Self::InvalidConfig(s) => write!(f, "invalid config: {s}"),
            Self::IncompatibleDatabase => write!(
                f,
                "incompatible or unverified data root; no migration performed"
            ),
        }
    }
}

impl StdError for V561StoreError {}

impl From<std::io::Error> for V561StoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for V561StoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sql(value)
    }
}

impl From<serde_json::Error> for V561StoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub type Result<T> = std::result::Result<T, V561StoreError>;

#[derive(Debug, Clone, Default)]
pub struct V561OpenOptions {
    /// Optional config to write on first open. Callers can inject deployment or
    /// test config without changing the adapter. Existing config is never
    /// overwritten by open().
    pub config: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V561AssetKind {
    Portrait,
    Voice,
    Knowledge,
}

impl V561AssetKind {
    fn dir(self) -> &'static str {
        match self {
            Self::Portrait => "assets/characters",
            Self::Voice => "assets/voices",
            Self::Knowledge => "assets/knowledge",
        }
    }

    fn max_bytes(self) -> u64 {
        match self {
            Self::Portrait => 20 * 1024 * 1024,
            Self::Voice => 50 * 1024 * 1024,
            Self::Knowledge => 10 * 1024 * 1024,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Portrait => "portrait",
            Self::Voice => "voice",
            Self::Knowledge => "knowledge",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct V561ImportOptions<'a> {
    pub name: Option<&'a str>,
    pub license_note: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V561ImportedAsset {
    pub id: String,
    pub kind: V561AssetKind,
    pub path: PathBuf,
    pub hash: String,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct V561DataRoot {
    root: PathBuf,
    config: Value,
}

impl V561DataRoot {
    /// Create/open a single data root:
    /// `config.json`, `nai.db`, `assets/`, `logs/`, `recordings/`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(root, V561OpenOptions::default())
    }

    pub fn open_with(root: impl AsRef<Path>, options: V561OpenOptions) -> Result<Self> {
        let root = root.as_ref();
        validate_data_root_path(root)
            .map_err(|_| V561StoreError::UnsafePath(root.display().to_string()))?;
        // Refuse existing foreign databases before creating directories/config or
        // applying any writable SQLite pragmas. Legacy data is never migrated.
        if root.exists() {
            let existing_root = root.canonicalize()?;
            // Older foundation roots use a different database filename.
            match std::fs::symlink_metadata(existing_root.join("foundation.sqlite")) {
                Ok(_) => return Err(V561StoreError::IncompatibleDatabase),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            let existing_db = existing_root.join("nai.db");
            match std::fs::symlink_metadata(&existing_db) {
                Ok(meta) => {
                    if !meta.is_file() || meta.file_type().is_symlink() {
                        return Err(V561StoreError::IncompatibleDatabase);
                    }
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::MetadataExt;
                        if meta.file_attributes() & 0x400 != 0 {
                            return Err(V561StoreError::IncompatibleDatabase);
                        }
                    }
                    verify_existing_database(&existing_db)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        std::fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        for dir in [
            "assets",
            "assets/characters",
            "assets/voices",
            "assets/knowledge",
            "assets/clips",
            "logs",
            "recordings",
            "cache",
        ] {
            std::fs::create_dir_all(root.join(dir))?;
        }
        let config = options
            .config
            .clone()
            .unwrap_or_else(|| default_config(&root));
        if !config.is_object() {
            return Err(V561StoreError::InvalidConfig(
                "root must be an object".into(),
            ));
        }
        validate_no_plaintext_secrets(&config)?;
        let data = Self {
            root,
            config: config.clone(),
        };
        let config_path = data.resolve("config.json")?;
        if !config_path.exists() {
            data.write_json(&config_path, &config)?;
        }
        let db_path = data.resolve("nai.db")?;
        let conn = Connection::open(db_path)?;
        configure(&conn)?;
        create_schema(&conn)?;
        Ok(data)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_path(&self) -> Result<PathBuf> {
        self.resolve("config.json")
    }

    pub fn db_path(&self) -> Result<PathBuf> {
        self.resolve("nai.db")
    }

    pub fn assets_dir(&self) -> Result<PathBuf> {
        self.resolve("assets")
    }

    pub fn logs_dir(&self) -> Result<PathBuf> {
        self.resolve("logs")
    }

    pub fn recordings_dir(&self) -> Result<PathBuf> {
        self.resolve("recordings")
    }

    /// Resolve a relative path inside the data root. Absolute paths, `..`,
    /// prefixes, and symlink escapes are rejected.
    pub fn resolve(&self, relative: impl AsRef<Path>) -> Result<PathBuf> {
        let relative = relative.as_ref();
        if relative.is_absolute() {
            return Err(V561StoreError::UnsafePath(relative.display().to_string()));
        }
        let mut out = self.root.clone();
        for component in relative.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => out.push(part),
                _ => return Err(V561StoreError::UnsafePath(relative.display().to_string())),
            }
        }
        let parent = out
            .parent()
            .ok_or_else(|| V561StoreError::MissingParent(out.clone()))?;
        let parent = parent.canonicalize()?;
        if !parent.starts_with(&self.root) {
            return Err(V561StoreError::UnsafePath(relative.display().to_string()));
        }
        Ok(out)
    }

    pub fn load_config(&self) -> Result<Value> {
        let path = self.config_path()?;
        let raw = std::fs::read_to_string(path)?;
        let value: Value = serde_json::from_str(&raw)?;
        if !value.is_object() {
            return Err(V561StoreError::InvalidConfig(
                "root must be an object".into(),
            ));
        }
        Ok(value)
    }

    pub fn save_config(&self, value: &Value) -> Result<()> {
        if !value.is_object() {
            return Err(V561StoreError::InvalidConfig(
                "root must be an object".into(),
            ));
        }
        validate_no_plaintext_secrets(value)?;
        self.write_json(&self.config_path()?, value)
    }

    /// Config safe to show in UI/logs: secret-shaped values are masked, but
    /// keyring references such as `api_key_ref` remain visible.
    pub fn redacted_config(&self) -> Result<Value> {
        Ok(redact_value(&self.load_config()?))
    }

    /// Restore the V5.6.1 default config. Callers should keep a backup first.
    pub fn reset_config(&self) -> Result<Value> {
        let value = default_config(&self.root);
        self.write_json(&self.config_path()?, &value)?;
        Ok(value)
    }

    pub fn initial_config(&self) -> &Value {
        &self.config
    }

    /// Manually import a user asset. Old databases/license structures are not
    /// read; only the file bytes and an explicit new license note are stored.
    pub fn import_asset(
        &self,
        kind: V561AssetKind,
        source: impl AsRef<Path>,
        options: V561ImportOptions<'_>,
    ) -> Result<V561ImportedAsset> {
        let source = source.as_ref();
        let metadata = std::fs::symlink_metadata(source)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(V561StoreError::InvalidConfig(format!(
                "asset source must be a regular file: {}",
                source.display()
            )));
        }
        if metadata.len() == 0 || metadata.len() > kind.max_bytes() {
            return Err(V561StoreError::InvalidConfig(format!(
                "asset size {} out of range for {}",
                metadata.len(),
                kind.label()
            )));
        }
        let bytes = std::fs::read(source)?;
        if bytes.is_empty() || bytes.len() as u64 > kind.max_bytes() {
            return Err(V561StoreError::InvalidConfig(
                "asset changed while reading".into(),
            ));
        }
        let hash = hex_sha256(&bytes);
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        let id = format!("{}-{}", kind.label(), &hash[..16]);
        let relative = format!("{}/{}{}", kind.dir(), id, extension);
        let destination = self.resolve(&relative)?;
        std::fs::write(&destination, &bytes)?;
        let name = options
            .name
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                source
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| id.clone());
        let license_note = options.license_note.unwrap_or("user confirmed at import");
        let now = now_ms()?;
        let conn = self.open_db()?;
        match kind {
            V561AssetKind::Portrait => {
                conn.execute(
                    "INSERT OR REPLACE INTO characters(id,name,source_image,driving_idle,license_note,source_hash,created_at)
                     VALUES(?1,?2,?3,NULL,?4,?5,?6)",
                    params![id, name, relative, license_note, hash, now],
                )?;
            }
            V561AssetKind::Voice => {
                conn.execute(
                    "INSERT OR REPLACE INTO voices(id,name,ref_audio,params_json,source_hash,created_at)
                     VALUES(?1,?2,?3,'{}',?4,?5)",
                    params![id, name, relative, hash, now],
                )?;
            }
            V561AssetKind::Knowledge => {
                let size = i64::try_from(bytes.len()).map_err(|_| {
                    V561StoreError::InvalidConfig("knowledge asset too large".into())
                })?;
                conn.execute(
                    "INSERT OR REPLACE INTO knowledge_docs(id,title,path,hash,bytes,created_at)
                     VALUES(?1,?2,?3,?4,?5,?6)",
                    params![id, name, relative, hash, size, now],
                )?;
            }
        }
        Ok(V561ImportedAsset {
            id,
            kind,
            path: destination,
            hash,
            bytes: bytes.len() as u64,
        })
    }

    pub fn open_db(&self) -> Result<Connection> {
        let db = self.db_path()?;
        verify_existing_database(&db)?;
        let conn = Connection::open(db)?;
        configure(&conn)?;
        create_schema(&conn)?;
        Ok(conn)
    }

    fn write_json(&self, path: &Path, value: &Value) -> Result<()> {
        let raw = serde_json::to_string_pretty(value)?;
        std::fs::write(path, format!("{raw}\n"))?;
        Ok(())
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "api_key"
            | "apikey"
            | "api_keys"
            | "secret"
            | "secret_key"
            | "client_secret"
            | "token"
            | "access_token"
            | "refresh_token"
            | "password"
            | "authorization"
            | "cookie"
            | "credential"
    ) || key.ends_with("_secret")
        || key.ends_with("_token")
        || key.ends_with("_password")
        || key.ends_with("_api_key")
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_sensitive_key(key) && !value.is_null() {
                        (key.clone(), Value::String("***".into()))
                    } else {
                        (key.clone(), redact_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}

fn validate_no_plaintext_secrets(value: &Value) -> Result<()> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if is_sensitive_key(key) && !value.is_null() && value.as_str() != Some("***") {
                    return Err(V561StoreError::InvalidConfig(format!(
                        "plaintext secret at {key}; store it in the OS keyring and keep only a *_ref"
                    )));
                }
                validate_no_plaintext_secrets(value)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_no_plaintext_secrets(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn default_config(root: &Path) -> Value {
    json!({
        "app": {
            "data_root": root.to_string_lossy(),
            "language": "zh-CN",
            "log_retention_days": 7
        },
        "server": {"host": "127.0.0.1", "port": 8780},
        "llm": {
            "base_url": "https://api.deepseek.com",
            "model": "deepseek-chat",
            "api_key_ref": "keyring:nai/llm",
            "temperature": 0.7,
            "max_tokens": 512
        },
        "tts": {
            "engine": "qwen3",
            "protocol": "qwen_local",
            "model_size": "1.7B",
            "model": "qwen3-tts-1.7b-customvoice",
            "endpoint": "http://127.0.0.1:8000/v1",
            "voice": "vivian",
            "language": "chinese",
            "stream": false,
            "timeout_ms": 45000,
            "prefetch_sentences": 2
        },
        "avatar": {
            "engine": "flashhead",
            "mode": "lite",
            "model_dir": "assets/models/flashhead",
            "worker_python": "python",
            "output": {"width": 1280, "height": 720, "fps": 25}
        },
        "output": {
            "window": true,
            "always_on_top": true,
            "obs_capture_hint": true,
            "obs_path": null,
            "audio_device": null,
            "virtual_camera": false
        },
        "filters": {
            "redline": [],
            "blocked_actions": [],
            "max_danmaku_per_minute": 20
        }
    })
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "cache_size", -32768_i64)?;
    conn.pragma_update(None, "mmap_size", 268_435_456_i64)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// Read-only identification, not a migration. Reject incomplete or foreign schemas.
fn verify_existing_database(path: &Path) -> Result<()> {
    let inspect = || -> std::result::Result<bool, rusqlite::Error> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            return Ok(false);
        }
        let mut statement = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let expected = [
            "actions",
            "characters",
            "clips",
            "events",
            "knowledge_docs",
            "messages",
            "metrics",
            "sessions",
            "settings",
            "voices",
        ];
        if names.iter().map(String::as_str).ne(expected) {
            return Ok(false);
        }
        let schemas: &[(&str, &[&str])] = &[
            ("actions", &["id", "session_id", "action", "source", "at"]),
            (
                "characters",
                &[
                    "id",
                    "name",
                    "source_image",
                    "driving_idle",
                    "license_note",
                    "source_hash",
                    "created_at",
                ],
            ),
            (
                "clips",
                &[
                    "id",
                    "action",
                    "path",
                    "hash",
                    "duration_ms",
                    "loop",
                    "license_note",
                    "created_at",
                ],
            ),
            (
                "events",
                &["id", "session_id", "kind", "payload_json", "at"],
            ),
            (
                "knowledge_docs",
                &["id", "title", "path", "hash", "bytes", "created_at"],
            ),
            (
                "messages",
                &[
                    "id",
                    "session_id",
                    "role",
                    "text",
                    "audio_path",
                    "started_at",
                    "duration_ms",
                    "source",
                ],
            ),
            (
                "metrics",
                &["id", "session_id", "name", "value", "unit", "at"],
            ),
            (
                "sessions",
                &[
                    "id",
                    "started_at",
                    "ended_at",
                    "mode",
                    "config_json",
                    "metrics_json",
                ],
            ),
            ("settings", &["key", "value_json", "updated_at"]),
            (
                "voices",
                &[
                    "id",
                    "name",
                    "ref_audio",
                    "params_json",
                    "source_hash",
                    "created_at",
                ],
            ),
        ];
        for (table, expected_columns) in schemas {
            let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
            let cols = stmt
                .query_map([table], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if cols
                .iter()
                .map(String::as_str)
                .ne(expected_columns.iter().copied())
            {
                return Ok(false);
            }
        }
        let stored: String = conn.query_row(
            "SELECT value_json FROM settings WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )?;
        Ok(stored == SCHEMA_VERSION.to_string())
    };
    match inspect() {
        Ok(true) => Ok(()),
        _ => Err(V561StoreError::IncompatibleDatabase),
    }
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS characters (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            source_image TEXT,
            driving_idle TEXT,
            license_note TEXT,
            source_hash TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS voices (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            ref_audio TEXT,
            params_json TEXT NOT NULL DEFAULT '{}',
            source_hash TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS knowledge_docs (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            path TEXT NOT NULL,
            hash TEXT NOT NULL,
            bytes INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            mode TEXT,
            config_json TEXT NOT NULL DEFAULT '{}',
            metrics_json TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            text TEXT NOT NULL,
            audio_path TEXT,
            started_at INTEGER NOT NULL,
            duration_ms INTEGER,
            source TEXT
        );
        CREATE TABLE IF NOT EXISTS actions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            action TEXT NOT NULL,
            source TEXT,
            at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT,
            kind TEXT NOT NULL,
            payload_json TEXT NOT NULL DEFAULT '{}',
            at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS clips (
            id TEXT PRIMARY KEY,
            action TEXT NOT NULL,
            path TEXT NOT NULL,
            hash TEXT NOT NULL,
            duration_ms INTEGER,
            loop INTEGER NOT NULL DEFAULT 0,
            license_note TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT,
            name TEXT NOT NULL,
            value REAL,
            unit TEXT,
            at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, started_at);
        CREATE INDEX IF NOT EXISTS idx_actions_session ON actions(session_id, at);
        CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id, at);
        CREATE INDEX IF NOT EXISTS idx_metrics_session ON metrics(session_id, at);
        "#,
    )?;
    let now = now_ms()?;
    conn.execute(
        "INSERT INTO settings(key, value_json, updated_at) VALUES('schema_version', ?1, ?2)
         ON CONFLICT(key) DO NOTHING",
        params![SCHEMA_VERSION.to_string(), now],
    )?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

fn now_ms() -> Result<i64> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| V561StoreError::InvalidConfig(e.to_string()))?
        .as_millis();
    i64::try_from(ms).map_err(|e| V561StoreError::InvalidConfig(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn creates_single_root_and_default_config() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("open root");
        for name in ["config.json", "nai.db", "assets", "logs", "recordings"] {
            assert!(root.root().join(name).exists(), "{name}");
        }
        let config = root.load_config().expect("config");
        assert_eq!(config["avatar"]["engine"], "flashhead");
        assert_eq!(config["tts"]["model_size"], "1.7B");
        assert_eq!(config["tts"]["model"], "qwen3-tts-1.7b-customvoice");
        assert_eq!(config["tts"]["endpoint"], "http://127.0.0.1:8000/v1");
        assert_eq!(config["tts"]["stream"], false);
    }

    #[test]
    fn creates_only_the_minimal_v561_tables() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("open root");
        let conn = root.open_db().expect("db");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for expected in [
            "settings",
            "characters",
            "voices",
            "sessions",
            "messages",
            "actions",
            "events",
            "clips",
            "metrics",
            "knowledge_docs",
        ] {
            assert!(tables.iter().any(|t| t == expected), "missing {expected}");
        }
        for forbidden in [
            "workspace",
            "roles",
            "approval",
            "budget",
            "ledger",
            "outbox",
            "audit",
        ] {
            assert!(
                !tables.iter().any(|t| t == forbidden),
                "forbidden {forbidden}"
            );
        }
        let version: String = conn
            .query_row(
                "SELECT value_json FROM settings WHERE key='schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn rejects_foreign_database_without_modifying_root_or_version() {
        let dir = temp_root();
        let path = dir.path().join("nai.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE old_workspace (id TEXT); PRAGMA user_version=1;")
            .unwrap();
        drop(conn);
        let original = std::fs::read(&path).unwrap();
        assert!(matches!(
            V561DataRoot::open(dir.path()),
            Err(V561StoreError::IncompatibleDatabase)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!dir.path().join("config.json").exists());
        assert!(!dir.path().join("assets").exists());
    }

    #[test]
    fn rejects_legacy_foundation_root_before_creating_any_v561_files() {
        let dir = temp_root();
        std::fs::write(dir.path().join("foundation.sqlite"), b"legacy data").unwrap();
        assert!(matches!(
            V561DataRoot::open(dir.path()),
            Err(V561StoreError::IncompatibleDatabase)
        ));
        assert_eq!(
            std::fs::read(dir.path().join("foundation.sqlite")).unwrap(),
            b"legacy data"
        );
        assert!(!dir.path().join("nai.db").exists());
        assert!(!dir.path().join("config.json").exists());
    }

    #[test]
    fn rejects_unknown_or_incomplete_current_version_database() {
        let dir = temp_root();
        let path = dir.path().join("nai.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY, value_json TEXT, updated_at INTEGER); INSERT INTO settings VALUES('schema_version','2',0); PRAGMA user_version=2;").unwrap();
        drop(conn);
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(
            V561DataRoot::open(dir.path()),
            Err(V561StoreError::IncompatibleDatabase)
        ));
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(!dir.path().join("assets").exists());
    }

    #[test]
    fn open_is_idempotent() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("open root");
        let conn = root.open_db().expect("db");
        drop(conn);
        let root2 = V561DataRoot::open(dir.path()).expect("reopen root");
        assert_eq!(root.root(), root2.root());
        assert!(root2.db_path().unwrap().exists());
    }

    #[test]
    fn accepts_injected_config_and_saves_it() {
        let dir = temp_root();
        let config = json!({
            "app": {"data_root": dir.path().to_string_lossy()},
            "avatar": {"engine": "flashhead", "mode": "lite"},
            "tts": {"engine": "qwen3", "model_size": "0.6B"}
        });
        let root = V561DataRoot::open_with(
            dir.path(),
            V561OpenOptions {
                config: Some(config.clone()),
            },
        )
        .expect("open with config");
        assert_eq!(root.load_config().unwrap(), config);
        let mut updated = config;
        updated["avatar"]["mode"] = json!("pro");
        root.save_config(&updated).expect("save config");
        assert_eq!(root.load_config().unwrap()["avatar"]["mode"], "pro");
        let reopened = V561DataRoot::open(dir.path()).expect("reopen");
        let reopened_config = reopened.load_config().unwrap();
        assert_eq!(reopened_config["avatar"]["mode"], "pro");
        assert_eq!(reopened_config["tts"]["model_size"], "0.6B");
    }

    #[test]
    fn rejects_relative_root_and_sets_performance_pragmas() {
        let relative = Path::new("relative-v561-root");
        assert!(V561DataRoot::open(relative).is_err());
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("open root");
        let conn = root.open_db().expect("db");
        let journal: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal.to_lowercase(), "wal");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let synchronous: i64 = conn
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .unwrap();
        assert!(synchronous <= 1);
    }

    #[test]
    fn redacts_secrets_but_keeps_keyring_refs_and_resets() {
        let dir = temp_root();
        let config = json!({
            "llm": {"api_key": "plain", "api_key_ref": "keyring:nai/llm"},
            "tts": {"token": "plain-token"},
            "nested": [{"password": "p"}, {"ok": "visible"}]
        });
        // open_with rejects plaintext secrets.
        let opened = V561DataRoot::open_with(
            dir.path(),
            V561OpenOptions {
                config: Some(config.clone()),
            },
        );
        assert!(opened.is_err());
        let root = V561DataRoot::open(dir.path()).expect("default root");
        root.save_config(&json!({"llm": {"api_key_ref": "keyring:nai/llm"}}))
            .unwrap();
        let redacted = root.redacted_config().unwrap();
        assert_eq!(redacted["llm"]["api_key_ref"], "keyring:nai/llm");
        let reset = root.reset_config().unwrap();
        assert_eq!(reset["avatar"]["engine"], "flashhead");
        assert_eq!(root.load_config().unwrap()["avatar"]["engine"], "flashhead");
    }

    #[test]
    fn rejects_plaintext_secret_on_save() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("root");
        assert!(
            root.save_config(&json!({"llm": {"api_key": "plain"}}))
                .is_err()
        );
    }

    #[test]
    fn imports_user_assets_with_hash_and_no_old_license_import() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("root");
        let source = dir.path().join("portrait.png");
        std::fs::write(&source, b"not-a-real-png-but-bytes").unwrap();
        let imported = root
            .import_asset(
                V561AssetKind::Portrait,
                &source,
                V561ImportOptions {
                    name: Some("小雅"),
                    license_note: Some("user owns it"),
                },
            )
            .expect("import");
        assert!(imported.path.is_file());
        assert!(imported.hash.len() == 64);
        let conn = root.open_db().unwrap();
        let (name, hash, note): (String, String, String) = conn
            .query_row(
                "SELECT name, source_hash, license_note FROM characters WHERE id=?1",
                [&imported.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "小雅");
        assert_eq!(hash, imported.hash);
        assert_eq!(note, "user owns it");
    }

    #[test]
    fn rejects_oversized_or_symlink_assets() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("root");
        let big = dir.path().join("big.bin");
        std::fs::write(&big, vec![0u8; 11 * 1024 * 1024]).unwrap();
        assert!(
            root.import_asset(V561AssetKind::Knowledge, &big, V561ImportOptions::default())
                .is_err()
        );
        #[cfg(unix)]
        {
            let link = dir.path().join("link.png");
            std::os::unix::fs::symlink(&big, &link).unwrap();
            assert!(
                root.import_asset(V561AssetKind::Portrait, &link, V561ImportOptions::default())
                    .is_err()
            );
        }
    }

    #[test]
    fn rejects_absolute_and_parent_escape() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("open root");
        assert!(root.resolve("../outside").is_err());
        assert!(root.resolve("/etc/passwd").is_err());
        assert!(
            root.resolve(Path::new("assets").join("..").join(".."))
                .is_err()
        );
        assert!(root.resolve("assets/characters/a.png").is_ok());
    }

    #[test]
    fn rejects_symlink_escape_when_parent_exists() {
        let dir = temp_root();
        let root = V561DataRoot::open(dir.path()).expect("open root");
        let outside = temp_root();
        let link = root.root().join("assets").join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(outside.path(), &link).unwrap();
        assert!(root.resolve("assets/escape/file.bin").is_err());
    }
}

//! The upgrade-package manifest as a typed value.
//!
//! Parsing is JSON Schema first and extraction second, the same order the model
//! admission check uses: the schema is the single source of structural truth, so
//! the extraction below may assume the shapes it guarantees and never has to
//! invent a default for a missing field.
use crate::{Result, error};
use nai_contracts::v1::ErrorCode;
use serde_json::Value;

/// A manifest larger than this is refused before it is parsed.
pub const MANIFEST_LIMIT: usize = 65_536;
const SCHEMA: &str = include_str!("../../../schemas/config/upgrade-package.schema.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeKind {
    Plugin,
    Kernel,
    Model,
}
impl UpgradeKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "plugin" => Some(Self::Plugin),
            "kernel" => Some(Self::Kernel),
            "model" => Some(Self::Model),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Kernel => "kernel",
            Self::Model => "model",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub reference: String,
    pub range: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEntry {
    pub id: String,
    pub accel: String,
    pub min_bytes: Option<u64>,
}

/// One declared artifact. `sha256` is lowercase hex; `path` is relative and
/// canonical (no empty, `.` or `..` segment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDecl {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Per-component cost. `None` means "cannot estimate", which is deliberately
/// distinct from `Some(0)` ("free"): a report has to be able to say "unknown"
/// without asserting a number it cannot support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostSpec {
    pub currency: String,
    pub cloud: Option<u64>,
    pub download: Option<u64>,
    pub storage: Option<u64>,
    pub runtime_delta: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct UpgradePackage {
    pub package_id: String,
    pub package_version: String,
    pub kind: UpgradeKind,
    pub kernel_range: String,
    pub requires: Vec<Requirement>,
    pub hosts: Vec<String>,
    pub os: Vec<String>,
    pub architectures: Vec<String>,
    pub runtime: Vec<RuntimeEntry>,
    pub files: Vec<FileDecl>,
    pub install_bytes: u64,
    pub migrated_bytes: u64,
    pub signer_ref: String,
    pub license_ref: String,
    pub migration_ref: String,
    pub rollback_range: String,
    pub cost: CostSpec,
}

fn canonical_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 256
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn text(value: &Value, key: &str) -> Result<String> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| error(ErrorCode::Internal, "manifest field missing after schema"))
}

fn amount(value: &Value, key: &str) -> Result<u64> {
    text(value, key)?
        .parse::<u64>()
        .map_err(|_| error(ErrorCode::Internal, "manifest amount unparsable"))
}

fn optional_amount(value: &Value) -> Result<Option<u64>> {
    match value.as_str() {
        None => Ok(None),
        Some(text) => text
            .parse::<u64>()
            .map(Some)
            .map_err(|_| error(ErrorCode::Internal, "manifest amount unparsable")),
    }
}

fn strings(value: &Value, key: &str) -> Result<Vec<String>> {
    value[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .ok_or_else(|| error(ErrorCode::Internal, "manifest list missing after schema"))
}

impl UpgradePackage {
    /// Parse and structurally validate a manifest. Rejects anything that fails
    /// the embedded schema, has an unparsable version, or declares a file path
    /// that is not canonical or is duplicated.
    pub fn parse(manifest: &[u8]) -> Result<Self> {
        if manifest.is_empty() || manifest.len() > MANIFEST_LIMIT {
            return Err(error(ErrorCode::ResourceExhausted, "manifest size limit"));
        }
        let value: Value = serde_json::from_slice(manifest)
            .map_err(|_| error(ErrorCode::InvalidArgument, "invalid manifest JSON"))?;
        let schema: Value = serde_json::from_str(SCHEMA)
            .map_err(|_| error(ErrorCode::Internal, "embedded upgrade schema invalid"))?;
        if !jsonschema::validator_for(&schema)
            .map_err(|_| error(ErrorCode::Internal, "upgrade validator unavailable"))?
            .is_valid(&value)
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "manifest violates schema",
            ));
        }

        let package_version = text(&value, "package_version")?;
        semver::Version::parse(&package_version)
            .map_err(|_| error(ErrorCode::InvalidArgument, "unparsable package version"))?;

        let requires = value["requires"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| Requirement {
                        reference: item["ref"].as_str().unwrap_or_default().to_owned(),
                        range: item["range"].as_str().unwrap_or_default().to_owned(),
                    })
                    .collect()
            })
            .ok_or_else(|| error(ErrorCode::Internal, "manifest requires missing"))?;

        let runtime = value["runtime"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| RuntimeEntry {
                        id: item["id"].as_str().unwrap_or_default().to_owned(),
                        accel: item["accel"].as_str().unwrap_or_default().to_owned(),
                        min_bytes: item.get("min_bytes").and_then(|amount| {
                            amount.as_str().and_then(|text| text.parse::<u64>().ok())
                        }),
                    })
                    .collect()
            })
            .ok_or_else(|| error(ErrorCode::Internal, "manifest runtime missing"))?;

        let mut seen = std::collections::BTreeSet::new();
        let files = value["files"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        let path = item["path"].as_str().unwrap_or_default().to_owned();
                        Ok(FileDecl {
                            sha256: item["sha256"].as_str().unwrap_or_default().to_owned(),
                            bytes: item["bytes"]
                                .as_str()
                                .and_then(|text| text.parse::<u64>().ok())
                                .ok_or_else(|| {
                                    error(ErrorCode::Internal, "manifest file size unparsable")
                                })?,
                            path,
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .ok_or_else(|| error(ErrorCode::Internal, "manifest files missing"))??;
        for file in &files {
            if !canonical_path(&file.path) || !seen.insert(file.path.clone()) {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "ambiguous or non-canonical file path",
                ));
            }
        }

        let cost = &value["cost"];
        Ok(Self {
            package_id: text(&value, "package_id")?,
            package_version,
            kind: UpgradeKind::parse(&text(&value, "kind")?)
                .ok_or_else(|| error(ErrorCode::Internal, "manifest kind missing after schema"))?,
            kernel_range: text(&value, "kernel_range")?,
            requires,
            hosts: strings(&value, "hosts")?,
            os: strings(&value, "os")?,
            architectures: strings(&value, "architectures")?,
            runtime,
            files,
            install_bytes: amount(&value, "install_bytes")?,
            migrated_bytes: amount(&value, "migrated_bytes")?,
            signer_ref: text(&value, "signer_ref")?,
            license_ref: text(&value, "license_ref")?,
            migration_ref: text(&value, "migration_ref")?,
            rollback_range: text(&value, "rollback_range")?,
            cost: CostSpec {
                currency: text(cost, "currency")?,
                cloud: optional_amount(&cost["cloud"])?,
                download: optional_amount(&cost["download"])?,
                storage: optional_amount(&cost["storage"])?,
                runtime_delta: optional_amount(&cost["runtime_delta"])?,
            },
        })
    }
}

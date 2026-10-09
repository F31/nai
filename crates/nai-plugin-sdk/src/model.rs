//! Checks caller-owned immutable model content, not filesystem loading, ONNX
//! validity, licensing, readiness or G0–G3 certification.
use crate::{digest, error};
use nai_contracts::v1::{ContractError, ErrorCode};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

pub struct ModelFile<'a> {
    pub path: &'a str,
    pub content: &'a [u8],
}

pub struct ModelLimits {
    pub max_files: usize,
    pub max_bytes: u64,
}

/// Content identity only. No execution permission or certification is implied.
#[derive(Debug)]
pub struct ModelIntegrity {
    pub manifest_hash: String,
    pub file_count: usize,
    pub content_bytes: u64,
}

fn checkpoint(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ContractError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(error(ErrorCode::Cancelled, "model check cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(error(ErrorCode::DeadlineExceeded, "model check deadline"));
    }
    Ok(())
}

fn invalid(message: &str) -> ContractError {
    error(ErrorCode::InvalidArgument, message)
}

fn canonical_path(path: &str) -> bool {
    !path.is_empty()
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b))
}

/// `runtime` must equal one complete runtime_matrix object from the manifest.
/// Callers must load/execute the same verified bytes and enforce their own file
/// access and memory budgets before providing these slices.
pub fn check_model_integrity(
    manifest: &[u8],
    files: &[ModelFile<'_>],
    runtime: &Value,
    limits: ModelLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ModelIntegrity, ContractError> {
    checkpoint(deadline, cancelled)?;
    if manifest.len() > 65536 || files.len() > limits.max_files {
        return Err(error(ErrorCode::ResourceExhausted, "model input limit"));
    }
    let value: Value =
        serde_json::from_slice(manifest).map_err(|_| invalid("invalid model JSON"))?;
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/plugin/model.schema.json"))
            .map_err(|_| error(ErrorCode::Internal, "model schema invalid"))?;
    if !jsonschema::validator_for(&schema)
        .map_err(|_| error(ErrorCode::Internal, "model validator unavailable"))?
        .is_valid(&value)
    {
        return Err(invalid("model schema rejected"));
    }
    checkpoint(deadline, cancelled)?;
    if !value["runtime_matrix"]
        .as_array()
        .unwrap()
        .contains(runtime)
    {
        return Err(error(
            ErrorCode::VersionIncompatible,
            "model runtime mismatch",
        ));
    }
    let mut graph_ids = BTreeSet::new();
    let mut declared = BTreeMap::new();
    for graph in value["graphs"].as_array().unwrap() {
        checkpoint(deadline, cancelled)?;
        if !graph_ids.insert(graph["id"].as_str().unwrap()) {
            return Err(invalid("duplicate model graph"));
        }
        for file in
            std::iter::once(&graph["file"]).chain(graph["external_weights"].as_array().unwrap())
        {
            let path = file["path"].as_str().unwrap();
            if !canonical_path(path)
                || declared
                    .insert(path, file["sha256"].as_str().unwrap())
                    .is_some()
            {
                return Err(invalid("ambiguous model file path"));
            }
            if declared.len() > limits.max_files {
                return Err(error(
                    ErrorCode::ResourceExhausted,
                    "model file count limit",
                ));
            }
        }
    }
    if value["required_graph_ids"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| !graph_ids.contains(id.as_str().unwrap()))
    {
        return Err(invalid("missing required model graph"));
    }
    let mut provided = BTreeMap::new();
    let mut content_bytes = 0u64;
    for file in files {
        checkpoint(deadline, cancelled)?;
        content_bytes = content_bytes
            .checked_add(file.content.len() as u64)
            .filter(|total| *total <= limits.max_bytes)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "model content byte limit"))?;
        if !declared.contains_key(file.path) || provided.insert(file.path, file.content).is_some() {
            return Err(invalid("unexpected or duplicate model content"));
        }
    }
    if provided.len() != declared.len() {
        return Err(invalid("missing model content"));
    }
    for (path, expected) in declared {
        let mut hasher = Sha256::new();
        for chunk in provided[path].chunks(65536) {
            checkpoint(deadline, cancelled)?;
            hasher.update(chunk);
        }
        let actual: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if actual != expected {
            return Err(invalid("model content hash mismatch"));
        }
    }
    let manifest_hash = digest(manifest);
    checkpoint(deadline, cancelled)?;
    Ok(ModelIntegrity {
        manifest_hash,
        file_count: provided.len(),
        content_bytes,
    })
}

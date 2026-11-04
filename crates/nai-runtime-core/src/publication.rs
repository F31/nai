//! Pure publication policy. No storage, UI, provider or output authority.
use crate::foundation::{Operation, Result, manual_guard};
use nai_contracts::workbench::WorkbenchError as Error;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Mutex, OnceLock},
};
#[cfg(test)]
mod tests;

static ARTIFACT: OnceLock<std::result::Result<jsonschema::Validator, Error>> = OnceLock::new();
// Only immutable schema validity is memoized, never source currency or authority.
// Store hashes, not configuration bodies. Contention simply falls back to validation.
static VALID_ARTIFACTS: Mutex<VecDeque<[u8; 32]>> = Mutex::new(VecDeque::new());
pub fn artifact(text: &str) -> Result<Value> {
    manual_guard(text)?;
    let value: Value = serde_json::from_str(text).map_err(|_| Error::IntegrityFailed)?;
    let digest: [u8; 32] = Sha256::digest(text.as_bytes()).into();
    if VALID_ARTIFACTS
        .try_lock()
        .is_ok_and(|c| c.contains(&digest))
    {
        return Ok(value);
    }
    let validator = ARTIFACT
        .get_or_init(|| {
            let source = serde_json::from_str(include_str!(
                "../../../schemas/content/publication.schema.json"
            ))
            .map_err(|_| Error::IntegrityFailed)?;
            jsonschema::validator_for(&source).map_err(|_| Error::IntegrityFailed)
        })
        .as_ref()
        .map_err(|e| *e)?;
    if !validator.is_valid(&value) {
        return Err(Error::IntegrityFailed);
    }
    if let Ok(mut cache) = VALID_ARTIFACTS.try_lock()
        && !cache.contains(&digest)
    {
        if cache.len() == 64 {
            cache.pop_front();
        }
        cache.push_back(digest);
    }
    Ok(value)
}
pub struct CheckedClosure {
    pub dependencies: Value,
    pub valid_until_ms: Option<i64>,
}
/// All facts are owner-confirmed and currently valid in this initial restricted profile.
pub fn closure(op: &Operation, documents: [&Value; 4], now: i64) -> Result<CheckedClosure> {
    for (doc, expected) in
        documents
            .iter()
            .zip(["show_draft", "product_draft", "host_draft", "series_draft"])
    {
        if crate::preparation::validate(op, &doc.to_string())? != expected {
            return Err(Error::InvalidArgument);
        }
    }
    let [show, product, _, _] = documents;
    let facts = product["facts"].as_array().ok_or(Error::InvalidArgument)?;
    let cues = show["cues"].as_array().ok_or(Error::InvalidArgument)?;
    if facts.is_empty() || cues.is_empty() {
        return Err(Error::CapabilityUnavailable);
    }
    let mut until = None;
    for fact in facts {
        if fact["authority"] != "owner_confirmed"
            || fact["valid_from_ms"].as_i64().is_some_and(|t| t > now)
            || fact["valid_until_ms"].as_i64().is_some_and(|t| t <= now)
        {
            return Err(Error::CapabilityUnavailable);
        }
        if let Some(end) = fact["valid_until_ms"].as_i64() {
            until = Some(until.map_or(end, |old: i64| old.min(end)));
        }
    }
    let tracks = product["talk_tracks"]
        .as_array()
        .ok_or(Error::InvalidArgument)?;
    let mut dependencies = vec![];
    for cue in cues {
        let track = tracks
            .iter()
            .find(|t| t["id"] == cue["talk_track_id"])
            .ok_or(Error::InvalidArgument)?;
        let ids = track["fact_ids"].as_array().ok_or(Error::InvalidArgument)?;
        if ids.is_empty() {
            return Err(Error::CapabilityUnavailable);
        }
        for required in cue["mandatory_fact_ids"]
            .as_array()
            .ok_or(Error::InvalidArgument)?
        {
            if !ids.contains(required) {
                return Err(Error::InvalidArgument);
            }
        }
        dependencies.push(json!({"cue_id":cue["id"],"fact_ids":ids}));
    }
    op.check()?;
    Ok(CheckedClosure {
        dependencies: Value::Array(dependencies),
        valid_until_ms: until,
    })
}
/// Arrays of named facts/cues are compared by stable IDs, rather than display order.
pub fn diff(
    before: &Value,
    after: &Value,
    path: &str,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    if before == after {
        return Ok(());
    }
    if output.len() >= 128 {
        return Err(Error::ResourceExhausted);
    }
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                diff(
                    a.get(key).unwrap_or(&Value::Null),
                    b.get(key).unwrap_or(&Value::Null),
                    &format!("{path}/{key}"),
                    output,
                )?;
            }
        }
        (Value::Array(a), Value::Array(b)) if a.iter().chain(b).all(|x| x["id"].is_string()) => {
            let ids = a
                .iter()
                .chain(b)
                .filter_map(|x| x["id"].as_str())
                .collect::<BTreeSet<_>>();
            for id in ids {
                diff(
                    a.iter().find(|x| x["id"] == id).unwrap_or(&Value::Null),
                    b.iter().find(|x| x["id"] == id).unwrap_or(&Value::Null),
                    &format!("{path}/{id}"),
                    output,
                )?;
            }
        }
        _ => {
            output.insert(path.to_owned());
        }
    }
    if output.len() > 128 {
        return Err(Error::ResourceExhausted);
    }
    Ok(())
}

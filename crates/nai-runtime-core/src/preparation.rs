//! RF03 draft policy. Saving is never publication, fact verification or delivery authority.
use crate::foundation::{Operation, Result, manual_guard};
use nai_contracts::workbench::WorkbenchError as Error;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Mutex, OnceLock},
};

const SOURCE: &str = include_str!("../../../schemas/content/preparation.schema.json");
static VALIDATOR: OnceLock<std::result::Result<jsonschema::Validator, Error>> = OnceLock::new();
// Pure schema shape only; closure() still checks authority and validity at current time.
static VALID_SHAPES: Mutex<VecDeque<[u8; 32]>> = Mutex::new(VecDeque::new());

/// Reserved envelopes cannot be smuggled through the manual text command.
pub fn kind(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    let kind = value.get("kind")?.as_str()?;
    (kind.ends_with("_draft") || kind.starts_with("publication_")).then(|| kind.to_owned())
}

pub fn validate(op: &Operation, text: &str) -> Result<String> {
    op.check()?;
    manual_guard(text)?;
    let value: Value = serde_json::from_str(text).map_err(|_| Error::InvalidArgument)?;
    let digest: [u8; 32] = Sha256::digest(text.as_bytes()).into();
    let known = VALID_SHAPES.try_lock().is_ok_and(|c| c.contains(&digest));
    let validator = VALIDATOR
        .get_or_init(|| {
            let schema: Value = serde_json::from_str(SOURCE).map_err(|_| Error::IntegrityFailed)?;
            jsonschema::validator_for(&schema).map_err(|_| Error::IntegrityFailed)
        })
        .as_ref()
        .map_err(|e| *e)?;
    if !known && !validator.is_valid(&value) {
        return Err(Error::InvalidArgument);
    }
    if let Ok(mut cache) = VALID_SHAPES.try_lock()
        && !cache.contains(&digest)
    {
        if cache.len() == 64 {
            cache.pop_front();
        }
        cache.push_back(digest);
    }
    strings(&value)?;
    let kind = value["kind"].as_str().ok_or(Error::InvalidArgument)?;
    if kind == "product_draft" {
        let facts = unique(&value["facts"])?;
        for fact in array(&value["facts"])? {
            if let (Some(start), Some(end)) = (
                fact["valid_from_ms"].as_i64(),
                fact["valid_until_ms"].as_i64(),
            ) && start >= end
            {
                return Err(Error::InvalidArgument);
            }
            if fact["value_type"] != "money" && fact.get("currency").is_some() {
                return Err(Error::InvalidArgument);
            }
        }
        for field in ["faq", "talk_tracks"] {
            unique(&value[field])?;
            for binding in array(&value[field])? {
                for id in array(&binding["fact_ids"])? {
                    if !facts.contains(id.as_str().ok_or(Error::InvalidArgument)?) {
                        return Err(Error::InvalidArgument);
                    }
                }
            }
        }
    } else if kind == "show_draft" {
        unique(&value["cues"])?;
    } else if kind == "manual_run_draft" {
        let cues = array(&value["cue_ids"])?;
        if value["cursor"].as_u64().ok_or(Error::InvalidArgument)? >= cues.len() as u64 {
            return Err(Error::InvalidArgument);
        }
        unique(&value["marks"])?;
        if array(&value["marks"])?
            .iter()
            .any(|mark| !cues.contains(&mark["cue_id"]))
        {
            return Err(Error::InvalidArgument);
        }
    }
    op.check()?;
    Ok(kind.into())
}
fn array(value: &Value) -> Result<&Vec<Value>> {
    value.as_array().ok_or(Error::InvalidArgument)
}
fn unique(value: &Value) -> Result<BTreeSet<&str>> {
    let mut ids = BTreeSet::new();
    for entry in array(value)? {
        if !ids.insert(entry["id"].as_str().ok_or(Error::InvalidArgument)?) {
            return Err(Error::InvalidArgument);
        }
    }
    Ok(ids)
}
fn strings(value: &Value) -> Result<()> {
    match value {
        Value::String(s) if s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') => {
            Err(Error::InvalidArgument)
        }
        Value::Array(a) => a.iter().try_for_each(strings),
        Value::Object(o) => o.values().try_for_each(strings),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PRODUCT: &str = include_str!("../../../tests/fixtures/contracts/preparation.json");
    #[test]
    fn source_schema_and_local_bindings_are_enforced() {
        let op = Operation::new(4000).unwrap();
        assert_eq!(validate(&op, PRODUCT).unwrap(), "product_draft");
        let original: Value = serde_json::from_str(PRODUCT).unwrap();
        let mut duplicate = original.clone();
        duplicate["facts"]
            .as_array_mut()
            .unwrap()
            .push(original["facts"][0].clone());
        let mut dangling = original.clone();
        dangling["faq"][0]["fact_ids"][0] = "missing".into();
        let mut validity = original.clone();
        validity["facts"][0]["valid_from_ms"] = 2.into();
        validity["facts"][0]["valid_until_ms"] = 1.into();
        let mut control = original.clone();
        control["facts"][0]["source"] = "bad\0source".into();
        let mut unknown = original.clone();
        unknown["approved"] = true.into();
        for bad in [duplicate, dangling, validity, control, unknown] {
            assert_eq!(validate(&op, &bad.to_string()), Err(Error::InvalidArgument));
        }
        op.cancel();
        assert_eq!(validate(&op, PRODUCT), Err(Error::Cancelled));
    }
    #[test]
    fn preferences_and_incomplete_drafts_do_not_assert_verification() {
        let op = Operation::new(4000).unwrap();
        for text in [
            r#"{"schema_version":1,"kind":"host_draft","name":"主持","style":"concise","preferences":""}"#,
            r#"{"schema_version":1,"kind":"series_draft","name":"系列","goal":"介绍产品"}"#,
            r#"{"schema_version":1,"kind":"product_draft","name":"未完成","facts":[],"faq":[],"talk_tracks":[],"prohibitions":[]}"#,
        ] {
            assert!(validate(&op, text).is_ok());
        }
        assert!(kind(r#"{"kind":"future_draft"}"#).is_some());
        assert!(kind("人工文本").is_none());
        assert_eq!(
            validate(&op, &"x".repeat(8193)),
            Err(Error::InvalidArgument)
        );
    }
    #[test]
    fn admitted_avatar_profile_is_bounded_metadata_only() {
        let op = Operation::new(4000).unwrap();
        let valid = r#"{"schema_version":1,"kind":"avatar_profile_draft","name":"演示形象","avatar_id":"demo","package_version":"1","package_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","license_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","width":2,"height":1,"selected":true,"revoked":false}"#;
        assert_eq!(validate(&op, valid).unwrap(), "avatar_profile_draft");
        let mut raw: Value = serde_json::from_str(valid).unwrap();
        raw["asset_path"] = "C:\\secret\\avatar.rgba".into();
        assert_eq!(validate(&op, &raw.to_string()), Err(Error::InvalidArgument));
        raw.as_object_mut().unwrap().remove("asset_path");
        raw["package_sha256"] = "short".into();
        assert_eq!(validate(&op, &raw.to_string()), Err(Error::InvalidArgument));
    }
    #[test]
    fn admitted_avatar_video_is_bounded_metadata_with_model_missing() {
        let op = Operation::new(4000).unwrap();
        let valid = r#"{"schema_version":1,"kind":"avatar_video_draft","name":"演示视频","video_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","duration_ms":5000,"inspect_version":"mp4-ftyp-mvhd-v1","realistic_drive":"model_missing","revoked":false}"#;
        assert_eq!(validate(&op, valid).unwrap(), "avatar_video_draft");
        let mut raw: Value = serde_json::from_str(valid).unwrap();
        raw["asset_path"] = "C:\\secret\\clip.mp4".into();
        assert_eq!(validate(&op, &raw.to_string()), Err(Error::InvalidArgument));
        raw.as_object_mut().unwrap().remove("asset_path");
        raw["realistic_drive"] = "ready".into();
        assert_eq!(validate(&op, &raw.to_string()), Err(Error::InvalidArgument));
        raw["realistic_drive"] = "model_missing".into();
        raw["duration_ms"] = 500.into();
        assert_eq!(validate(&op, &raw.to_string()), Err(Error::InvalidArgument));
    }
}

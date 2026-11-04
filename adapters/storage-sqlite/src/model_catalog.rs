//! Model configuration only. TTS execution belongs to the later media/provider milestone.
use super::*;
use crate::llm;
use serde_json::{Value, json};
fn tts_view(id: String, v: Value, revision: u64) -> Result<TtsModelView> {
    Ok(TtsModelView {
        id,
        revision,
        name: v["name"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        base_url: v["base_url"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        model: v["model"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        voice: v["voice"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        enabled: v["enabled"].as_bool().ok_or(Error::IntegrityFailed)?,
        credential_saved: !v["secret"].is_null(),
        protocol: nai_runtime_core::tts::protocol(v["protocol"].as_str().unwrap_or(""))?.into(),
        language: v["language"].as_str().unwrap_or("").into(),
        price_known: v["price_known"].as_bool().ok_or(Error::IntegrityFailed)?,
        character_minor_per_million: v["character_minor_per_million"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?,
    })
}
pub(super) fn tts_views(conn: &Connection, root: &Path, ws: &str) -> Result<Vec<TtsModelView>> {
    llm::models_kind(conn, root, ws, "publication_tts_config")?
        .into_iter()
        .map(|(id, v, rev)| tts_view(id, v, rev))
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn save_tts(
    tx: &Transaction<'_>,
    root: &Path,
    op: &Operation,
    vault: Option<&dyn nai_runtime_core::llm::CredentialVault>,
    actor: &str,
    now: i64,
    x: &FoundationTtsModel,
) -> Result<FoundationResult> {
    uuid(&x.model_id)?;
    op.check()?;
    let protocol = nai_runtime_core::tts::protocol(&x.protocol)?;
    let local = nai_runtime_core::tts::validate_profile(protocol, &x.base_url, &x.language)?;
    if [&x.name, &x.model, &x.voice]
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        || x.base_url.is_empty()
        || x.base_url.len() > 2048
        || x.base_url.chars().any(char::is_control)
        || x.character_minor_per_million > 1_000_000_000
        || (!x.price_known && x.character_minor_per_million != 0)
    {
        return Err(Error::InvalidArgument);
    }
    let old = if x.expected_revision > 0 {
        let (v, rev, _) = llm::raw(
            tx,
            root,
            &x.workspace_id,
            &x.model_id,
            0,
            "publication_tts_config",
        )?;
        if rev != x.expected_revision {
            return Err(Error::RevisionConflict);
        }
        if v["removed"] == true {
            return Err(Error::NotFound);
        }
        Some(v)
    } else {
        if tts_views(tx, root, &x.workspace_id)?.len() >= 8 {
            return Err(Error::ResourceExhausted);
        }
        None
    };
    let secret = if local {
        if !x.api_key.is_empty() {
            return Err(Error::InvalidArgument);
        }
        Value::Null
    } else if x.api_key.is_empty() {
        if let Some(v) = &old {
            if v["base_url"] != x.base_url && !v["secret"].is_null() {
                return Err(Error::InvalidArgument);
            }
            v["secret"].clone()
        } else {
            Value::Null
        }
    } else {
        vault.ok_or(Error::CapabilityUnavailable)?.seal(
            if crate::shared_catalog::enabled(tx)? {
                crate::shared_catalog::SCOPE
            } else {
                &x.workspace_id
            },
            &x.model_id,
            &x.base_url,
            &x.api_key,
        )?
    };
    let v = json!({"schema_version":1,"kind":"publication_tts_config","created_at_ms":now,"created_by":actor,"name":x.name.trim(),"base_url":x.base_url,"model":x.model.trim(),"voice":x.voice.trim(),"protocol":protocol,"language":x.language,"enabled":x.enabled,"price_known":x.price_known,"character_minor_per_million":x.character_minor_per_million,"secret":secret,"removed":false});
    let (revision, sha) = llm::put(
        tx,
        root,
        &x.workspace_id,
        &x.model_id,
        x.expected_revision,
        &v,
        now,
    )?;
    Ok(FoundationResult {
        resource_id: x.model_id.clone(),
        revision,
        sha256: sha,
        tts_models: vec![tts_view(x.model_id.clone(), v, revision)?],
        ..Default::default()
    })
}
pub(super) fn remove(
    tx: &Transaction<'_>,
    root: &Path,
    op: &Operation,
    now: i64,
    x: &FoundationDelete,
    kind: &str,
) -> Result<FoundationResult> {
    op.check()?;
    let (mut v, revision, _) = llm::raw(tx, root, &x.workspace_id, &x.object_id, 0, kind)?;
    if revision != x.expected_revision {
        return Err(Error::RevisionConflict);
    }
    if v["removed"] == true {
        return Err(Error::NotFound);
    }
    if llm::active(tx)? {
        return Err(Error::Busy);
    }
    v["removed"] = true.into();
    v["enabled"] = false.into();
    let (revision, sha) = llm::put(
        tx,
        root,
        &x.workspace_id,
        &x.object_id,
        x.expected_revision,
        &v,
        now,
    )?;
    let mut result = llm::settings(tx, root, &x.workspace_id)?;
    result.resource_id = x.object_id.clone();
    result.revision = revision;
    result.sha256 = sha;
    Ok(result)
}
impl Store {
    pub fn attach_tts_vault(&mut self, vault: Arc<dyn nai_runtime_core::llm::CredentialVault>) {
        self.tts_vault = Some(vault);
    }
}

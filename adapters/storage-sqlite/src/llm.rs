//! Private model configuration, task journal and money, owned by the sole writer.
use super::*;
use nai_runtime_core::llm as policy;
use serde_json::{Value, json};
pub(super) fn raw(
    conn: &Connection,
    root: &Path,
    ws: &str,
    id: &str,
    revision: u64,
    kind: &str,
) -> Result<(Value, u64, String)> {
    if crate::shared_catalog::model_kind(kind)
        && let Some(model) = crate::shared_catalog::raw(conn, id, revision, kind)?
    {
        return Ok(model);
    }
    let (latest, sha) = object(conn, ws, id, revision)?;
    let v = nai_runtime_core::publication::artifact(&body(root, &sha)?)?;
    if v["kind"] != kind {
        return Err(Error::InvalidArgument);
    }
    Ok((v, if revision == 0 { latest } else { revision }, sha))
}
pub(super) fn put(
    tx: &Transaction<'_>,
    root: &Path,
    ws: &str,
    id: &str,
    expected: u64,
    v: &Value,
    now: i64,
) -> Result<(u64, String)> {
    uuid(id)?;
    if expected >= 1024 {
        return Err(Error::ResourceExhausted);
    }
    if crate::shared_catalog::model_kind(v["kind"].as_str().unwrap_or(""))
        && crate::shared_catalog::enabled(tx)?
    {
        return crate::shared_catalog::put(tx, id, expected, v);
    }
    let current: Option<u64> = tx
        .query_row(
            "SELECT revision FROM objects WHERE workspace_id=?1 AND id=?2 AND deleted=0",
            params![ws, id],
            |r| uint(r, 0),
        )
        .optional()
        .wb()?;
    if current.unwrap_or(0) != expected {
        return Err(Error::RevisionConflict);
    }
    let text = v.to_string();
    if text.len() > 8192 {
        return Err(Error::ResourceExhausted);
    }
    nai_runtime_core::publication::artifact(&text)?;
    let sha = store_body(root, text.as_bytes())?;
    tx.execute("INSERT INTO objects VALUES(?1,?2,1,0,?3,?3) ON CONFLICT(workspace_id,id) DO UPDATE SET revision=revision+1,updated_at=excluded.updated_at",params![ws,id,now]).wb()?;
    let rev = expected + 1;
    tx.execute(
        "INSERT INTO object_versions VALUES(?1,?2,?3,?4,?5,?5)",
        params![ws, id, int(rev)?, sha, now],
    )
    .wb()?;
    Ok((rev, sha))
}
fn models(conn: &Connection, root: &Path, ws: &str) -> Result<Vec<(String, Value, u64)>> {
    models_kind(conn, root, ws, "publication_llm_config")
}
pub(super) fn models_kind(
    conn: &Connection,
    root: &Path,
    ws: &str,
    kind: &str,
) -> Result<Vec<(String, Value, u64)>> {
    if crate::shared_catalog::enabled(conn)? {
        return crate::shared_catalog::models(conn, kind);
    }
    let mut q = conn
        .prepare("SELECT id FROM objects WHERE workspace_id=?1 AND deleted=0 ORDER BY id LIMIT 257")
        .wb()?;
    let ids = q
        .query_map([ws], |r| r.get::<_, String>(0))
        .wb()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .wb()?;
    if ids.len() > 256 {
        return Err(Error::ResourceExhausted);
    }
    let mut out = vec![];
    for id in ids {
        let (rev, sha) = object(conn, ws, &id, 0)?;
        let v: Value = serde_json::from_str(&body(root, &sha)?).unwrap_or(Value::Null);
        if v["kind"] == kind && v["removed"] != true {
            nai_runtime_core::publication::artifact(&v.to_string())?;
            out.push((id, v, rev));
        }
    }
    if out.len() > 8 {
        return Err(Error::ResourceExhausted);
    }
    Ok(out)
}
pub(super) fn budget(conn: &Connection, ws: &str) -> Result<LlmBudgetView> {
    let (rev, limit) = conn
        .query_row(
            "SELECT revision,budget_limit FROM workspaces WHERE id=?1",
            [ws],
            |r| Ok((uint(r, 0)?, uint(r, 1)?)),
        )
        .wb()?;
    let invalid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM budget_reservations WHERE workspace_id=?1 AND status='known' AND actual_minor IS NULL)",[ws],|r|r.get(0)).wb()?;
    if invalid {
        return Err(Error::IntegrityFailed);
    }
    let(used,unknown,reserved)=conn.query_row("SELECT COALESCE(SUM(CASE WHEN status='released' THEN 0 WHEN status='known' THEN actual_minor ELSE reserved_minor END),0),COALESCE(SUM(CASE WHEN status='unknown' THEN reserved_minor ELSE 0 END),0),COALESCE(SUM(CASE WHEN status='reserved' THEN reserved_minor ELSE 0 END),0) FROM budget_reservations WHERE workspace_id=?1",[ws],|r|Ok((uint(r,0)?,uint(r,1)?,uint(r,2)?))).wb()?;
    Ok(LlmBudgetView {
        mode: super::workspace_settings::mode(conn, ws, limit)?,
        revision: rev,
        limit_minor: limit,
        used_minor: used,
        unknown_minor: unknown,
        reserved_minor: reserved,
        currency: "CNY".into(),
    })
}
/// The ledger's used_minor already includes unknown and reserved amounts.
/// Keep existing roots disabled/limited until explicit mode migration is active.
pub(super) fn admit_budget(b: &LlmBudgetView, next: u64, cap: u64) -> Result<()> {
    let mode = WorkspaceBudgetMode::try_from(b.mode).map_err(|_| Error::IntegrityFailed)?;
    nai_runtime_core::workspace_budget::admit(
        mode as i32,
        b.limit_minor,
        b.used_minor,
        0,
        0,
        next,
        cap,
    )
}
fn view(id: String, v: Value, revision: u64) -> Result<LlmModelView> {
    Ok(LlmModelView {
        id,
        revision,
        name: v["name"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        base_url: v["base_url"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        model: v["model"].as_str().ok_or(Error::IntegrityFailed)?.into(),
        input_minor_per_million: v["input_minor_per_million"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?,
        output_minor_per_million: v["output_minor_per_million"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?,
        enabled: v["enabled"].as_bool().ok_or(Error::IntegrityFailed)?,
        credential_saved: !v["secret"].is_null(),
        pricing_unknown: v["pricing_unknown"] == true,
    })
}
pub(super) fn settings(conn: &Connection, root: &Path, ws: &str) -> Result<FoundationResult> {
    let list = models(conn, root, ws)?
        .into_iter()
        .map(|(id, v, rev)| view(id, v, rev))
        .collect::<Result<Vec<_>>>()?;
    Ok(FoundationResult {
        llm_models: list,
        tts_models: crate::model_catalog::tts_views(conn, root, ws)?,
        llm_budget: Some(budget(conn, ws)?),
        shared_catalog_enabled: crate::shared_catalog::enabled(conn)?,
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn save(
    tx: &Transaction<'_>,
    root: &Path,
    op: &Operation,
    vault: Option<&dyn policy::CredentialVault>,
    actor: &str,
    now: i64,
    x: &FoundationLlmModel,
) -> Result<FoundationResult> {
    uuid(&x.model_id)?;
    op.check()?;
    if x.name.trim().is_empty()
        || x.name.len() > 128
        || x.model.trim().is_empty()
        || x.model.len() > 128
        || x.base_url.len() > 2048
        || x.base_url.is_empty()
        || x.input_minor_per_million > 1_000_000_000
        || x.output_minor_per_million > 1_000_000_000
        || (!x.prices_confirmed
            && (x.input_minor_per_million != 0 || x.output_minor_per_million != 0))
        || [x.name.as_str(), &x.model, &x.base_url]
            .iter()
            .any(|s| s.chars().any(char::is_control))
    {
        return Err(Error::InvalidArgument);
    }
    let old = if x.expected_revision > 0 {
        let (v, rev, _) = raw(
            tx,
            root,
            &x.workspace_id,
            &x.model_id,
            0,
            "publication_llm_config",
        )?;
        if rev != x.expected_revision {
            return Err(Error::RevisionConflict);
        }
        if v["removed"] == true {
            return Err(Error::NotFound);
        }
        Some(v)
    } else {
        if models(tx, root, &x.workspace_id)?.len() >= 8 {
            return Err(Error::ResourceExhausted);
        }
        None
    };
    let secret = if x.api_key.is_empty() {
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
    let v = json!({"schema_version":1,"kind":"publication_llm_config","created_at_ms":now,"created_by":actor,"name":x.name.trim(),"base_url":x.base_url,"model":x.model.trim(),"pricing_unknown":!x.prices_confirmed,"input_minor_per_million":x.input_minor_per_million,"output_minor_per_million":x.output_minor_per_million,"enabled":x.enabled,"secret":secret});
    let (revision, sha) = put(
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
        llm_models: vec![view(x.model_id.clone(), v, revision)?],
        ..Default::default()
    })
}
pub(super) fn set_budget(
    tx: &Transaction<'_>,
    x: &FoundationLlmBudget,
    now: i64,
    actor: &str,
) -> Result<FoundationResult> {
    let prior = budget(tx, &x.workspace_id)?;
    if prior.revision != x.expected_revision {
        return Err(Error::RevisionConflict);
    }
    if x.limit_minor > 1_000_000_000 || x.limit_minor < prior.used_minor {
        return Err(Error::ResourceExhausted);
    }
    let count=tx.execute("UPDATE workspaces SET budget_limit=?1,revision=revision+1,updated_at=?2 WHERE id=?3 AND revision=?4",params![int(x.limit_minor)?,now,x.workspace_id,int(x.expected_revision)?]).wb()?;
    if count != 1 {
        return Err(Error::RevisionConflict);
    }
    audit(
        tx,
        &x.workspace_id,
        actor,
        if x.limit_minor == 0 {
            "budget_disabled"
        } else {
            "budget_limited"
        },
        &x.workspace_id,
        now,
    )?;
    Ok(FoundationResult {
        resource_id: x.workspace_id.clone(),
        llm_budget: Some(budget(tx, &x.workspace_id)?),
        ..Default::default()
    })
}
fn task(
    conn: &Connection,
    root: &Path,
    actor: &str,
    ws: &str,
    id: &str,
) -> Result<(Value, u64, String)> {
    let (rev, sha) = object(conn, ws, id, 0)?;
    let v = nai_runtime_core::publication::artifact(&body(root, &sha)?)?;
    if !matches!(
        v["kind"].as_str(),
        Some("publication_llm_task" | "publication_llm_test")
    ) {
        return Err(Error::InvalidArgument);
    }
    if v["created_by"] != actor {
        return Err(Error::PermissionDenied);
    }
    Ok((v, rev, sha))
}
#[allow(clippy::too_many_arguments)]
fn context(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    v: &Value,
    now: i64,
) -> Result<[Value; 4]> {
    op.check()?;
    fence.check(fence.generation())?;
    let (_, rev) = role(conn, ws, actor)?;
    if v["role_revision"].as_u64() != Some(rev) {
        return Err(Error::PermissionDenied);
    }
    fence.check_scope(&role_scope(ws, actor, rev))?;
    if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    if v["kind"] == "publication_llm_test" {
        let (model, revision, sha) = raw(
            conn,
            root,
            ws,
            v["model_ref"]["object_id"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?,
            0,
            "publication_llm_config",
        )?;
        if model["enabled"] != true
            || Some(revision) != v["model_ref"]["revision"].as_u64()
            || sha != v["model_ref"]["sha256"]
        {
            return Err(Error::RevisionConflict);
        }
        return Ok(std::array::from_fn(|_| Value::Null));
    }
    let snapshot = publication::read(
        conn,
        root,
        fence,
        op,
        ws,
        v["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        now,
    )?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32
        || snapshot.sha256 != v["snapshot"]["sha256"]
    {
        return Err(Error::CapabilityUnavailable);
    }
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    let expected = crate::model_context::from_manifest(
        ws,
        v["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        &snapshot.sha256,
        &manifest,
    )?;
    crate::model_context::verify(&v["task_context"], &expected)?;
    if v["refs"] != manifest["refs"] {
        return Err(Error::IntegrityFailed);
    }
    let docs = publication::documents(conn, root, fence, ws, &v["refs"], true)?;
    let (model, rev, sha) = raw(
        conn,
        root,
        ws,
        v["model_ref"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        0,
        "publication_llm_config",
    )?;
    if rev
        != v["model_ref"]["revision"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?
        || sha != v["model_ref"]["sha256"]
        || model["enabled"] != true
    {
        return Err(Error::RevisionConflict);
    }
    nai_runtime_core::publication::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    Ok(docs)
}
struct Prepared {
    value: Value,
    model: Value,
    reserved: u64,
    budget: LlmBudgetView,
}
#[allow(clippy::too_many_arguments)]
fn prepare(
    tx: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationLlmStart,
) -> Result<Prepared> {
    fence.check(fence.generation())?;
    let snapshot = publication::read(tx, root, fence, op, &x.workspace_id, &x.snapshot_id, now)?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32 {
        return Err(Error::CapabilityUnavailable);
    }
    if snapshot.sha256 != x.snapshot_sha256 {
        return Err(Error::RevisionConflict);
    }
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    let docs = publication::documents(tx, root, fence, &x.workspace_id, &manifest["refs"], true)?;
    let checked =
        nai_runtime_core::publication::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    let (model, revision, sha) = raw(
        tx,
        root,
        &x.workspace_id,
        &x.model_id,
        0,
        "publication_llm_config",
    )?;
    if revision != x.model_revision {
        return Err(Error::RevisionConflict);
    }
    let (input, ids) = policy::input(op, &docs, &x.cue_id)?;
    let reserved = if model["pricing_unknown"] == true {
        x.max_cost_minor.max(1)
    } else {
        policy::amount(
            u64::try_from(input.len() + policy::SYSTEM.len() + 512)
                .map_err(|_| Error::ResourceExhausted)?,
            model["input_minor_per_million"]
                .as_u64()
                .ok_or(Error::IntegrityFailed)?,
            256,
            model["output_minor_per_million"]
                .as_u64()
                .ok_or(Error::IntegrityFailed)?,
        )?
        .max(1)
    };
    let b = budget(tx, &x.workspace_id)?;
    let mut v = json!({"schema_version":1,"kind":"publication_llm_task","created_at_ms":now,"created_by":actor,"role_revision":role_rev,"snapshot":{"object_id":x.snapshot_id,"revision":1,"sha256":snapshot.sha256},"refs":manifest["refs"],"model_ref":{"object_id":x.model_id,"revision":revision,"sha256":sha},"cue_id":x.cue_id,"state":"queued","reason":"QUEUED","text":"","fact_ids":ids,"input":input,"expires_at_ms":checked.valid_until_ms.unwrap_or(now+300000).min(now+300000),"max_output_tokens":256,"reserved_minor":reserved,"input_tokens":0,"output_tokens":0,"usage_known":false,"target":"private_workbench"});
    let context = crate::model_context::from_manifest(
        &x.workspace_id,
        &x.snapshot_id,
        &snapshot.sha256,
        &manifest,
    )?;
    v["task_context"] = nai_runtime_core::task_context::to_value(&context);
    Ok(Prepared {
        value: v,
        model,
        reserved,
        budget: b,
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn test(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationModelTest,
) -> Result<FoundationResult> {
    uuid(&x.task_id)?;
    if active(tx)? {
        return Err(Error::Busy);
    }
    fence.check(fence.generation())?;
    op.check()?;
    let (model, revision, sha) = raw(
        tx,
        root,
        &x.workspace_id,
        &x.model_id,
        0,
        "publication_llm_config",
    )?;
    if revision != x.model_revision {
        return Err(Error::RevisionConflict);
    }
    if model["enabled"] != true || model["secret"].is_null() {
        return Err(Error::CapabilityUnavailable);
    }
    let v = json!({"schema_version":1,"kind":"publication_llm_test","created_at_ms":now,"created_by":actor,"role_revision":role_rev,"model_ref":{"object_id":x.model_id,"revision":revision,"sha256":sha},"state":"queued","reason":"QUEUED","text":"","input":policy::TEST_INPUT,"expires_at_ms":now+300000,"max_output_tokens":256,"request_started":false,"input_tokens":0,"output_tokens":0,"usage_known":false,"target":"private_workbench"});
    put(tx, root, &x.workspace_id, &x.task_id, 0, &v, now)?;
    tx.execute(
        "INSERT INTO outbox VALUES(?1,?2,?3,'pending',0,1,?4,?4)",
        params![format!("llm:{}", x.task_id), x.workspace_id, x.task_id, now],
    )
    .wb()?;
    read(tx, root, fence, op, actor, &x.workspace_id, &x.task_id, now)
}
pub(super) fn safe_worker_failure(error: &str) -> &str {
    let code = error.strip_prefix("WORKBENCH_ERROR_").unwrap_or(error);
    if matches!(
        code,
        "BALANCE_INSUFFICIENT"
            | "CONNECT_TIMEOUT"
            | "CONNECTION_FAILED"
            | "RESPONSE_TIMEOUT"
            | "AUDIO_DOWNLOAD_TIMEOUT"
            | "DEADLINE_EXCEEDED"
            | "OUTCOME_UNKNOWN"
            | "CANCELLED"
            | "PROTOCOL_ERROR"
            | "RESOURCE_EXHAUSTED"
            | "CAPABILITY_UNAVAILABLE"
            | "INVALID_ARGUMENT"
            | "WORKER_LOST"
            | "WORKER_STALLED"
            | "WORKER_UNAVAILABLE"
            | "PREPARE_FAILED"
            | "RESPONSE_INVALID_JSON"
            | "RESPONSE_EMPTY_CONTENT"
            | "RESPONSE_TRUNCATED"
            | "RESPONSE_FINISH_UNEXPECTED"
            | "RESPONSE_SENSITIVE_CONTENT"
    ) || code
        .strip_prefix("HTTP_")
        .is_some_and(|s| s.len() == 3 && s.parse::<u16>().is_ok_and(|n| (100..600).contains(&n)))
    {
        code
    } else {
        "PROVIDER_FAILED"
    }
}
pub(super) fn active(conn: &Connection) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM outbox WHERE (event_id LIKE 'llm:%' OR event_id LIKE 'tts:%') AND state IN ('pending','dispatching'))",[],|r|r.get(0)).wb()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn start(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationLlmStart,
) -> Result<FoundationResult> {
    uuid(&x.task_id)?;
    if x.max_cost_minor == 0 || x.max_cost_minor > 1_000_000_000 {
        return Err(Error::InvalidArgument);
    }
    if active(tx)? {
        return Err(Error::Busy);
    }
    let prepared = prepare(tx, root, fence, op, actor, role_rev, now, x)?;
    if prepared.model["enabled"] != true || prepared.model["secret"].is_null() {
        return Err(Error::CapabilityUnavailable);
    }
    let reserved = prepared.reserved;
    admit_budget(&prepared.budget, reserved, x.max_cost_minor)?;
    put(
        tx,
        root,
        &x.workspace_id,
        &x.task_id,
        0,
        &prepared.value,
        now,
    )?;
    tx.execute(
        "INSERT INTO budget_reservations VALUES(?1,?2,?3,?4,NULL,'reserved','CNY',1,?5,?5)",
        params![x.workspace_id, x.task_id, actor, int(reserved)?, now],
    )
    .wb()?;
    tx.execute(
        "INSERT INTO outbox VALUES(?1,?2,?3,'pending',0,1,?4,?4)",
        params![format!("llm:{}", x.task_id), x.workspace_id, x.task_id, now],
    )
    .wb()?;
    read(tx, root, fence, op, actor, &x.workspace_id, &x.task_id, now)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    vault: Option<&dyn policy::CredentialVault>,
    actor: &str,
    role_rev: u64,
    now: i64,
    x: &FoundationLlmPlan,
) -> Result<FoundationResult> {
    op.check()?;
    let b = budget(conn, &x.workspace_id)?;
    let mut view = LlmLocalCheck {
        available_minor: if b.mode == WorkspaceBudgetMode::Unlimited as i32 {
            u64::MAX.saturating_sub(b.used_minor)
        } else {
            b.limit_minor.saturating_sub(b.used_minor)
        },
        max_output_tokens: 256,
        deadline_ms: 15000,
        currency: "CNY".into(),
        ..Default::default()
    };
    let mut inspect = || -> Result<LlmLocalReason> {
        let request = FoundationLlmStart {
            workspace_id: x.workspace_id.clone(),
            snapshot_id: x.snapshot_id.clone(),
            snapshot_sha256: x.snapshot_sha256.clone(),
            cue_id: x.cue_id.clone(),
            model_id: x.model_id.clone(),
            model_revision: x.model_revision,
            max_cost_minor: x.max_cost_minor,
            ..Default::default()
        };
        let p = match prepare(conn, root, fence, op, actor, role_rev, now, &request) {
            Ok(p) => p,
            Err(Error::RevisionConflict) => return Ok(LlmLocalReason::StaleContext),
            Err(Error::Cancelled) => return Ok(LlmLocalReason::Stopped),
            Err(Error::ResourceExhausted) => return Ok(LlmLocalReason::InputTooLarge),
            Err(
                Error::CapabilityUnavailable
                | Error::NotFound
                | Error::InvalidArgument
                | Error::DeadlineExceeded,
            ) => return Ok(LlmLocalReason::ContextUnavailable),
            Err(e) => return Err(e),
        };
        view.estimate_known = p.model["pricing_unknown"] != true;
        view.estimated_reserve_minor = p.reserved;
        view.input_bytes = u32::try_from(
            p.value["input"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .len(),
        )
        .map_err(|_| Error::ResourceExhausted)?;
        if p.model["enabled"] != true {
            return Ok(LlmLocalReason::ModelDisabled);
        }
        if p.model["secret"].is_null() {
            return Ok(LlmLocalReason::CredentialMissing);
        }
        if active(conn)? {
            return Ok(LlmLocalReason::Busy);
        }
        let Some(vault) = vault else {
            return Ok(LlmLocalReason::CredentialUnavailable);
        };
        let credential = vault.open(
            crate::shared_catalog::credential_scope(conn, &x.workspace_id, &x.model_id)?,
            &x.model_id,
            p.model["base_url"].as_str().ok_or(Error::IntegrityFailed)?,
            &p.model["secret"],
        );
        match credential {
            Ok(key) if !key.is_empty() => {
                drop(key);
                view.credential_checked = true;
            }
            _ => return Ok(LlmLocalReason::CredentialUnavailable),
        }
        context(
            conn,
            root,
            fence,
            op,
            actor,
            &x.workspace_id,
            &p.value,
            utc()?,
        )?;
        if b.mode == WorkspaceBudgetMode::Disabled as i32 {
            return Ok(LlmLocalReason::BudgetDisabled);
        }
        if x.max_cost_minor < p.reserved || x.max_cost_minor > 1_000_000_000 {
            return Ok(LlmLocalReason::CallLimit);
        }
        if view.available_minor < p.reserved {
            return Ok(LlmLocalReason::WorkspaceLimit);
        }
        Ok(LlmLocalReason::ReadyLocal)
    };
    view.reason = inspect()? as i32;
    op.check()?;
    Ok(FoundationResult {
        llm_check: Some(view),
        ..Default::default()
    })
}
fn resolution_id(id: &str) -> String {
    let h = hash(format!("nai-llm-resolution:{id}").as_bytes());
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}
fn resolution(conn: &Connection, root: &Path, ws: &str, id: &str) -> Result<Option<Value>> {
    let r = resolution_id(id);
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM objects WHERE workspace_id=?1 AND id=?2)",
            params![ws, r],
            |r| r.get(0),
        )
        .wb()?;
    if exists {
        let (v, _, _) = raw(conn, root, ws, &r, 0, "publication_llm_resolution")?;
        Ok(Some(v))
    } else {
        Ok(None)
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<FoundationResult> {
    let (v, rev, sha) = task(conn, root, actor, ws, id)?;
    let testing = v["kind"] == "publication_llm_test";
    let (charge, amount, actual): (String, u64, Option<u64>) = if testing {
        (
            if v["request_started"] != false {
                "unknown"
            } else {
                "not_sent"
            }
            .into(),
            0,
            None,
        )
    } else {
        conn.query_row("SELECT status,reserved_minor,actual_minor FROM budget_reservations WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|Ok((r.get(0)?,uint(r,1)?,uint_opt(r,2)?))).wb()?
    };
    let outbox: String = conn
        .query_row(
            "SELECT state FROM outbox WHERE event_id=?1",
            [format!("llm:{id}")],
            |r| r.get(0),
        )
        .wb()?;
    let state = match v["state"].as_str() {
        Some("queued") if outbox == "pending" => AgentTaskState::Queued,
        Some("running") if outbox == "dispatching" => AgentTaskState::Running,
        Some("completed") => AgentTaskState::Completed,
        Some("cancelled") => AgentTaskState::Cancelled,
        Some("failed") => AgentTaskState::Failed,
        Some("queued" | "running") => AgentTaskState::Failed,
        _ => return Err(Error::IntegrityFailed),
    };
    let mut result = FoundationResult {
        resource_id: id.into(),
        revision: rev,
        sha256: sha,
        task_state: state as i32,
        charge_status: charge,
        reserved_minor: amount,
        actual_minor: actual.unwrap_or(0),
        llm_budget: if testing {
            None
        } else {
            Some(budget(conn, ws)?)
        },
        task_reason: if matches!(v["state"].as_str(), Some("queued" | "running"))
            && state == AgentTaskState::Failed
        {
            "ENGINE_RESTARTED".into()
        } else {
            v["reason"].as_str().ok_or(Error::IntegrityFailed)?.into()
        },
        ..Default::default()
    };
    if v["kind"] == "publication_llm_test" {
        result.suggestion_state = SuggestionState::Unspecified as i32;
    }
    if fence.check_scope(&object_scope(ws, id)).is_err() {
        result.suggestion_state = SuggestionState::Rejected as i32;
        result.task_reason = "CANCELLED".into();
        if state != AgentTaskState::Completed {
            result.task_state = AgentTaskState::Cancelled as i32;
        }
        return Ok(result);
    }
    if let Some(r) = resolution(conn, root, ws, id)? {
        result.suggestion_state = match r["decision"].as_str() {
            Some("accept") => SuggestionState::Accepted,
            Some("edit") => SuggestionState::Edited,
            Some("ignore") => SuggestionState::Rejected,
            _ => return Err(Error::IntegrityFailed),
        } as i32;
        result.adopted_object_id = r["adopted_object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .into();
        result.adopted_revision = r["adopted_revision"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?;
        return Ok(result);
    }
    if let Err(e) = context(conn, root, fence, op, actor, ws, &v, now) {
        result.suggestion_state = SuggestionState::Expired as i32;
        result.task_reason = if e == Error::RevisionConflict {
            "STALE_CONTEXT"
        } else if e == Error::DeadlineExceeded {
            "EXPIRED"
        } else {
            "CONTEXT_UNAVAILABLE"
        }
        .into();
        return Ok(result);
    }
    if !testing {
        result.agent_context = Some(nai_runtime_core::task_context::from_value(
            &v["task_context"],
        )?);
    }
    if state == AgentTaskState::Completed {
        let mut safe = v.clone();
        safe.as_object_mut()
            .ok_or(Error::IntegrityFailed)?
            .remove("input");
        result.body = safe.to_string();
        result.suggestion_state = if v["kind"] == "publication_llm_test" {
            SuggestionState::Unspecified
        } else {
            SuggestionState::Proposed
        } as i32;
    }
    Ok(result)
}
pub(super) fn cancel(
    tx: &Transaction<'_>,
    root: &Path,
    actor: &str,
    fence: &StopFence,
    now: i64,
    x: &FoundationLlmTaskRead,
) -> Result<FoundationResult> {
    let (mut v, rev, _) = task(tx, root, actor, &x.workspace_id, &x.task_id)?;
    let status = v["state"].as_str().ok_or(Error::IntegrityFailed)?;
    if !matches!(status, "queued" | "running" | "cancelled") {
        return Err(Error::RevisionConflict);
    }
    root::revoke(root, fence, &object_scope(&x.workspace_id, &x.task_id))?;
    if status == "queued" || status == "running" {
        let outbox: String = tx
            .query_row(
                "SELECT state FROM outbox WHERE event_id=?1",
                [format!("llm:{}", x.task_id)],
                |r| r.get(0),
            )
            .wb()?;
        let charge = if outbox == "pending" {
            "released"
        } else {
            "unknown"
        };
        if v["kind"] == "publication_llm_test" {
            v["request_started"] = (outbox != "pending").into();
        } else {
            tx.execute("UPDATE budget_reservations SET status=?1,actual_minor=NULL,revision=revision+1,updated_at=?2 WHERE workspace_id=?3 AND id=?4 AND status='reserved'",params![charge,now,x.workspace_id,x.task_id]).wb()?;
        }
        tx.execute("UPDATE outbox SET state='cancelled',revision=revision+1,updated_at=?1 WHERE event_id=?2",params![now,format!("llm:{}",x.task_id)]).wb()?;
        v["state"] = "cancelled".into();
        v["reason"] = "CANCELLED".into();
        v["text"] = "".into();
        put(tx, root, &x.workspace_id, &x.task_id, rev, &v, now)?;
    }
    Ok(FoundationResult {
        resource_id: x.task_id.clone(),
        task_state: AgentTaskState::Cancelled as i32,
        ..Default::default()
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    tx: &Transaction<'_>,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    now: i64,
    x: &FoundationLlmResolve,
) -> Result<FoundationResult> {
    let (v, rev, sha) = task(tx, root, actor, &x.workspace_id, &x.task_id)?;
    if v["kind"] == "publication_llm_test" {
        return Err(Error::InvalidArgument);
    }
    if sha != x.task_sha256 || resolution(tx, root, &x.workspace_id, &x.task_id)?.is_some() {
        return Err(Error::RevisionConflict);
    }
    let decision = FaqDecision::try_from(x.decision).map_err(|_| Error::InvalidArgument)?;
    let name = match decision {
        FaqDecision::Accept => "accept",
        FaqDecision::Edit => "edit",
        FaqDecision::Ignore => "ignore",
        _ => return Err(Error::InvalidArgument),
    };
    if decision != FaqDecision::Edit && (!x.edited_text.is_empty() || x.acknowledge_edit) {
        return Err(Error::InvalidArgument);
    }
    let (mut adopted, mut adopted_rev) = (String::new(), 0);
    if matches!(decision, FaqDecision::Accept | FaqDecision::Edit) {
        let docs = context(tx, root, fence, op, actor, &x.workspace_id, &v, now)?;
        if v["state"] != "completed" {
            return Err(Error::CapabilityUnavailable);
        }
        let text = if decision == FaqDecision::Edit {
            if !x.acknowledge_edit || x.edited_text.chars().count() > 2048 {
                return Err(Error::InvalidArgument);
            }
            manual_guard(&x.edited_text)?;
            x.edited_text.as_str()
        } else {
            v["text"].as_str().ok_or(Error::IntegrityFailed)?
        };
        let mut product = docs[1].clone();
        let tracks = product["talk_tracks"]
            .as_array_mut()
            .ok_or(Error::IntegrityFailed)?;
        if tracks.len() >= 32 {
            return Err(Error::ResourceExhausted);
        }
        tracks.push(json!({"id":format!("llm_{}",x.task_id.replace('-',"")),"text":text,"fact_ids":v["fact_ids"]}));
        let bytes = product.to_string();
        nai_runtime_core::preparation::validate(op, &bytes)?;
        adopted = v["refs"]["product"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .into();
        let prior = v["refs"]["product"]["revision"]
            .as_u64()
            .ok_or(Error::IntegrityFailed)?;
        if prior >= 1024 {
            return Err(Error::ResourceExhausted);
        }
        adopted_rev = prior + 1;
        let sha = store_body(root, bytes.as_bytes())?;
        let count=tx.execute("UPDATE objects SET revision=revision+1,updated_at=?1 WHERE workspace_id=?2 AND id=?3 AND revision=?4 AND deleted=0",params![now,x.workspace_id,adopted,int(prior)?]).wb()?;
        if count != 1 {
            return Err(Error::RevisionConflict);
        }
        tx.execute(
            "INSERT INTO object_versions VALUES(?1,?2,?3,?4,?5,?5)",
            params![x.workspace_id, adopted, int(adopted_rev)?, sha, now],
        )
        .wb()?;
    }
    let value = json!({"schema_version":1,"kind":"publication_llm_resolution","created_at_ms":now,"created_by":actor,"task":{"object_id":x.task_id,"revision":rev,"sha256":sha},"decision":name,"adopted_object_id":adopted,"adopted_revision":adopted_rev});
    publication::insert(
        tx,
        root,
        &x.workspace_id,
        &resolution_id(&x.task_id),
        &value,
        now,
    )?;
    read(tx, root, fence, op, actor, &x.workspace_id, &x.task_id, now)
}
/// Last gate before reservation/adoption commit. Adoption deliberately advances the product head.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_gate(
    conn: &Connection,
    root: &Path,
    fence: &StopFence,
    op: &Operation,
    actor: &str,
    ws: &str,
    id: &str,
    now: i64,
) -> Result<()> {
    let (v, _, _) = task(conn, root, actor, ws, id)?;
    fence.check(fence.generation())?;
    fence.check_scope(&object_scope(ws, id))?;
    let (_, revision) = role(conn, ws, actor)?;
    if v["role_revision"].as_u64() != Some(revision) {
        return Err(Error::PermissionDenied);
    }
    fence.check_scope(&role_scope(ws, actor, revision))?;
    if now < v["created_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
        || now >= v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)?
    {
        return Err(Error::DeadlineExceeded);
    }
    if v["kind"] == "publication_llm_test" {
        context(conn, root, fence, op, actor, ws, &v, now)?;
        return op.check();
    }
    let snapshot = publication::read(
        conn,
        root,
        fence,
        op,
        ws,
        v["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        now,
    )?;
    if snapshot.publication_status != PublicationStatus::PrivateContent as i32
        || snapshot.sha256 != v["snapshot"]["sha256"]
    {
        return Err(Error::CapabilityUnavailable);
    }
    let manifest: Value =
        serde_json::from_str(&snapshot.body).map_err(|_| Error::IntegrityFailed)?;
    let expected = crate::model_context::from_manifest(
        ws,
        v["snapshot"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?,
        &snapshot.sha256,
        &manifest,
    )?;
    crate::model_context::verify(&v["task_context"], &expected)?;
    if v["refs"] != manifest["refs"] {
        return Err(Error::IntegrityFailed);
    }
    // The sole writer transaction excludes competing model changes; recheck the snapshot facts at the actual commit time.
    let docs = publication::documents(conn, root, fence, ws, &v["refs"], false)?;
    nai_runtime_core::publication::closure(op, [&docs[0], &docs[1], &docs[2], &docs[3]], now)?;
    op.check()
}
impl Store {
    pub fn attach_llm_vault(&mut self, vault: Arc<dyn policy::CredentialVault>) {
        self.llm_vault = Some(vault);
    }
    pub fn root_owner(&self) -> &str {
        &self.root_owner
    }
    /// Construct only after a committed queue reservation; the worker never receives database access.
    pub fn llm_request(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
    ) -> Result<LlmWorkerRequest> {
        self.begin(op)?;
        let (v, _, _) = task(&self.conn, &self.root, actor, ws, id)?;
        context(
            &self.conn,
            &self.root,
            &self.fence,
            op,
            actor,
            ws,
            &v,
            utc()?,
        )?;
        if v["state"] != "queued" {
            return Err(Error::RevisionConflict);
        }
        let model_id = v["model_ref"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?;
        let (model, _, _) = raw(
            &self.conn,
            &self.root,
            ws,
            model_id,
            0,
            "publication_llm_config",
        )?;
        let endpoint = model["base_url"].as_str().ok_or(Error::IntegrityFailed)?;
        let key = self
            .llm_vault
            .as_deref()
            .ok_or(Error::CapabilityUnavailable)?
            .open(
                crate::shared_catalog::credential_scope(&self.conn, ws, model_id)?,
                model_id,
                endpoint,
                &model["secret"],
            )?;
        Ok(LlmWorkerRequest {
            task_id: id.into(),
            base_url: endpoint.into(),
            model: model["model"]
                .as_str()
                .ok_or(Error::IntegrityFailed)?
                .into(),
            api_key: key.to_vec(),
            system: if v["kind"] == "publication_llm_test" {
                policy::TEST_SYSTEM
            } else {
                policy::SYSTEM
            }
            .into(),
            connection_test: v["kind"] == "publication_llm_test",
            input: v["input"].as_str().ok_or(Error::IntegrityFailed)?.into(),
            max_output_tokens: 256,
            deadline_ms: u32::try_from(
                (v["expires_at_ms"].as_i64().ok_or(Error::IntegrityFailed)? - utc()?)
                    .clamp(1, 15000),
            )
            .map_err(|_| Error::DeadlineExceeded)?,
        })
    }
    pub fn claim_llm(&mut self, op: &Operation, actor: &str, ws: &str, id: &str) -> Result<()> {
        self.begin(op)?;
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        let (mut v, rev, _) = task(&tx, &self.root, actor, ws, id)?;
        self.fence.check_scope(&object_scope(ws, id))?;
        context(&tx, &self.root, &self.fence, op, actor, ws, &v, now)?;
        if v["state"] != "queued" {
            return Err(Error::RevisionConflict);
        }
        let changed=tx.execute("UPDATE outbox SET state='dispatching',attempts=1,revision=revision+1,updated_at=?1 WHERE event_id=?2 AND state='pending' AND attempts=0",params![now,format!("llm:{id}")]).wb()?;
        if changed != 1 {
            return Err(Error::RevisionConflict);
        }
        v["state"] = "running".into();
        v["reason"] = "RUNNING".into();
        if v["kind"] == "publication_llm_test" {
            v["request_started"] = true.into();
        }
        put(&tx, &self.root, ws, id, rev, &v, now)?;
        audit(&tx, ws, actor, "llm_dispatch", id, now)?;
        commit_gate(&tx, &self.root, &self.fence, op, actor, ws, id, utc()?)?;
        op.check()?;
        tx.commit().wb()?;
        Ok(())
    }
    pub fn llm_current(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        id: &str,
    ) -> Result<FoundationResult> {
        self.begin(op)?;
        role(&self.conn, ws, actor)?;
        read(
            &self.conn,
            &self.root,
            &self.fence,
            op,
            actor,
            ws,
            id,
            utc()?,
        )
    }
    /// Internal trusted completion path: settlement must survive actor/context revocation.
    pub fn finish_llm(
        &mut self,
        op: &Operation,
        actor: &str,
        ws: &str,
        reply: &LlmWorkerResult,
    ) -> Result<()> {
        self.begin(op)?;
        let now = utc()?;
        let tx = self.conn.transaction().wb()?;
        let (mut v, rev, _) = task(&tx, &self.root, actor, ws, &reply.task_id)?;
        let state = v["state"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?
            .to_owned();
        if !matches!(state.as_str(), "queued" | "running" | "cancelled") {
            return Ok(());
        }
        let model_id = v["model_ref"]["object_id"]
            .as_str()
            .ok_or(Error::IntegrityFailed)?;
        let (model, _, _) = raw(
            &tx,
            &self.root,
            ws,
            model_id,
            v["model_ref"]["revision"]
                .as_u64()
                .ok_or(Error::IntegrityFailed)?,
            "publication_llm_config",
        )?;
        let testing = v["kind"] == "publication_llm_test";
        let known = (testing || model["pricing_unknown"] != true)
            && reply.usage_known
            && reply.input_tokens <= 1_000_000_000
            && reply.output_tokens <= 1_000_000_000;
        let cost = if known && !testing {
            Some(policy::amount(
                reply.input_tokens,
                model["input_minor_per_million"]
                    .as_u64()
                    .ok_or(Error::IntegrityFailed)?,
                reply.output_tokens,
                model["output_minor_per_million"]
                    .as_u64()
                    .ok_or(Error::IntegrityFailed)?,
            )?)
        } else {
            None
        };
        let charge = if known {
            "known"
        } else if !reply.request_started {
            "released"
        } else {
            "unknown"
        };
        if v["kind"] == "publication_llm_test" {
            v["request_started"] = reply.request_started.into();
        } else {
            tx.execute("UPDATE budget_reservations SET status=?1,actual_minor=?2,revision=revision+1,updated_at=?3 WHERE workspace_id=?4 AND id=?5 AND status IN('reserved','unknown')",params![charge,cost.map(int).transpose()?,now,ws,reply.task_id]).wb()?;
        }
        let checked = self
            .fence
            .check_scope(&object_scope(ws, &reply.task_id))
            .and_then(|()| context(&tx, &self.root, &self.fence, op, actor, ws, &v, now));
        let text = if v["kind"] == "publication_llm_test" && reply.error.is_empty() {
            if reply.text.trim().is_empty() || reply.text.len() > 8192 {
                Err(Error::ProtocolError)
            } else {
                Ok(reply.text.chars().take(512).collect::<String>())
            }
        } else if reply.error.is_empty() {
            policy::output(
                v["input"].as_str().ok_or(Error::IntegrityFailed)?,
                &reply.text,
            )
        } else {
            Err(Error::CapabilityUnavailable)
        };
        if self
            .fence
            .check_scope(&object_scope(ws, &reply.task_id))
            .is_err()
            || reply.error == Error::Cancelled.as_str_name()
        {
            v["state"] = "cancelled".into();
            v["reason"] = "CANCELLED".into();
            v["text"] = "".into();
        } else if state != "cancelled" {
            if checked.is_ok() && text.is_ok() && state == "running" {
                v["state"] = "completed".into();
                v["reason"] = if v["kind"] == "publication_llm_test" {
                    "API_RESPONSE_OK"
                } else {
                    "COMPLETED_REVIEW_REQUIRED"
                }
                .into();
                v["text"] = text?.into();
            } else {
                v["state"] = "failed".into();
                v["reason"] = if checked.is_err() {
                    "CONTEXT_UNAVAILABLE".into()
                } else if !reply.error.is_empty() {
                    safe_worker_failure(&reply.error).into()
                } else {
                    "OUTPUT_REJECTED".into()
                };
                v["text"] = "".into();
            }
        }
        v["usage_known"] = known.into();
        v["input_tokens"] = reply.input_tokens.into();
        v["output_tokens"] = reply.output_tokens.into();
        put(&tx, &self.root, ws, &reply.task_id, rev, &v, now)?;
        tx.execute(
            "UPDATE outbox SET state=?1,revision=revision+1,updated_at=?2 WHERE event_id=?3",
            params![
                if v["state"] == "cancelled" {
                    "cancelled"
                } else {
                    "applied"
                },
                now,
                format!("llm:{}", reply.task_id)
            ],
        )
        .wb()?;
        audit(&tx, ws, actor, "llm_finish_settle", &reply.task_id, now)?;
        op.check()?;
        tx.commit().wb()?;
        Ok(())
    }
}

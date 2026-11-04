use super::*;
pub(super) fn mode(conn: &Connection, ws: &str, limit: u64) -> Result<i32> {
    let action:Option<String>=conn.query_row("SELECT action FROM audit_events WHERE workspace_id=?1 AND action IN('budget_unlimited','budget_disabled','budget_limited','llm_budget_set') ORDER BY seq DESC LIMIT 1",[ws],|r|r.get(0)).optional().wb()?;
    Ok(match action.as_deref() {
        Some("budget_unlimited") => WorkspaceBudgetMode::Unlimited as i32,
        Some("budget_disabled") => WorkspaceBudgetMode::Disabled as i32,
        _ => {
            if limit == 0 {
                WorkspaceBudgetMode::Disabled as i32
            } else {
                WorkspaceBudgetMode::Limited as i32
            }
        }
    })
}
pub(super) fn apply(
    tx: &Transaction<'_>,
    actor: &str,
    now: i64,
    x: &WorkspaceSettingsCommand,
    create: bool,
) -> Result<FoundationResult> {
    let s = x.settings.as_ref().ok_or(Error::InvalidArgument)?;
    let ws = &x.workspace_id;
    let mode = nai_runtime_core::workspace_budget::validate(s.budget_mode, s.budget_limit_minor)?;
    let name = s.name.trim();
    if name.is_empty()
        || name.len() > 128
        || name.chars().any(char::is_control)
        || s.currency != "CNY"
    {
        return Err(Error::InvalidArgument);
    }
    if create {
        if s.expected_revision != 0 || workspace_lifecycle::state(tx, ws)? == "workspace_delete" {
            return Err(Error::RevisionConflict);
        }
        tx.execute(
            "INSERT INTO workspaces VALUES(?1,?2,1,?3,?3,?4,'CNY')",
            params![ws, name, now, int(s.budget_limit_minor)?],
        )
        .wb()?;
        tx.execute(
            "INSERT INTO roles VALUES(?1,?2,1,1,?3,?3)",
            params![ws, actor, now],
        )
        .wb()?;
    } else {
        let prior = llm::budget(tx, ws)?;
        if prior.revision != s.expected_revision {
            return Err(Error::RevisionConflict);
        }
        if mode == WorkspaceBudgetMode::Limited && s.budget_limit_minor < prior.used_minor {
            return Err(Error::ResourceExhausted);
        }
        if tx.execute("UPDATE workspaces SET name=?1,budget_limit=?2,revision=revision+1,updated_at=?3 WHERE id=?4 AND revision=?5",params![name,int(s.budget_limit_minor)?,now,ws,int(s.expected_revision)?]).wb()?!=1{return Err(Error::RevisionConflict);}
    }
    let label = match mode {
        WorkspaceBudgetMode::Unlimited => "budget_unlimited",
        WorkspaceBudgetMode::Disabled => "budget_disabled",
        _ => "budget_limited",
    };
    audit(tx, ws, actor, label, ws, now)?;
    Ok(FoundationResult {
        resource_id: ws.clone(),
        revision: if create { 1 } else { s.expected_revision + 1 },
        llm_budget: Some(llm::budget(tx, ws)?),
        ..Default::default()
    })
}

//! Shared source identity and bounded local interaction admission. No output authority.
use crate::foundation::{Result, uuid};
use nai_contracts::workbench::{AgentTaskContext, WorkbenchError as Error};
pub const POOL_CAPACITY: usize = 32;
pub const POOL_TTL_MS: i64 = 120000;
pub fn source(
    workspace: &str,
    snapshot: &str,
    snapshot_sha: &str,
    facts_sha: &str,
) -> Result<AgentTaskContext> {
    uuid(workspace)?;
    uuid(snapshot)?;
    if [snapshot_sha, facts_sha].iter().any(|s| {
        s.len() != 64
            || !s
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }) {
        return Err(Error::InvalidArgument);
    }
    let mut c = AgentTaskContext {
        workspace_id: workspace.into(),
        snapshot_id: snapshot.into(),
        snapshot_sha256: snapshot_sha.into(),
        facts_sha256: facts_sha.into(),
        context_sha256: String::new(),
    };
    c.context_sha256 = nai_contracts::agent_task_context_hash(&c);
    Ok(c)
}
/// Persisted shape shared by FAQ, text generation and speech preparation.
pub fn to_value(c: &AgentTaskContext) -> serde_json::Value {
    serde_json::json!({"workspace_id":c.workspace_id,"snapshot_id":c.snapshot_id,"snapshot_sha256":c.snapshot_sha256,"facts_sha256":c.facts_sha256,"context_sha256":c.context_sha256})
}
pub fn from_value(v: &serde_json::Value) -> Result<AgentTaskContext> {
    let text = |key: &str| v[key].as_str().ok_or(Error::IntegrityFailed);
    let c = source(
        text("workspace_id")?,
        text("snapshot_id")?,
        text("snapshot_sha256")?,
        text("facts_sha256")?,
    )?;
    if text("context_sha256")? != c.context_sha256 {
        return Err(Error::IntegrityFailed);
    }
    Ok(c)
}
pub fn question_key(question: &str) -> Result<String> {
    if question.chars().count() > 256 || question.chars().any(char::is_control) {
        return Err(Error::InvalidArgument);
    }
    let normalized = question
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .trim_end_matches(['?', '？', '!', '！'])
        .to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(Error::InvalidArgument);
    }
    Ok(nai_contracts::question_dedup_hash(&normalized))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_scope_and_question_keys_do_not_merge_other_contexts() {
        let id = "00000000-0000-4000-8000-000000000001";
        let c = source(id, id, &"a".repeat(64), &"b".repeat(64)).unwrap();
        assert_ne!(
            c.context_sha256,
            source(id, id, &"c".repeat(64), &"b".repeat(64))
                .unwrap()
                .context_sha256
        );
        assert_eq!(
            question_key(" 容量 多大？").unwrap(),
            question_key("容量多大?").unwrap()
        );
        assert_ne!(
            question_key("可以退货").unwrap(),
            question_key("不可以退货").unwrap()
        );
        assert!(question_key("？？").is_err());
        assert!(source(id, id, "bad", &"b".repeat(64)).is_err());
    }
}

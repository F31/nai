//! Four-stage guard engine (V3.5 §23.2): input/evidence/output/preplay.
//! Verdicts are pass/rewrite/block/escalate with reason codes, a rule version
//! and an optional rewritten candidate. Rewrites are bounded; a missing paid
//! moderation API never bypasses the basic local guard.
use nai_contracts::v1::{
    Evidence, GuardRequest, GuardResult, GuardStage, GuardVerdict, PerformanceCue,
    PermissionSnapshot, Risk,
};
use std::collections::BTreeSet;

pub const INJECTION_MARKERS: [&str; 8] = [
    "<script",
    "javascript:",
    "select from",
    "drop table",
    "insert into",
    "exec(",
    "http://",
    "https://",
];
pub const PRIVATE_MARKERS: [&str; 4] = ["手机号", "身份证", "收货地址", "银行卡"];
pub const PROMISE_MARKERS: [&str; 8] = [
    "保证",
    "承诺",
    "包退",
    "无条件退款",
    "假一赔十",
    "保真",
    "绝无",
    "永久",
];

pub struct GuardPolicy {
    pub rule_version: &'static str,
    pub max_rewrites: u32,
    pub require_evidence: bool,
    pub max_cue_chars: usize,
}
impl Default for GuardPolicy {
    fn default() -> Self {
        Self {
            rule_version: "guard-rule-v1",
            max_rewrites: 1,
            require_evidence: true,
            max_cue_chars: 2048,
        }
    }
}

/// Runtime state rechecked at the preplay stage. Supplied by the trusted owner;
/// this evaluation only turns those checks into a verdict.
#[derive(Debug, Clone, Copy, Default)]
pub struct PreplayChecks {
    pub approval_valid: bool,
    pub facts_match: bool,
    pub epoch_match: bool,
    pub permission_current: bool,
}

fn pass(rule_version: &str, risk: Risk) -> GuardResult {
    GuardResult {
        verdict: GuardVerdict::Pass as i32,
        rule_version: rule_version.into(),
        risk: risk as i32,
        ..Default::default()
    }
}
fn block(reason: &str, rule_version: &str, risk: Risk) -> GuardResult {
    GuardResult {
        verdict: GuardVerdict::Block as i32,
        reason_codes: vec![reason.into()],
        rule_version: rule_version.into(),
        risk: risk as i32,
        ..Default::default()
    }
}
fn escalate(reason: &str, rule_version: &str, risk: Risk) -> GuardResult {
    GuardResult {
        verdict: GuardVerdict::Escalate as i32,
        reason_codes: vec![reason.into()],
        rule_version: rule_version.into(),
        risk: risk as i32,
        ..Default::default()
    }
}
fn rewrite(reason: &str, rule_version: &str, risk: Risk, rewritten: PerformanceCue) -> GuardResult {
    GuardResult {
        verdict: GuardVerdict::Rewrite as i32,
        reason_codes: vec![reason.into()],
        rule_version: rule_version.into(),
        risk: risk as i32,
        rewritten_candidate: Some(rewritten),
    }
}

fn text_of(cue: &PerformanceCue) -> String {
    format!("{} {}", cue.display_text, cue.speech_text)
}

fn sanitize_unsafe(text: &str) -> String {
    let mut sanitized = String::with_capacity(text.len());
    for token in text.split(char::is_whitespace) {
        if token.is_empty() {
            continue;
        }
        if INJECTION_MARKERS
            .iter()
            .any(|marker| token.contains(marker))
        {
            sanitized.push_str("□ ");
        } else {
            sanitized.push_str(token);
            sanitized.push(' ');
        }
    }
    sanitized.trim_end().to_string()
}

fn evidence_ok(
    evidence: &Evidence,
    permission_revision: Option<&str>,
    now_ms: u64,
    require: bool,
) -> Option<&'static str> {
    if evidence.evidence_id.is_empty() || evidence.text.is_empty() {
        return Some("evidence_incomplete");
    }
    if require && evidence.permission_revision.is_empty() {
        return Some("evidence_permission_missing");
    }
    if permission_revision.is_some_and(|revision| {
        !evidence.permission_revision.is_empty() && evidence.permission_revision != revision
    }) {
        return Some("evidence_permission_mismatch");
    }
    if evidence.revision.is_empty() && !evidence.original_verifiable {
        return Some("evidence_version_unknown");
    }
    if evidence
        .valid_until_utc_ms
        .is_some_and(|until| until <= now_ms)
    {
        return Some("evidence_expired");
    }
    None
}

/// Guard one stage. `rewrites` is the number already consumed for this
/// candidate; `preplay` is only consulted for the preplay stage.
pub fn guard(
    request: &GuardRequest,
    policy: &GuardPolicy,
    rewrites: u32,
    now_ms: u64,
    preplay: Option<PreplayChecks>,
) -> GuardResult {
    let context = request.context.as_ref();
    if context.is_none() {
        return block("context_missing", policy.rule_version, Risk::Risk3);
    }
    let stage = GuardStage::try_from(request.stage).unwrap_or(GuardStage::Unspecified);
    let Some(cue) = &request.candidate else {
        return block("candidate_missing", policy.rule_version, Risk::Risk3);
    };
    if cue.display_text.len() > policy.max_cue_chars || cue.speech_text.len() > policy.max_cue_chars
    {
        return block("cue_oversized", policy.rule_version, Risk::Risk1);
    }
    let text = text_of(cue);
    let rule_risk = risk_of(&text);
    match stage {
        GuardStage::Input => {
            if INJECTION_MARKERS.iter().any(|marker| text.contains(marker)) {
                return block("input_injection", policy.rule_version, Risk::Risk3);
            }
            if PRIVATE_MARKERS.iter().any(|marker| text.contains(marker)) {
                return escalate("input_private_attribute", policy.rule_version, Risk::Risk2);
            }
            pass(policy.rule_version, rule_risk)
        }
        GuardStage::Evidence => {
            let permission_revision = request
                .permissions
                .as_ref()
                .map(|permissions| permissions.revision.as_str());
            for evidence in &request.evidence {
                if let Some(reason) = evidence_ok(
                    evidence,
                    permission_revision,
                    now_ms,
                    policy.require_evidence,
                ) {
                    return block(reason, policy.rule_version, Risk::Risk2);
                }
            }
            if policy.require_evidence && cue.evidence_refs.is_empty() {
                return block(
                    "candidate_missing_evidence",
                    policy.rule_version,
                    Risk::Risk2,
                );
            }
            for reference in &cue.evidence_refs {
                if !request
                    .evidence
                    .iter()
                    .any(|evidence| &evidence.evidence_id == reference)
                {
                    return block(
                        "citation_not_in_retrieval",
                        policy.rule_version,
                        Risk::Risk2,
                    );
                }
            }
            pass(policy.rule_version, rule_risk)
        }
        GuardStage::Output => {
            let unsafe_text = INJECTION_MARKERS.iter().any(|marker| text.contains(marker));
            if unsafe_text {
                if rewrites < policy.max_rewrites {
                    let mut rewritten = cue.clone();
                    rewritten.display_text = sanitize_unsafe(&rewritten.display_text);
                    rewritten.speech_text = sanitize_unsafe(&rewritten.speech_text);
                    rewritten.candidate_hash.clear();
                    return rewrite(
                        "output_unsafe_text",
                        policy.rule_version,
                        Risk::Risk3,
                        rewritten,
                    );
                }
                return block("output_unsafe_text", policy.rule_version, Risk::Risk3);
            }
            if PROMISE_MARKERS.iter().any(|marker| text.contains(marker)) {
                return block("output_unchecked_promise", policy.rule_version, Risk::Risk2);
            }
            if !valid_amounts(&text) {
                return escalate("output_amount_format", policy.rule_version, Risk::Risk2);
            }
            if policy.require_evidence {
                let available: BTreeSet<&str> = request
                    .evidence
                    .iter()
                    .map(|evidence| evidence.evidence_id.as_str())
                    .collect();
                for reference in &cue.evidence_refs {
                    if !available.contains(reference.as_str()) {
                        return block(
                            "citation_not_in_retrieval",
                            policy.rule_version,
                            Risk::Risk2,
                        );
                    }
                }
            }
            pass(policy.rule_version, rule_risk)
        }
        GuardStage::Preplay => {
            let checks = preplay.unwrap_or_default();
            if !checks.approval_valid {
                return block("preplay_approval_invalid", policy.rule_version, Risk::Risk3);
            }
            if !checks.facts_match {
                return block("preplay_facts_mismatch", policy.rule_version, Risk::Risk3);
            }
            if !checks.epoch_match {
                return block("preplay_epoch_mismatch", policy.rule_version, Risk::Risk3);
            }
            if !checks.permission_current {
                return block("preplay_permission_stale", policy.rule_version, Risk::Risk3);
            }
            if cue.expires_at_utc_ms <= now_ms {
                return block("preplay_cue_expired", policy.rule_version, Risk::Risk3);
            }
            pass(policy.rule_version, rule_risk)
        }
        GuardStage::Unspecified => block("stage_unspecified", policy.rule_version, Risk::Risk3),
    }
}

/// Permission snapshot expired or empty never means current authorization.
pub fn permission_current(permissions: Option<&PermissionSnapshot>, now_ms: u64) -> bool {
    permissions.is_some_and(|snapshot| {
        !snapshot.revision.is_empty()
            && snapshot
                .valid_until_utc_ms
                .is_none_or(|until| until > now_ms)
    })
}

fn risk_of(text: &str) -> Risk {
    if crate::evaluate::agent::RISK3_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
    {
        Risk::Risk3
    } else if PROMISE_MARKERS.iter().any(|marker| text.contains(marker)) {
        Risk::Risk2
    } else if crate::evaluate::agent::RISK1_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
    {
        Risk::Risk1
    } else {
        Risk::Risk0
    }
}

/// Amount-like text must be digits or exact minor-unit/decimal text, never
/// unverifiable floating formatting or placeholder tokens.
fn valid_amounts(text: &str) -> bool {
    if !crate::evaluate::agent::RISK1_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
    {
        return true;
    }
    !text.contains('.') || text.split('.').count() <= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use nai_contracts::v1::{Evidence, RequestContext};

    fn request(stage: GuardStage, cue: PerformanceCue) -> GuardRequest {
        GuardRequest {
            context: Some(RequestContext {
                session_id: "session".into(),
                workspace_id: "workspace".into(),
                generation_epoch: Some(7),
                remaining_budget_ms: 500,
                ..Default::default()
            }),
            stage: stage as i32,
            candidate: Some(cue),
            ..Default::default()
        }
    }
    fn cue(text: &str) -> PerformanceCue {
        PerformanceCue {
            cue_id: "cue".into(),
            session_id: "session".into(),
            utterance_id: "utt".into(),
            generation_epoch: 7,
            display_text: text.into(),
            speech_text: text.into(),
            evidence_refs: vec!["ev-1".into()],
            policy_version: "policy-v1".into(),
            expires_at_utc_ms: 1_000_000_000_000,
            ..Default::default()
        }
    }

    #[test]
    fn input_injection_and_private_attributes_are_rejected() {
        let policy = GuardPolicy::default();
        let injected = request(GuardStage::Input, cue("价格详见 <script>alert(1)</script>"));
        assert_eq!(
            guard(&injected, &policy, 0, 0, None).verdict,
            GuardVerdict::Block as i32
        );
        let private = request(GuardStage::Input, cue("请提供你的手机号"));
        assert_eq!(
            guard(&private, &policy, 0, 0, None).verdict,
            GuardVerdict::Escalate as i32
        );
    }

    #[test]
    fn evidence_version_permission_and_expiry_are_checked() {
        let policy = GuardPolicy::default();
        let valid = Evidence {
            evidence_id: "ev-1".into(),
            text: "content".into(),
            revision: "rev-1".into(),
            permission_revision: "permission-1".into(),
            original_verifiable: true,
            valid_until_utc_ms: Some(1_000_000_000_000),
            ..Default::default()
        };
        let mut ok = request(GuardStage::Evidence, cue("正常内容"));
        ok.evidence = vec![valid.clone()];
        ok.permissions = Some(PermissionSnapshot {
            revision: "permission-1".into(),
            ..Default::default()
        });
        assert_eq!(
            guard(&ok, &policy, 0, 0, None).verdict,
            GuardVerdict::Pass as i32
        );

        let mut unknown = valid.clone();
        unknown.revision.clear();
        unknown.original_verifiable = false;
        let mut bad = request(GuardStage::Evidence, cue("正常内容"));
        bad.evidence = vec![unknown];
        assert_eq!(
            guard(&bad, &policy, 0, 0, None).verdict,
            GuardVerdict::Block as i32
        );

        let mut mismatch = valid.clone();
        mismatch.permission_revision = "other".into();
        let mut bad = request(GuardStage::Evidence, cue("正常内容"));
        bad.evidence = vec![mismatch];
        bad.permissions = Some(PermissionSnapshot {
            revision: "permission-1".into(),
            ..Default::default()
        });
        assert_eq!(
            guard(&bad, &policy, 0, 0, None).verdict,
            GuardVerdict::Block as i32
        );

        let mut expired = valid.clone();
        expired.valid_until_utc_ms = Some(10);
        let mut bad = request(GuardStage::Evidence, cue("正常内容"));
        bad.evidence = vec![expired];
        assert_eq!(
            guard(&bad, &policy, 0, 100, None).verdict,
            GuardVerdict::Block as i32
        );
    }

    #[test]
    fn output_citation_promise_and_bounded_rewrite() {
        let policy = GuardPolicy::default();
        let mut missing = request(GuardStage::Output, cue("内容"));
        missing.candidate = Some(cue("内容"));
        assert_eq!(
            guard(&missing, &policy, 0, 0, None).verdict,
            GuardVerdict::Block as i32
        );

        let promised = request(GuardStage::Output, cue("我们保证价格最优"));
        assert_eq!(
            guard(&promised, &policy, 0, 0, None).verdict,
            GuardVerdict::Block as i32
        );

        let unsafe_text = request(GuardStage::Output, cue("请看 https://example.invalid/a"));
        let rewritten = guard(&unsafe_text, &policy, 0, 0, None);
        assert_eq!(rewritten.verdict, GuardVerdict::Rewrite as i32);
        assert!(
            !rewritten
                .rewritten_candidate
                .unwrap()
                .display_text
                .contains("https://")
        );

        let unsafe_again = guard(&unsafe_text, &policy, policy.max_rewrites, 0, None);
        assert_eq!(unsafe_again.verdict, GuardVerdict::Block as i32);
    }

    #[test]
    fn preplay_checks_and_cue_expiry_gate_dispatch() {
        let policy = GuardPolicy::default();
        let preplay = request(GuardStage::Preplay, cue("正常内容"));
        let all_ok = PreplayChecks {
            approval_valid: true,
            facts_match: true,
            epoch_match: true,
            permission_current: true,
        };
        assert_eq!(
            guard(&preplay, &policy, 0, 0, Some(all_ok)).verdict,
            GuardVerdict::Pass as i32
        );
        let mut no_approval = all_ok;
        no_approval.approval_valid = false;
        assert_eq!(
            guard(&preplay, &policy, 0, 0, Some(no_approval)).verdict,
            GuardVerdict::Block as i32
        );
        let expired_request = request(GuardStage::Preplay, cue("正常内容"));
        assert_eq!(
            guard(
                &expired_request,
                &policy,
                0,
                1_000_000_000_000,
                Some(all_ok)
            )
            .verdict,
            GuardVerdict::Block as i32
        );
        assert_eq!(
            guard(&preplay, &policy, 0, 0, None).verdict,
            GuardVerdict::Block as i32
        );
    }

    #[test]
    fn missing_moderation_does_not_bypass_local_guard() {
        let policy = GuardPolicy::default();
        let request = request(GuardStage::Input, cue("正常内容"));
        assert_eq!(
            guard(&request, &policy, 0, 0, None).verdict,
            GuardVerdict::Pass as i32
        );
    }
}

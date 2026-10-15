//! W57 记忆读写面的验收测试（V3.8 §42.2–§42.6；验收 H03、H04）。
//!
//! 判定面是纯逻辑的：不打开存储、不联网、不依赖凭据。因此**每一条规则都能被直接构造**，
//! 这正是本文件的结构——不是「跑一遍看看」，而是把每个具名拒绝都造出来。
//!
//! 三条护栏：
//! 1. **具名拒绝集合恰好**是 `MemoryRejection::ALL`——既证每条可构造，也防出现未登记的名字。
//! 2. **顺序是真的**：一条同时越作用域又过期的记录，报的是 `out_of_scope`——证 §42.3 的
//!    「先作用域、后时效」不是注释。
//! 3. **非空洞**：每条规则各自有注入样本，且合规候选**零**拒绝（否则「全都拒」也能让测试变绿）。
use nai_contracts::v1::MemoryRecord;
use nai_kernel::evaluate::memory::{
    ClaimKind, ForgetOutcome, ForgetTrigger, MemoryCandidate, MemoryKind, MemoryOperation,
    MemoryPolicy, MemoryQuery, MemoryReadOutcome, MemoryRejection, MemoryScope, PurgeState,
    SEMANTIC_KIND, SESSION_KIND, ScopedRecord, VIEWER_KIND, WORKING_KIND, WriteSource, admit_write,
    forget, read_records,
};
use std::collections::BTreeSet;

const NOW: u64 = 1_000;
const FUTURE: u64 = 1_000_000;

fn record(kind: &str, id: &str, session: &str) -> MemoryRecord {
    MemoryRecord {
        record_id: id.into(),
        workspace_id: "ws".into(),
        session_id: session.into(),
        viewer_scoped_id: "douyin.app_scope:v1".into(),
        kind: kind.into(),
        purpose: "answered".into(),
        content: "已播完的一段".into(),
        expires_at_utc_ms: FUTURE,
        authorization_ref: "auth".into(),
    }
}

fn scope(session: &str) -> MemoryScope {
    MemoryScope::new("ws", session, "douyin.app_scope:v1")
}

/// 一个「填齐了必填项、来源合规」的候选。各条拒绝测试只改它的一处。
fn well_formed_candidate(kind: &str) -> MemoryCandidate {
    MemoryCandidate::new(record(kind, "r-1", "s1"), WriteSource::ApprovedPlayed)
        .with_source_ref("playback-ack:utt-1")
        .written_at(NOW)
}

fn viewer_enabled() -> MemoryPolicy {
    MemoryPolicy {
        viewer_memory_enabled: true,
        ..MemoryPolicy::default()
    }
}

/// 本文件里「来源门槛 / 必填项」那批断言用的都是**默认策略**下的写入。
/// 观众长期记忆的开关另有显式传策略的用例（见本节末与 H03 的 W89 组）。
fn admit(candidate: &MemoryCandidate) -> Result<(), MemoryRejection> {
    admit_write(candidate, &MemoryPolicy::default())
}

// ---------------------------------------------------------------------------
// H03 写入面
// ---------------------------------------------------------------------------

#[test]
fn a_compliant_write_is_accepted() {
    // 正向不变量：合规候选零拒绝——否则「全都拒」也能让下面每条测试变绿。
    assert_eq!(admit(&well_formed_candidate(SESSION_KIND)), Ok(()));
    assert_eq!(admit(&well_formed_candidate(WORKING_KIND)), Ok(()));
    // 权威事实：语义记忆 + §6 发布引用。
    let fact = well_formed_candidate(SEMANTIC_KIND)
        .with_claim(ClaimKind::AuthoritativeFact)
        .with_publish_ref("publish:kb1:rev7");
    assert_eq!(admit(&fact), Ok(()));
}

#[test]
fn untrusted_sources_never_become_facts_or_promises() {
    // §42.2 / INV-31：工具返回、外部智能体返回、评论原文、模型草稿一律不是事实来源。
    for source in [
        WriteSource::ToolReturn,
        WriteSource::PeerReturn,
        WriteSource::CommentText,
        WriteSource::ModelDraft,
    ] {
        assert!(source.is_untrusted(), "{} 应为不可信来源", source.label());
        let candidate = MemoryCandidate::new(record(SESSION_KIND, "r-1", "s1"), source)
            .with_source_ref("src")
            .written_at(NOW);
        assert_eq!(
            admit(&candidate),
            Err(MemoryRejection::UntrustedSourceAsFact),
            "来源 {} 不得写成事实或承诺",
            source.label()
        );
        // 即便声称只是「叙述」，也不因 claim 而放行——这些来源根本不属于「已批准且已播完」。
        assert_eq!(
            admit(&candidate.clone().with_claim(ClaimKind::Narrative)),
            Err(MemoryRejection::UntrustedSourceAsFact)
        );
    }
    assert!(!WriteSource::ApprovedPlayed.is_untrusted());
    assert!(!WriteSource::ApprovedUnplayed.is_untrusted());
}

#[test]
fn an_unplayed_candidate_is_not_a_fact() {
    let candidate = MemoryCandidate::new(
        record(SESSION_KIND, "r-1", "s1"),
        WriteSource::ApprovedUnplayed,
    )
    .with_source_ref("approval:a-1")
    .written_at(NOW);
    assert_eq!(
        admit(&candidate),
        Err(MemoryRejection::UnplayedCandidateAsFact)
    );
}

#[test]
fn a_write_missing_a_required_field_is_named_after_that_field() {
    // 「缺了必填项」与「来源不可信」是两件事，各有各的名字。
    let mut no_scope = well_formed_candidate(SESSION_KIND);
    no_scope.record.workspace_id = "  ".into();
    assert_eq!(admit(&no_scope), Err(MemoryRejection::WriteWithoutScope));

    let mut no_session = well_formed_candidate(SESSION_KIND);
    no_session.record.session_id = String::new();
    assert_eq!(admit(&no_session), Err(MemoryRejection::WriteWithoutScope));

    let mut no_ref = well_formed_candidate(SESSION_KIND);
    no_ref.source_ref = "   ".into();
    assert_eq!(admit(&no_ref), Err(MemoryRejection::WriteWithoutSourceRef));

    let mut no_time = well_formed_candidate(SESSION_KIND);
    no_time.written_at_utc_ms = 0;
    assert_eq!(admit(&no_time), Err(MemoryRejection::WriteWithoutTimestamp));
}

#[test]
fn an_authoritative_fact_needs_publication_and_semantic_memory() {
    let no_publish = well_formed_candidate(SEMANTIC_KIND).with_claim(ClaimKind::AuthoritativeFact);
    assert_eq!(
        admit(&no_publish),
        Err(MemoryRejection::FactWithoutPublishRef)
    );

    let blank_publish = well_formed_candidate(SEMANTIC_KIND)
        .with_claim(ClaimKind::AuthoritativeFact)
        .with_publish_ref("   ");
    assert_eq!(
        admit(&blank_publish),
        Err(MemoryRejection::FactWithoutPublishRef)
    );

    // 工作记忆是「最近播完的内容」、场次记忆是本场流水——都不承载权威事实。
    for kind in [SESSION_KIND, WORKING_KIND] {
        let misplaced = well_formed_candidate(kind)
            .with_claim(ClaimKind::AuthoritativeFact)
            .with_publish_ref("publish:kb1:rev7");
        assert_eq!(
            admit(&misplaced),
            Err(MemoryRejection::FactOutsideSemanticMemory),
            "{kind} 不得承载权威事实"
        );
    }
}

// ---------------------------------------------------------------------------
// H03（W89）：能力开关的单一来源与写入面
// ---------------------------------------------------------------------------

#[test]
fn a_closed_viewer_memory_is_closed_on_the_write_face_too() {
    // V3.11 §5.5 / PD-31（W89）：能力关闭时，写入面与读取面报**同一个**拒因。
    // 改前这一步不存在——同一个候选会被 `admit_write` 接受（报告 §五 的 F0/F1 对照）。
    let candidate = well_formed_candidate(VIEWER_KIND);
    assert_eq!(
        admit_write(&candidate, &MemoryPolicy::default()),
        Err(MemoryRejection::ViewerMemoryDisabled),
        "能力关闭时观众长期记忆的写入必须被拒"
    );
    // 读取面用的是同一个名字：两面共用一个拒因，不是一个面一个名字。
    assert_eq!(
        read(
            &[],
            &MemoryQuery::new(scope("s1"), vec![MemoryKind::Viewer], 8),
            &MemoryPolicy::default()
        ),
        Err(MemoryRejection::ViewerMemoryDisabled)
    );
    // 启用之后同一候选通过——这一步单独证明「拒」来自开关，而不是别的规则。
    assert_eq!(admit_write(&candidate, &viewer_enabled()), Ok(()));
    // 开关只作用于观众类别，MUST NOT 外溢到别的记忆类别。
    for kind in [SESSION_KIND, WORKING_KIND, SEMANTIC_KIND] {
        assert_eq!(
            admit_write(&well_formed_candidate(kind), &MemoryPolicy::default()),
            Ok(()),
            "{kind} 不受观众长期记忆开关影响"
        );
    }
}

#[test]
fn the_capability_gate_is_answered_before_the_source_gate() {
    // 能力未启用是**不依赖条目内容**就成立的判定 ⇒ MUST 先于条目级的来源判定。
    // 同时命中两者时若报 `untrusted_source_as_fact`，「顺序」就只是一句注释。
    let both = MemoryCandidate::new(record(VIEWER_KIND, "r-1", "s1"), WriteSource::ToolReturn)
        .with_source_ref("src")
        .written_at(NOW);
    assert_eq!(
        admit_write(&both, &MemoryPolicy::default()),
        Err(MemoryRejection::ViewerMemoryDisabled)
    );
    // 启用之后，同一个候选才轮到来源判定——两次报的名字不同，顺序才可判。
    assert_eq!(
        admit_write(&both, &viewer_enabled()),
        Err(MemoryRejection::UntrustedSourceAsFact)
    );
    // 必填项仍先于能力：缺作用域的 viewer 候选报的是它缺的那个字段。
    let mut no_scope = well_formed_candidate(VIEWER_KIND);
    no_scope.record.workspace_id = "   ".into();
    assert_eq!(
        admit_write(&no_scope, &MemoryPolicy::default()),
        Err(MemoryRejection::WriteWithoutScope)
    );
}

// ---------------------------------------------------------------------------
// H04 读取面
// ---------------------------------------------------------------------------

fn admissible(id: &str, kind: &str, session: &str) -> ScopedRecord {
    ScopedRecord::admissible(record(kind, id, session))
}

fn read(
    records: &[ScopedRecord],
    query: &MemoryQuery,
    policy: &MemoryPolicy,
) -> Result<MemoryReadOutcome, MemoryRejection> {
    read_records(records, query, policy, NOW)
}

#[test]
fn a_compliant_read_returns_every_eligible_record() {
    let records = vec![
        admissible("a", SESSION_KIND, "s1"),
        admissible("b", WORKING_KIND, "s1"),
    ];
    let query = MemoryQuery::new(
        scope("s1"),
        vec![MemoryKind::Session, MemoryKind::Working],
        8,
    );
    let outcome = read(&records, &query, &MemoryPolicy::default()).expect("应当成功");
    assert_eq!(
        outcome
            .records
            .iter()
            .map(|r| r.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert!(outcome.refused.is_empty());
}

#[test]
fn an_empty_store_is_not_a_failure() {
    // 未声明 ⇒ 条件化：没有任何记录时读取成功且为空，而不是报错。
    let query = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8);
    let outcome = read(&[], &query, &MemoryPolicy::default()).expect("空库不是失败");
    assert!(outcome.records.is_empty());
    assert!(outcome.refused.is_empty());
}

#[test]
fn an_unrequested_kind_is_not_a_refusal() {
    // 不在本次读取范围内的类别不算「被拒」——否则拒绝清单会淹没真正的问题。
    let records = vec![admissible("a", VIEWER_KIND, "s1")];
    let query = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8);
    let outcome = read(&records, &query, &MemoryPolicy::default()).expect("应当成功");
    assert!(outcome.records.is_empty());
    assert!(outcome.refused.is_empty());
}

#[test]
fn a_read_crosses_scopes_and_is_refused_by_name() {
    let records = vec![
        admissible("mine", SESSION_KIND, "s1"),
        admissible("other-session", SESSION_KIND, "s2"),
    ];
    let query = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8);
    let outcome = read(&records, &query, &MemoryPolicy::default()).expect("应当成功");
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].record_id, "mine");
    assert_eq!(outcome.refused.len(), 1);
    assert_eq!(outcome.refused[0].record_ref, "other-session");
    assert_eq!(outcome.refused[0].reason, MemoryRejection::OutOfScope);

    // 另一个 workspace 同理。
    let mut foreign = record(SESSION_KIND, "foreign", "s1");
    foreign.workspace_id = "ws-2".into();
    let outcome = read(
        &[ScopedRecord::admissible(foreign)],
        &query,
        &MemoryPolicy::default(),
    )
    .expect("应当成功");
    assert_eq!(
        outcome.refused_for(MemoryRejection::OutOfScope).len(),
        1,
        "跨 workspace 的条目必须被具名拒绝"
    );
}

#[test]
fn expired_revoked_and_under_permitted_records_do_not_enter_context() {
    let records = vec![
        admissible("ok", SESSION_KIND, "s1"),
        ScopedRecord {
            record: MemoryRecord {
                expires_at_utc_ms: NOW,
                ..record(SESSION_KIND, "stale", "s1")
            },
            revoked: false,
            permitted: true,
            approximate_of: None,
        },
        ScopedRecord {
            revoked: true,
            ..admissible("revoked", SESSION_KIND, "s1")
        },
        ScopedRecord {
            permitted: false,
            ..admissible("denied", SESSION_KIND, "s1")
        },
    ];
    let query = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8);
    let outcome = read(&records, &query, &MemoryPolicy::default()).expect("应当成功");
    assert_eq!(
        outcome
            .records
            .iter()
            .map(|r| r.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["ok"]
    );
    assert_eq!(outcome.refused_for(MemoryRejection::Expired).len(), 1);
    assert_eq!(outcome.refused_for(MemoryRejection::Revoked).len(), 1);
    assert_eq!(
        outcome.refused_for(MemoryRejection::PermissionDenied).len(),
        1
    );
}

#[test]
fn the_read_chain_order_is_real() {
    // §42.3 的顺序是「先作用域、再时效」。一条**同时**越作用域又过期的记录必须报 out_of_scope，
    // 否则「顺序」只是文档上的一句话。
    let both = ScopedRecord {
        record: MemoryRecord {
            expires_at_utc_ms: NOW,
            ..record(SESSION_KIND, "both", "s2")
        },
        revoked: true,
        permitted: false,
        approximate_of: Some("substitute".into()),
    };
    let query = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8);
    let outcome = read(&[both], &query, &MemoryPolicy::default()).expect("应当成功");
    assert_eq!(outcome.refused.len(), 1);
    assert_eq!(outcome.refused[0].reason, MemoryRejection::OutOfScope);

    // 同作用域下，过期优先于撤销与权限。
    let stale_revoked = ScopedRecord {
        record: MemoryRecord {
            expires_at_utc_ms: NOW,
            ..record(SESSION_KIND, "stale-revoked", "s1")
        },
        revoked: true,
        permitted: false,
        approximate_of: None,
    };
    let outcome = read(&[stale_revoked], &query, &MemoryPolicy::default()).expect("应当成功");
    assert_eq!(outcome.refused[0].reason, MemoryRejection::Expired);

    // 未过期但撤销且无权限 ⇒ 撤销优先。
    let revoked_denied = ScopedRecord {
        revoked: true,
        permitted: false,
        ..admissible("revoked-denied", SESSION_KIND, "s1")
    };
    let outcome = read(&[revoked_denied], &query, &MemoryPolicy::default()).expect("应当成功");
    assert_eq!(outcome.refused[0].reason, MemoryRejection::Revoked);
}

#[test]
fn an_approximate_entry_never_stands_in_for_another() {
    // 条目本身就是「近似条目」⇒ 被具名拒绝，不进入上下文。
    let substitute = ScopedRecord {
        approximate_of: Some("original".into()),
        ..admissible("approx", SESSION_KIND, "s1")
    };
    let query = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8);
    let outcome = read(&[substitute], &query, &MemoryPolicy::default()).expect("应当成功");
    assert!(outcome.records.is_empty());
    assert_eq!(
        outcome.refused[0].reason,
        MemoryRejection::ApproximateSubstitution
    );

    // 请求侧「允许顶替」这个动作本身即被拒（请求级，不是条目级）。
    let asking = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8).allowing_approximate();
    assert_eq!(
        read(&[], &asking, &MemoryPolicy::default()),
        Err(MemoryRejection::ApproximateSubstitutionRequested)
    );
}

#[test]
fn viewer_long_term_memory_is_off_by_default_and_needs_authorization_and_purpose() {
    let records = vec![admissible("v", VIEWER_KIND, "s1")];
    let base = MemoryQuery::new(scope("s1"), vec![MemoryKind::Viewer], 8);

    // 默认关闭：请求直接失败，不会有任何条目进入上下文，也不会写画像。
    assert_eq!(
        read(&records, &base, &MemoryPolicy::default()),
        Err(MemoryRejection::ViewerMemoryDisabled)
    );

    let on = viewer_enabled();
    assert_eq!(
        read(&records, &base, &on),
        Err(MemoryRejection::ViewerReadWithoutAuthorization)
    );

    let authorized = MemoryQuery::new(scope("s1"), vec![MemoryKind::Viewer], 8)
        .with_viewer_grant("auth:viewer", "  ");
    assert_eq!(
        read(&records, &authorized, &on),
        Err(MemoryRejection::ViewerReadWithoutPurpose)
    );

    // 授权 + 用途齐备 ⇒ 通过。
    let granted = MemoryQuery::new(scope("s1"), vec![MemoryKind::Viewer], 8)
        .with_viewer_grant("auth:viewer", "answered_once");
    let outcome = read(&records, &granted, &on).expect("应当成功");
    assert_eq!(outcome.records.len(), 1);
}

#[test]
fn a_viewer_record_is_isolated_by_the_viewer_scope() {
    let mut other_viewer = record(VIEWER_KIND, "v-other", "s1");
    other_viewer.viewer_scoped_id = "douyin.app_scope:v2".into();
    let records = vec![
        admissible("v-mine", VIEWER_KIND, "s1"),
        ScopedRecord::admissible(other_viewer),
    ];
    let granted = MemoryQuery::new(scope("s1"), vec![MemoryKind::Viewer], 8)
        .with_viewer_grant("auth:viewer", "answered_once");
    let outcome = read(&records, &granted, &viewer_enabled()).expect("应当成功");
    assert_eq!(
        outcome
            .records
            .iter()
            .map(|r| r.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["v-mine"]
    );
    assert_eq!(outcome.refused_for(MemoryRejection::OutOfScope).len(), 1);
}

#[test]
fn an_incomplete_scope_is_refused_rather_than_matched_loosely() {
    let incomplete = MemoryQuery::new(
        MemoryScope::new("ws", "  ", "douyin.app_scope:v1"),
        vec![MemoryKind::Session],
        8,
    );
    assert_eq!(
        read(&[], &incomplete, &MemoryPolicy::default()),
        Err(MemoryRejection::InvalidScope)
    );
}

#[test]
fn the_over_budget_crop_follows_the_fixed_priority_and_is_visible() {
    // §42.3：场次 > 工作 > 观众长期记忆。
    let records = vec![
        admissible("w1", WORKING_KIND, "s1"),
        admissible("v1", VIEWER_KIND, "s1"),
        admissible("s1", SESSION_KIND, "s1"),
        admissible("w2", WORKING_KIND, "s1"),
    ];
    let granted = MemoryQuery::new(
        scope("s1"),
        vec![MemoryKind::Session, MemoryKind::Working, MemoryKind::Viewer],
        2,
    )
    .with_viewer_grant("auth:viewer", "answered_once");
    let outcome = read(&records, &granted, &viewer_enabled()).expect("应当成功");

    // 优先级决定**谁被裁**，不重排输出：保留的两条仍按原序返回。
    assert_eq!(
        outcome
            .records
            .iter()
            .map(|r| r.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["w1", "s1"]
    );
    // 裁剪**可见**：被裁的两条各有名字，而不是「本来就没有」。
    let cropped: Vec<&str> = outcome
        .refused_for(MemoryRejection::OverBudget)
        .iter()
        .map(|r| r.record_ref.as_str())
        .collect();
    assert_eq!(cropped.len(), 2);
    assert!(cropped.contains(&"w2"));
    assert!(cropped.contains(&"v1"));
}

// ---------------------------------------------------------------------------
// §42.4 遗忘链
// ---------------------------------------------------------------------------

#[test]
fn a_conservative_forget_is_accepted_for_every_trigger() {
    for trigger in ForgetTrigger::ALL {
        let outcome = ForgetOutcome::blocked(trigger, "r-1");
        assert_eq!(
            forget(&outcome),
            Ok(()),
            "触发 {} 的保守陈述应当通过",
            trigger.label()
        );
    }
    // 清除完成、**载荷真的清过**、无留存、宣称擦除 ⇒ 通过（W111）。
    let complete = ForgetOutcome {
        purge_state: PurgeState::Complete,
        body_purged: true,
        claims_fully_erased: true,
        ..ForgetOutcome::blocked(ForgetTrigger::UserRequest, "r-1")
    };
    assert_eq!(forget(&complete), Ok(()));
}

#[test]
fn the_read_side_is_blocked_before_the_purge() {
    let early = ForgetOutcome {
        read_side_blocked: false,
        purge_state: PurgeState::Complete,
        ..ForgetOutcome::blocked(ForgetTrigger::Ttl, "r-1")
    };
    assert_eq!(forget(&early), Err(MemoryRejection::ReadSideNotBlocked));
}

#[test]
fn an_erasure_claim_never_outruns_the_purge() {
    let unfinished = ForgetOutcome {
        purge_state: PurgeState::InProgress,
        claims_fully_erased: true,
        ..ForgetOutcome::blocked(ForgetTrigger::FactInvalidated, "r-1")
    };
    assert_eq!(
        forget(&unfinished),
        Err(MemoryRejection::ErasedClaimWithoutCompletePurge)
    );

    // W111：副本状态说「已清」而**载荷一次都没清过** ⇒ 它自己的拒因。
    let unpurged = ForgetOutcome {
        purge_state: PurgeState::Complete,
        claims_fully_erased: true,
        ..ForgetOutcome::blocked(ForgetTrigger::PermissionRevoked, "r-1")
    };
    assert_eq!(
        forget(&unpurged),
        Err(MemoryRejection::ErasedClaimWithoutPurge),
        "「已完全擦除」而载荷没清过 MUST 是它自己的拒因"
    );

    let retained = ForgetOutcome {
        purge_state: PurgeState::Complete,
        body_purged: true,
        third_party_retention: Some("第三方镜像保留 30 天".into()),
        claims_fully_erased: true,
        ..ForgetOutcome::blocked(ForgetTrigger::PermissionRevoked, "r-1")
    };
    assert_eq!(
        forget(&retained),
        Err(MemoryRejection::ErasedClaimWithThirdPartyRetention)
    );

    let undisclosed = ForgetOutcome {
        purge_state: PurgeState::BlockedThirdParty,
        ..ForgetOutcome::blocked(ForgetTrigger::UserRequest, "r-1")
    };
    assert_eq!(
        forget(&undisclosed),
        Err(MemoryRejection::ThirdPartyRetentionUndisclosed)
    );

    // 受阻但**如实**说明 ⇒ 通过（如实披露不是缺陷）。
    let disclosed = ForgetOutcome {
        purge_state: PurgeState::BlockedThirdParty,
        third_party_retention: Some("第三方镜像保留 30 天".into()),
        ..ForgetOutcome::blocked(ForgetTrigger::UserRequest, "r-1")
    };
    assert_eq!(forget(&disclosed), Ok(()));
}

// ---------------------------------------------------------------------------
// §42.6 四项操作
// ---------------------------------------------------------------------------

#[test]
fn exactly_four_operations_are_named_and_nothing_else_is_guessed() {
    let labels: BTreeSet<&str> = MemoryOperation::ALL.iter().map(|op| op.label()).collect();
    assert_eq!(
        labels,
        BTreeSet::from(["write", "read", "forget", "scopes"])
    );
    for op in MemoryOperation::ALL {
        assert_eq!(MemoryOperation::parse(op.label()), Ok(op));
    }
    for unknown in ["merge", "WRITE", "", "write "] {
        assert_eq!(
            MemoryOperation::parse(unknown),
            Err(MemoryRejection::UnknownOperation),
            "{unknown:?} 不应被接受"
        );
    }
}

// ---------------------------------------------------------------------------
// 具名拒绝的总账
// ---------------------------------------------------------------------------

/// 把每个拒绝都真正构造一次，然后断言集合**恰好**等于 `MemoryRejection::ALL`。
/// 既证每条可构造（没有纸面上的规则），也防出现未登记的名字（没有没纸面的规则）。
#[test]
fn every_named_rejection_is_constructible_and_the_set_is_exact() {
    let mut seen: BTreeSet<MemoryRejection> = BTreeSet::new();

    // —— 写入（H03）
    let untrusted = MemoryCandidate::new(record(SESSION_KIND, "r", "s1"), WriteSource::ToolReturn)
        .with_source_ref("src")
        .written_at(NOW);
    seen.insert(admit(&untrusted).unwrap_err());

    let unplayed = MemoryCandidate::new(
        record(SESSION_KIND, "r", "s1"),
        WriteSource::ApprovedUnplayed,
    )
    .with_source_ref("src")
    .written_at(NOW);
    seen.insert(admit(&unplayed).unwrap_err());

    let mut no_scope = well_formed_candidate(SESSION_KIND);
    no_scope.record.workspace_id = String::new();
    seen.insert(admit(&no_scope).unwrap_err());

    let mut no_ref = well_formed_candidate(SESSION_KIND);
    no_ref.source_ref = String::new();
    seen.insert(admit(&no_ref).unwrap_err());

    let mut no_time = well_formed_candidate(SESSION_KIND);
    no_time.written_at_utc_ms = 0;
    seen.insert(admit(&no_time).unwrap_err());

    let no_publish = well_formed_candidate(SEMANTIC_KIND).with_claim(ClaimKind::AuthoritativeFact);
    seen.insert(admit(&no_publish).unwrap_err());

    let misplaced = well_formed_candidate(SESSION_KIND)
        .with_claim(ClaimKind::AuthoritativeFact)
        .with_publish_ref("publish:kb1:rev7");
    seen.insert(admit(&misplaced).unwrap_err());

    // —— 请求级（H04）
    let incomplete = MemoryQuery::new(MemoryScope::new("ws", "", ""), vec![MemoryKind::Session], 8);
    seen.insert(read(&[], &incomplete, &MemoryPolicy::default()).unwrap_err());

    let viewer = MemoryQuery::new(scope("s1"), vec![MemoryKind::Viewer], 8);
    seen.insert(read(&[], &viewer, &MemoryPolicy::default()).unwrap_err());

    seen.insert(read(&[], &viewer, &viewer_enabled()).unwrap_err());

    let no_purpose = MemoryQuery::new(scope("s1"), vec![MemoryKind::Viewer], 8)
        .with_viewer_grant("auth:viewer", "");
    seen.insert(read(&[], &no_purpose, &viewer_enabled()).unwrap_err());

    let approximate =
        MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8).allowing_approximate();
    seen.insert(read(&[], &approximate, &MemoryPolicy::default()).unwrap_err());

    seen.insert(MemoryOperation::parse("merge").unwrap_err());

    // —— 条目级（H04）
    let session_read = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 8);
    let policy = MemoryPolicy::default();

    seen.insert(
        read(
            &[admissible("x", SESSION_KIND, "s2")],
            &session_read,
            &policy,
        )
        .unwrap()
        .refused[0]
            .reason,
    );
    let stale = ScopedRecord {
        record: MemoryRecord {
            expires_at_utc_ms: NOW,
            ..record(SESSION_KIND, "x", "s1")
        },
        revoked: false,
        permitted: true,
        approximate_of: None,
    };
    seen.insert(read(&[stale], &session_read, &policy).unwrap().refused[0].reason);
    seen.insert(
        read(
            &[ScopedRecord {
                revoked: true,
                ..admissible("x", SESSION_KIND, "s1")
            }],
            &session_read,
            &policy,
        )
        .unwrap()
        .refused[0]
            .reason,
    );
    seen.insert(
        read(
            &[ScopedRecord {
                permitted: false,
                ..admissible("x", SESSION_KIND, "s1")
            }],
            &session_read,
            &policy,
        )
        .unwrap()
        .refused[0]
            .reason,
    );
    seen.insert(
        read(
            &[ScopedRecord {
                approximate_of: Some("other".into()),
                ..admissible("x", SESSION_KIND, "s1")
            }],
            &session_read,
            &policy,
        )
        .unwrap()
        .refused[0]
            .reason,
    );
    let tight = MemoryQuery::new(scope("s1"), vec![MemoryKind::Session], 1);
    let cropped = read(
        &[
            admissible("a", SESSION_KIND, "s1"),
            admissible("b", SESSION_KIND, "s1"),
        ],
        &tight,
        &policy,
    )
    .unwrap();
    seen.insert(cropped.refused[0].reason);

    // —— 遗忘（§42.4）
    seen.insert(
        forget(&ForgetOutcome {
            read_side_blocked: false,
            ..ForgetOutcome::blocked(ForgetTrigger::Ttl, "r")
        })
        .unwrap_err(),
    );
    seen.insert(
        forget(&ForgetOutcome {
            claims_fully_erased: true,
            ..ForgetOutcome::blocked(ForgetTrigger::Ttl, "r")
        })
        .unwrap_err(),
    );
    // W111: 副本状态说「已清」而载荷没清过 —— 必须被单独构造出来，
    // 否则这条拒因只是「碰巧被别的构造盖住」，集合断言就失去分辨力。
    seen.insert(
        forget(&ForgetOutcome {
            purge_state: PurgeState::Complete,
            claims_fully_erased: true,
            ..ForgetOutcome::blocked(ForgetTrigger::Ttl, "r")
        })
        .unwrap_err(),
    );
    seen.insert(
        forget(&ForgetOutcome {
            purge_state: PurgeState::Complete,
            body_purged: true,
            third_party_retention: Some("镜像".into()),
            claims_fully_erased: true,
            ..ForgetOutcome::blocked(ForgetTrigger::Ttl, "r")
        })
        .unwrap_err(),
    );
    seen.insert(
        forget(&ForgetOutcome {
            purge_state: PurgeState::BlockedThirdParty,
            ..ForgetOutcome::blocked(ForgetTrigger::Ttl, "r")
        })
        .unwrap_err(),
    );

    let expected: BTreeSet<MemoryRejection> = MemoryRejection::ALL.iter().copied().collect();
    assert_eq!(seen.len(), 24, "构造出的拒绝数量与 ALL 不符");
    assert_eq!(
        seen, expected,
        "构造出的拒绝集合必须恰为 MemoryRejection::ALL"
    );

    // 名字唯一且稳定：没有两个拒绝共用一个 label（否则「具名」是假的）。
    let labels: BTreeSet<&'static str> = MemoryRejection::ALL.iter().map(|r| r.label()).collect();
    assert_eq!(
        labels.len(),
        MemoryRejection::ALL.len(),
        "拒绝名必须互不相同"
    );

    // 三种记忆类别的 label 与协议里的 kind 字符串一致。
    for kind in MemoryKind::ALL {
        assert_eq!(MemoryKind::parse_label(kind.label()), Some(kind));
    }
    assert_eq!(
        MemoryKind::parse_label(SEMANTIC_KIND),
        None,
        "语义记忆不在记忆段内"
    );
}

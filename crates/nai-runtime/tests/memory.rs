//! W107 lands the H03 half of §42.2's runtime wiring: a memory candidate that passes the write
//! gate lands in the **real** `memory_records` table, a refused one lands nothing, and the read
//! back is a per-column fetch rather than the §42.3 read judgement.
//!
//! Before this round both halves existed and were joined by nothing: `admit_write` had no
//! production caller, and no production path touched `memory_records`. The guard below is the
//! non-void half of that claim — `refusal_sample` matches `MemoryRejection` **exhaustively**, so a
//! new write-face rejection in the kernel fails to compile here instead of passing invisibly.
//!
//! W109 lands the other half: the §42.3 **read** face and the §42.4 **forget** block point. The
//! read face needed a carrier first — `memory_records` had no revocation column, so `revoked` had
//! nothing to be derived *from*. Two guards below are the non-void half:
//!
//! * every rejection is classified (`cover`), so a new variant without a decision fails to compile;
//! * the read/forget face rejections are **observed end to end**, so "classified as reachable" is
//!   not confused with "actually produced by the runtime".
mod common;
use common::*;
use nai_contracts::v1::{ErrorCode, MemoryRecord};
use nai_kernel::evaluate::memory::{
    ClaimKind, ForgetTrigger, MemoryCandidate, MemoryKind, MemoryPolicy, MemoryQuery,
    MemoryReadOutcome, MemoryRejection, MemoryScope, PurgeState, SEMANTIC_KIND, SESSION_KIND,
    VIEWER_KIND, WriteSource,
};
use nai_runtime::{
    Composition, MemoryAdapter, MemoryForgetError, MemoryReadError, MemoryWriteOutcome,
};
use nai_storage::{DataPaths, DecisionEntryView};
use std::time::Duration;
use tempfile::tempdir;

const SESSION: &str = "session-1";
const OTHER_SESSION: &str = "session-2";
const SOURCE_REF: &str = "evidence-kb0";
const TIMESTAMP: u64 = 1_700_000_000_000;
/// Far enough out that no test races the clock.
const EXPIRES: u64 = 4_000_000_000_000;
const BODY: &str = "the record body that MUST be persisted verbatim, not a pointer";
/// §42.3: 观众长期记忆按**观众作用域**隔离（不是按会话）。故查询里的观众作用域与
/// 记录自己的 `viewer_scoped_id` MUST 逐字相同，否则每条观众记录都会先被判越作用域，
/// 后面几层（时效/撤销/权限）根本没有对象可判。
const VIEWER_SCOPE: &str = "viewer-scope-7";
/// 一条**故意**不在查询作用域里的观众记录：用来证明「越作用域」是最先被报的那一层。
const FOREIGN_VIEWER_SCOPE: &str = "viewer-scope-9";

/// The eight rules that can refuse a **write**. Listed in `MemoryRejection::ALL` order so the
/// guard's collected sequence can be compared against it directly.
const WRITE_FACE: [MemoryRejection; 8] = [
    MemoryRejection::UntrustedSourceAsFact,
    MemoryRejection::UnplayedCandidateAsFact,
    MemoryRejection::WriteWithoutScope,
    MemoryRejection::WriteWithoutSourceRef,
    MemoryRejection::WriteWithoutTimestamp,
    MemoryRejection::FactWithoutPublishRef,
    MemoryRejection::FactOutsideSemanticMemory,
    MemoryRejection::ViewerMemoryDisabled,
];

fn prepared(root: &std::path::Path) -> Composition {
    provision(root, WORKSPACE, ACTOR, PROFILE, &runtime_profile(WORKSPACE));
    let composition = Composition::prepare(
        DataPaths::at(root).unwrap(),
        WORKSPACE,
        ACTOR,
        PROFILE,
        false,
    )
    .unwrap();
    // `memory_records.session_id` has a real foreign key onto `sessions`, so the record's scope
    // must be a session that exists. That is a storage-layer fact, not a §42.2 rule.
    open_session(&composition, SESSION);
    composition
}

fn open_session(composition: &Composition, session: &str) {
    let operation = store_operation();
    composition
        .store()
        .lock()
        .unwrap()
        .create_session(&operation, WORKSPACE, ACTOR, session)
        .unwrap();
}

/// The nine stored columns, from the perspective of the record the gate judged. `viewer_scope` is
/// only filled for a viewer record, matching the table's nullable column.
fn record(record_id: &str, kind: &str) -> MemoryRecord {
    MemoryRecord {
        record_id: record_id.into(),
        workspace_id: WORKSPACE.into(),
        session_id: SESSION.into(),
        viewer_scoped_id: if kind == VIEWER_KIND {
            VIEWER_SCOPE
        } else {
            ""
        }
        .into(),
        kind: kind.into(),
        purpose: "order_status".into(),
        content: BODY.into(),
        expires_at_utc_ms: EXPIRES,
        authorization_ref: "authorization-ref-1".into(),
    }
}

/// A candidate §42.2 lets through: session memory from **approved and played** content, carrying a
/// scope, a source reference and a timestamp.
fn admissible(record_id: &str, kind: &str) -> MemoryCandidate {
    MemoryCandidate::new(record(record_id, kind), WriteSource::ApprovedPlayed)
        .with_source_ref(SOURCE_REF)
        .written_at(TIMESTAMP)
}

/// A candidate that provokes `rejection` on the write face, or `None` when the variant is not a
/// write-face rule.
///
/// Matching `MemoryRejection` **exhaustively** is the point of this function: adding a variant to
/// the kernel without deciding where it is provoked here fails to compile, so the guard cannot
/// silently lose coverage.
fn refusal_sample(rejection: MemoryRejection) -> Option<MemoryCandidate> {
    Some(match rejection {
        MemoryRejection::WriteWithoutScope => {
            let mut candidate = admissible("memory-without-scope", SESSION_KIND);
            candidate.record.session_id = String::new();
            candidate
        }
        MemoryRejection::WriteWithoutSourceRef => MemoryCandidate::new(
            record("memory-without-source-ref", SESSION_KIND),
            WriteSource::ApprovedPlayed,
        )
        .written_at(TIMESTAMP),
        MemoryRejection::WriteWithoutTimestamp => MemoryCandidate::new(
            record("memory-without-timestamp", SESSION_KIND),
            WriteSource::ApprovedPlayed,
        )
        .with_source_ref(SOURCE_REF),
        MemoryRejection::ViewerMemoryDisabled => admissible("memory-viewer-disabled", VIEWER_KIND),
        MemoryRejection::UntrustedSourceAsFact => {
            let mut candidate = admissible("memory-untrusted-source", SESSION_KIND);
            candidate.source = WriteSource::ToolReturn;
            candidate
        }
        MemoryRejection::UnplayedCandidateAsFact => {
            let mut candidate = admissible("memory-unplayed-candidate", SESSION_KIND);
            candidate.source = WriteSource::ApprovedUnplayed;
            candidate
        }
        MemoryRejection::FactOutsideSemanticMemory => {
            let mut candidate = admissible("memory-fact-outside-semantic", SESSION_KIND);
            candidate.claim = ClaimKind::AuthoritativeFact;
            candidate.with_publish_ref("publish-ref-1")
        }
        MemoryRejection::FactWithoutPublishRef => {
            let mut candidate = admissible("memory-fact-without-ref", SEMANTIC_KIND);
            candidate.claim = ClaimKind::AuthoritativeFact;
            candidate
        }
        // The read face (§42.3), the forget chain (§42.4) and the operation face (§42.6) are not
        // provoked by a write: they are refused on their own faces.
        MemoryRejection::InvalidScope
        | MemoryRejection::ViewerReadWithoutAuthorization
        | MemoryRejection::ViewerReadWithoutPurpose
        | MemoryRejection::ApproximateSubstitutionRequested
        | MemoryRejection::UnknownOperation
        | MemoryRejection::OutOfScope
        | MemoryRejection::Expired
        | MemoryRejection::Revoked
        | MemoryRejection::PermissionDenied
        | MemoryRejection::ApproximateSubstitution
        | MemoryRejection::OverBudget
        | MemoryRejection::ReadSideNotBlocked
        | MemoryRejection::ErasedClaimWithoutCompletePurge
        | MemoryRejection::ErasedClaimWithThirdPartyRetention
        | MemoryRejection::ThirdPartyRetentionUndisclosed => return None,
    })
}

/// The rows actually in `memory_records` for one session, read through the storage accessor.
fn stored(composition: &Composition, session: &str) -> Vec<MemoryRecord> {
    let operation = store_operation();
    composition
        .store()
        .lock()
        .unwrap()
        .memory_records(&operation, WORKSPACE, ACTOR, session)
        .unwrap()
}

fn logged(composition: &Composition) -> Vec<DecisionEntryView> {
    let operation = store_operation();
    composition
        .store()
        .lock()
        .unwrap()
        .decision_entries(&operation, composition.workspace(), 500)
        .unwrap()
}

fn write(composition: &mut Composition, candidate: &MemoryCandidate) -> MemoryWriteOutcome {
    composition
        .write_memory(candidate, &MemoryPolicy::default())
        .unwrap()
}

// ---------------------------------------------------------------------------
// §42.2 写入面：放行则落库，拒绝则具名且零落库
// ---------------------------------------------------------------------------

#[test]
fn an_admitted_write_lands_exactly_one_row() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let candidate = admissible("memory-admitted", SESSION_KIND);

    match write(&mut composition, &candidate) {
        MemoryWriteOutcome::Written(record) => assert_eq!(record, candidate.record),
        MemoryWriteOutcome::Refused(refusal) => panic!("admitted candidate refused: {refusal:?}"),
    }

    assert_eq!(stored(&composition, SESSION), vec![candidate.record]);
}

#[test]
fn a_refused_write_is_named_and_lands_nothing() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let mut candidate = admissible("memory-refused", SESSION_KIND);
    candidate.source = WriteSource::ModelDraft;

    match write(&mut composition, &candidate) {
        MemoryWriteOutcome::Refused(refusal) => {
            assert_eq!(
                refusal.reason,
                MemoryRejection::UntrustedSourceAsFact.label()
            );
            assert_eq!(refusal.record_id, "memory-refused");
        }
        MemoryWriteOutcome::Written(_) => panic!("a model draft became a stored memory"),
    }

    assert!(
        stored(&composition, SESSION).is_empty(),
        "a refused write MUST leave the store empty"
    );
}

#[test]
fn every_write_rejection_is_reachable_through_the_runtime() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());

    let mut reached = Vec::new();
    for rejection in MemoryRejection::ALL {
        let Some(candidate) = refusal_sample(rejection) else {
            continue;
        };
        match write(&mut composition, &candidate) {
            MemoryWriteOutcome::Refused(refusal) => {
                assert_eq!(
                    refusal.reason,
                    rejection.label(),
                    "{rejection:?} was refused under a different name"
                );
                assert_eq!(refusal.record_id, candidate.record.record_id);
                reached.push(rejection);
            }
            MemoryWriteOutcome::Written(_) => panic!("{rejection:?} was admitted"),
        }
    }

    assert_eq!(
        reached, WRITE_FACE,
        "the write face MUST be exactly these eight rules, and each MUST be reachable"
    );
    assert!(
        stored(&composition, SESSION).is_empty(),
        "not one of these refusals may land a row"
    );
}

#[test]
fn the_viewer_switch_closes_the_write_face_too() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let candidate = admissible("memory-viewer", VIEWER_KIND);

    // The default policy has viewer long-term memory **off**.
    let closed = composition
        .write_memory(&candidate, &MemoryPolicy::default())
        .unwrap();
    match closed {
        MemoryWriteOutcome::Refused(refusal) => assert_eq!(
            refusal.reason,
            MemoryRejection::ViewerMemoryDisabled.label()
        ),
        MemoryWriteOutcome::Written(_) => panic!("a closed switch let a viewer write through"),
    }
    assert!(stored(&composition, SESSION).is_empty());

    // The same candidate with the switch open *does* land — so the refusal above was the switch,
    // not an unrelated defect in the candidate.
    let open = MemoryPolicy {
        viewer_memory_enabled: true,
        ..MemoryPolicy::default()
    };
    let outcome = composition.write_memory(&candidate, &open).unwrap();
    assert!(matches!(outcome, MemoryWriteOutcome::Written(_)));
    assert_eq!(stored(&composition, SESSION), vec![candidate.record]);
}

// ---------------------------------------------------------------------------
// 落库的列 = 被判定审过的输入
// ---------------------------------------------------------------------------

#[test]
fn the_stored_row_round_trips_column_by_column() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    // A viewer record: the one case where `viewer_scoped_id` → `viewer_scope` carries a value
    // rather than the empty string, so a mapping that was dropped instead of copied is visible.
    let candidate = admissible("memory-viewer-roundtrip", VIEWER_KIND);
    let open = MemoryPolicy {
        viewer_memory_enabled: true,
        ..MemoryPolicy::default()
    };
    composition.write_memory(&candidate, &open).unwrap();

    let rows = stored(&composition, SESSION);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.record_id, "memory-viewer-roundtrip");
    assert_eq!(row.workspace_id, WORKSPACE);
    assert_eq!(row.session_id, SESSION);
    assert_eq!(row.viewer_scoped_id, VIEWER_SCOPE);
    assert_eq!(row.kind, VIEWER_KIND);
    assert_eq!(row.purpose, "order_status");
    assert_eq!(row.authorization_ref, "authorization-ref-1");
    assert_eq!(row.content, BODY);
    assert_eq!(row.expires_at_utc_ms, EXPIRES);
    assert_eq!(
        row, &candidate.record,
        "every column MUST be the judged input, verbatim"
    );
}

#[test]
fn a_memory_write_reaches_the_memory_store_and_not_the_decision_log() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    assert!(logged(&composition).is_empty());

    write(
        &mut composition,
        &admissible("memory-store-only", SESSION_KIND),
    );

    assert_eq!(stored(&composition, SESSION).len(), 1);
    assert!(
        logged(&composition).is_empty(),
        "§42.2's execution body is the memory table; a memory *observation* reaching the chain is \
         the harness surface's job (W103), and a second path here would be a second definition"
    );
}

#[test]
fn a_duplicate_record_id_is_a_named_conflict_and_adds_no_second_row() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let candidate = admissible("memory-duplicate", SESSION_KIND);
    write(&mut composition, &candidate);

    let error = composition
        .write_memory(&candidate, &MemoryPolicy::default())
        .expect_err("the same record id MUST NOT be written twice");
    assert_eq!(error.code, ErrorCode::Conflict);
    assert_eq!(stored(&composition, SESSION).len(), 1);
}

// ---------------------------------------------------------------------------
// 读回访问器是取数入口，不是 §42.3 的读取面
// ---------------------------------------------------------------------------

#[test]
fn the_read_back_does_not_cross_scopes() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    open_session(&composition, OTHER_SESSION);

    write(
        &mut composition,
        &admissible("memory-this-session", SESSION_KIND),
    );
    let mut elsewhere = admissible("memory-other-session", SESSION_KIND);
    elsewhere.record.session_id = OTHER_SESSION.into();
    write(&mut composition, &elsewhere);

    let rows = stored(&composition, SESSION);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].record_id, "memory-this-session");
}

#[test]
fn the_read_back_applies_no_read_judgement() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());

    // A record whose retention has already lapsed. §42.2 has no freshness rule, so the gate admits
    // it and it lands; the accessor must still hand it back, because expiry is §42.3's *read*-face
    // judgement and this entry point is a fetch. A fetch that filtered here would be answering a
    // question nobody asked it, under a rule the read face already owns.
    let mut candidate = admissible("memory-expired", SESSION_KIND);
    candidate.record.expires_at_utc_ms = 1;
    write(&mut composition, &candidate);

    assert_eq!(stored(&composition, SESSION), vec![candidate.record]);
}

// ---------------------------------------------------------------------------
// §42.3 读取面 / §42.4 遗忘落点（W109）
// ---------------------------------------------------------------------------

fn ids(records: &[MemoryRecord]) -> Vec<&str> {
    records
        .iter()
        .map(|record| record.record_id.as_str())
        .collect()
}

/// §42.3: 一次只请求场次记忆的读取。
fn session_query(session: &str, budget: usize) -> MemoryQuery {
    MemoryQuery::new(
        MemoryScope::new(WORKSPACE, session, ""),
        vec![MemoryKind::Session],
        budget,
    )
}

/// §42.3: 一次**请求级**合法的观众记忆读取——开关开、有授权引用、有用途记录。
/// 三项缺任一都是请求级拒绝：整个读取都不成立，一条记录都没被考虑。
fn viewer_query(budget: usize) -> MemoryQuery {
    MemoryQuery::new(
        MemoryScope::new(WORKSPACE, SESSION, VIEWER_SCOPE),
        vec![MemoryKind::Viewer],
        budget,
    )
    .with_viewer_grant("authorization-ref-1", "order_status")
}

/// 一次同时请求场次与观众记忆的读取——条目级五条拒因要**同一次**读取里全部出现。
fn all_kinds_query(budget: usize) -> MemoryQuery {
    MemoryQuery::new(
        MemoryScope::new(WORKSPACE, SESSION, VIEWER_SCOPE),
        vec![MemoryKind::Session, MemoryKind::Viewer],
        budget,
    )
    .with_viewer_grant("authorization-ref-1", "order_status")
}

/// §5.5 / PD-31: 观众长期记忆**开**。写入面与读取面都必须显式打开，默认是关的。
fn open_policy() -> MemoryPolicy {
    MemoryPolicy {
        viewer_memory_enabled: true,
        ..MemoryPolicy::default()
    }
}

fn read(
    composition: &mut Composition,
    query: &MemoryQuery,
    policy: &MemoryPolicy,
) -> MemoryReadOutcome {
    composition
        .read_memory(query, policy, TIMESTAMP)
        .expect("这一路不该有故障；请求级拒绝走 observe_request")
}

/// §42.4: 阻断一条记录的读取——**最保守**的陈述（只阻断、尚未清理、无第三方留存、
/// 不宣称已擦除）。返回生效中的撤销时刻。
fn block(composition: &mut Composition, record_id: &str) -> i64 {
    composition
        .forget_memory(
            ForgetTrigger::UserRequest,
            record_id,
            PurgeState::NotStarted,
            None,
            false,
        )
        .expect("最保守的陈述 MUST 被接受")
}

/// §42.3: 请求级拒绝 MUST 以 `Rejected` 返回，**MUST NOT** 折成一次「空读取」——
/// 否则「这次读取本身不成立」与「读出来零条」将无法区分（后者是被允许的）。
fn observe_request(
    composition: &mut Composition,
    query: &MemoryQuery,
    policy: &MemoryPolicy,
    expected: MemoryRejection,
    observed: &mut Vec<MemoryRejection>,
) {
    match composition.read_memory(query, policy, TIMESTAMP) {
        Err(MemoryReadError::Rejected(rejection)) => {
            assert_eq!(rejection, expected);
            observed.push(rejection);
        }
        other => panic!("{expected:?} MUST 是请求级拒绝，实际是 {other:?}"),
    }
}

/// §42.4: 陈述不实 MUST 以 `Rejected` 返回——那是**判定**的结论，不是故障。
fn observe_forget(
    composition: &mut Composition,
    record_id: &str,
    purge_state: PurgeState,
    third_party_retention: Option<&str>,
    claims_fully_erased: bool,
    expected: MemoryRejection,
    observed: &mut Vec<MemoryRejection>,
) {
    match composition.forget_memory(
        ForgetTrigger::UserRequest,
        record_id,
        purge_state,
        third_party_retention.map(str::to_owned),
        claims_fully_erased,
    ) {
        Err(MemoryForgetError::Rejected(rejection)) => {
            assert_eq!(rejection, expected);
            observed.push(rejection);
        }
        other => panic!("{expected:?} MUST 是陈述不实的拒绝，实际是 {other:?}"),
    }
}

#[test]
fn a_record_outside_the_scope_is_refused_by_name_and_not_filtered_away() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    open_session(&composition, OTHER_SESSION);
    write(
        &mut composition,
        &admissible("memory-in-scope", SESSION_KIND),
    );
    let mut elsewhere = admissible("memory-elsewhere", SESSION_KIND);
    elsewhere.record.session_id = OTHER_SESSION.into();
    write(&mut composition, &elsewhere);

    let outcome = read(
        &mut composition,
        &session_query(SESSION, 8),
        &MemoryPolicy::default(),
    );

    assert_eq!(ids(&outcome.records), vec!["memory-in-scope"]);
    let out = outcome.refused_for(MemoryRejection::OutOfScope);
    assert_eq!(
        out.len(),
        1,
        "越作用域 MUST 被具名拒绝。若取数阶段就按会话把别的作用域先筛掉，这里会是 0：\
         那条守卫还在，但它守的对象已被上游拿走（假阴性，最危险的方向）"
    );
    assert_eq!(out[0].record_ref, "memory-elsewhere");
    // 这条记录确实在库里：它不是「没写进去」，而是「被判越作用域」。
    assert_eq!(stored(&composition, OTHER_SESSION).len(), 1);
}

#[test]
fn an_expired_record_is_refused_by_the_read_face_not_by_the_write_gate() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let mut candidate = admissible("memory-lapsed", SESSION_KIND);
    candidate.record.expires_at_utc_ms = 1;
    write(&mut composition, &candidate);

    let outcome = read(
        &mut composition,
        &session_query(SESSION, 8),
        &MemoryPolicy::default(),
    );

    assert!(outcome.records.is_empty());
    let expired = outcome.refused_for(MemoryRejection::Expired);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].record_ref, "memory-lapsed");
    assert_eq!(
        stored(&composition, SESSION).len(),
        1,
        "§42.2 没有时效规则 ⇒ 它确实落了库；判掉它的是读取面"
    );
}

#[test]
fn blocking_a_record_makes_the_next_read_refuse_it_as_revoked() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    write(
        &mut composition,
        &admissible("memory-revoked", SESSION_KIND),
    );

    let before = read(
        &mut composition,
        &session_query(SESSION, 8),
        &MemoryPolicy::default(),
    );
    assert_eq!(ids(&before.records), vec!["memory-revoked"]);

    let revoked_at = block(&mut composition, "memory-revoked");
    assert!(revoked_at > 0, "阻断 MUST 留下一个撤销时刻");

    let after = read(
        &mut composition,
        &session_query(SESSION, 8),
        &MemoryPolicy::default(),
    );
    assert!(after.records.is_empty());
    let revoked = after.refused_for(MemoryRejection::Revoked);
    assert_eq!(revoked.len(), 1);
    assert_eq!(revoked[0].record_ref, "memory-revoked");
    assert_eq!(
        stored(&composition, SESSION).len(),
        1,
        "阻断 ≠ 清除：行还在，只是撤销列有了时刻（§42.4「先阻断，再异步清除副本」）"
    );
}

#[test]
fn a_second_block_does_not_rewrite_the_revocation_moment() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    write(
        &mut composition,
        &admissible("memory-revoked-twice", SESSION_KIND),
    );

    let first = block(&mut composition, "memory-revoked-twice");
    // 睡过一个毫秒：否则「覆盖写」的注入可能与原值**碰巧相等**而绿着通过
    // （假阴性）。这条等待是「注入可见」的一部分，不是装饰。
    std::thread::sleep(Duration::from_millis(20));
    let second = block(&mut composition, "memory-revoked-twice");

    assert_eq!(
        second, first,
        "撤销时刻是一条事实：重复阻断 MUST 返回原有时刻，MUST NOT 覆盖它"
    );
}

#[test]
fn a_viewer_record_without_an_authorization_ref_is_refused_on_the_read_face() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let mut candidate = admissible("memory-unauthorized", VIEWER_KIND);
    candidate.record.authorization_ref = String::new();
    // §42.2 不要求记录级授权引用 ⇒ 它能入库；§42.3 要求观众记忆读取有授权 ⇒ 它读不出来。
    match composition
        .write_memory(&candidate, &open_policy())
        .unwrap()
    {
        MemoryWriteOutcome::Written(_) => {}
        MemoryWriteOutcome::Refused(refusal) => panic!("该候选不该被写入面拒绝：{refusal:?}"),
    }

    let outcome = read(&mut composition, &viewer_query(8), &open_policy());

    assert!(outcome.records.is_empty());
    let denied = outcome.refused_for(MemoryRejection::PermissionDenied);
    assert_eq!(
        denied.len(),
        1,
        "PermissionDenied MUST 可达。若取数入口已要求同一条 session.manage 授权、\
         条目级又只查这一条，二者恒一致 ⇒ 守卫在却无对象可守（明令禁止的形状）"
    );
    assert_eq!(denied[0].record_ref, "memory-unauthorized");
}

#[test]
fn a_budget_crop_is_visible_by_name() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    write(
        &mut composition,
        &admissible("memory-budget-a", SESSION_KIND),
    );
    write(
        &mut composition,
        &admissible("memory-budget-b", SESSION_KIND),
    );

    let outcome = read(
        &mut composition,
        &session_query(SESSION, 1),
        &MemoryPolicy::default(),
    );

    assert_eq!(ids(&outcome.records), vec!["memory-budget-a"]);
    let over = outcome.refused_for(MemoryRejection::OverBudget);
    assert_eq!(
        over.len(),
        1,
        "静默裁剪出预算是 §42.3 明令防止的失效模式之一：落选的每一条 MUST 具名"
    );
    assert_eq!(over[0].record_ref, "memory-budget-b");
}

#[test]
fn when_one_record_violates_every_layer_only_the_first_one_is_reported() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    let mut candidate = admissible("memory-every-layer", VIEWER_KIND);
    candidate.record.viewer_scoped_id = FOREIGN_VIEWER_SCOPE.into(); // 越作用域
    candidate.record.expires_at_utc_ms = 1; // 已过期
    candidate.record.authorization_ref = String::new(); // 缺授权
    composition
        .write_memory(&candidate, &open_policy())
        .unwrap();
    block(&mut composition, "memory-every-layer"); // 已撤销

    let outcome = read(&mut composition, &viewer_query(8), &open_policy());

    assert_eq!(
        outcome.refused.len(),
        1,
        "一条记录只报一次：报的是**最先**的那一层，不是把四层全报一遍"
    );
    assert_eq!(outcome.refused[0].reason, MemoryRejection::OutOfScope);
    assert_eq!(outcome.refused[0].record_ref, "memory-every-layer");
}

// ---------------------------------------------------------------------------
// 非空洞守卫：每个拒因要么在这里被构造，要么写明由谁构造
// ---------------------------------------------------------------------------

/// §42.3/§42.4: 一个拒因在本文件里的**覆盖方式**。
///
/// 穷尽 `match` 是这个守卫的形状：给 `MemoryRejection` 加一个变体而不在这里决定它归
/// 哪一类，本文件编译不过——守卫不会静默失去覆盖面。
enum Cover {
    /// 本文件的用例真的把它构造出来了。
    Reached,
    /// 不由本文件构造：`why` MUST 写出承载它的那一处（另一个入口 / 内核用例 /
    /// 结构上不存在）。只写「别处」而不写「何处」，等于把洞留在原地。
    Elsewhere(&'static str),
}

/// 本文件**不**构造的三个拒因，按 `MemoryRejection::ALL` 顺序。
///
/// 闭世界：加一个变体而不归类 ⇒ 编译不过；把它归进 `Reached` 却没有用例真的构造它
/// ⇒ 与 `the_read_and_forget_face_is_reachable_end_to_end` 的实测集合对不上。
const NOT_CONSTRUCTED_HERE: [MemoryRejection; 3] = [
    MemoryRejection::UnknownOperation,
    MemoryRejection::ApproximateSubstitution,
    MemoryRejection::ReadSideNotBlocked,
];

/// 读取面 + 遗忘面上**可达**的十三个拒因，按 `MemoryRejection::ALL` 顺序。
const READ_FORGET_FACE: [MemoryRejection; 13] = [
    MemoryRejection::InvalidScope,
    MemoryRejection::ViewerMemoryDisabled,
    MemoryRejection::ViewerReadWithoutAuthorization,
    MemoryRejection::ViewerReadWithoutPurpose,
    MemoryRejection::ApproximateSubstitutionRequested,
    MemoryRejection::OutOfScope,
    MemoryRejection::Expired,
    MemoryRejection::Revoked,
    MemoryRejection::PermissionDenied,
    MemoryRejection::OverBudget,
    MemoryRejection::ErasedClaimWithoutCompletePurge,
    MemoryRejection::ErasedClaimWithThirdPartyRetention,
    MemoryRejection::ThirdPartyRetentionUndisclosed,
];

fn cover(rejection: MemoryRejection) -> Cover {
    match rejection {
        MemoryRejection::InvalidScope
        | MemoryRejection::ViewerMemoryDisabled
        | MemoryRejection::ViewerReadWithoutAuthorization
        | MemoryRejection::ViewerReadWithoutPurpose
        | MemoryRejection::ApproximateSubstitutionRequested
        | MemoryRejection::OutOfScope
        | MemoryRejection::Expired
        | MemoryRejection::Revoked
        | MemoryRejection::PermissionDenied
        | MemoryRejection::OverBudget
        | MemoryRejection::ErasedClaimWithoutCompletePurge
        | MemoryRejection::ErasedClaimWithThirdPartyRetention
        | MemoryRejection::ThirdPartyRetentionUndisclosed => Cover::Reached,
        // 写入面（§42.2）：本文件上半的 `refusal_sample` **真的构造了它们** ⇒ 是 `Reached`，
        // 不是 `Elsewhere`。这一分类问的是「本文件有没有把它构造出来」，不是「它属于哪一节」。
        // （把写入面误归为「别处」会让 `NOT_CONSTRUCTED_HERE` 一次性多出七条 ⇒ 守卫自我矛盾。）
        MemoryRejection::UntrustedSourceAsFact
        | MemoryRejection::UnplayedCandidateAsFact
        | MemoryRejection::WriteWithoutScope
        | MemoryRejection::WriteWithoutSourceRef
        | MemoryRejection::WriteWithoutTimestamp
        | MemoryRejection::FactWithoutPublishRef
        | MemoryRejection::FactOutsideSemanticMemory => Cover::Reached,
        MemoryRejection::UnknownOperation => Cover::Elsewhere(
            "§42.6 操作面：MemoryOperation::parse（crates/nai-kernel/tests/memory.rs）",
        ),
        MemoryRejection::ApproximateSubstitution => Cover::Elsewhere(
            "结构上不存在：存储里没有顶替条目 ⇒ approximate_of 恒为 None（W109 决策 2）；\
             由内核 read_records 直接构造（crates/nai-kernel/tests/memory.rs）",
        ),
        MemoryRejection::ReadSideNotBlocked => Cover::Elsewhere(
            "结构上不可达：forget_memory 先阻断再判定（W109 决策 6）⇒ read_side_blocked \
             恒为真；由内核 forget 直接构造（crates/nai-kernel/tests/memory.rs）",
        ),
    }
}

#[test]
fn every_rejection_is_either_constructed_here_or_named_as_constructed_elsewhere() {
    let mut reached = Vec::new();
    let mut elsewhere = Vec::new();
    for rejection in MemoryRejection::ALL {
        match cover(rejection) {
            Cover::Reached => reached.push(rejection),
            Cover::Elsewhere(why) => {
                assert!(
                    !why.trim().is_empty(),
                    "{rejection:?} 的承载处 MUST 写出来，不能只说「别处」"
                );
                elsewhere.push(rejection);
            }
        }
    }

    assert_eq!(
        elsewhere, NOT_CONSTRUCTED_HERE,
        "「不由本文件构造」的那一类 MUST 恰为这三条"
    );
    assert_eq!(
        reached.len() + elsewhere.len(),
        MemoryRejection::ALL.len(),
        "每个拒因恰归一类"
    );
}

#[test]
fn the_read_and_forget_face_is_reachable_end_to_end() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    open_session(&composition, OTHER_SESSION);

    // —— 条目级：一次读取同时构造出越作用域 / 已过期 / 已撤销 / 权限不足 / 超配额
    write(
        &mut composition,
        &admissible("memory-cropped", SESSION_KIND),
    );
    let mut elsewhere = admissible("memory-elsewhere", SESSION_KIND);
    elsewhere.record.session_id = OTHER_SESSION.into();
    write(&mut composition, &elsewhere);
    let mut lapsed = admissible("memory-lapsed", SESSION_KIND);
    lapsed.record.expires_at_utc_ms = 1;
    write(&mut composition, &lapsed);
    write(
        &mut composition,
        &admissible("memory-revoked", SESSION_KIND),
    );
    block(&mut composition, "memory-revoked");
    let mut unauthorized = admissible("memory-unauthorized", VIEWER_KIND);
    unauthorized.record.authorization_ref = String::new();
    composition
        .write_memory(&unauthorized, &open_policy())
        .unwrap();
    write(&mut composition, &admissible("memory-kept", SESSION_KIND));

    let mut observed: Vec<MemoryRejection> = Vec::new();
    // 配额 1：场次记忆里只留最先的一条（同优先级先到先得），另一条**具名**落选。
    let entry = composition
        .read_memory(&all_kinds_query(1), &open_policy(), TIMESTAMP)
        .unwrap();
    assert_eq!(ids(&entry.records), vec!["memory-cropped"]);
    observed.extend(entry.refused.iter().map(|refusal| refusal.reason));

    // —— 请求级
    observe_request(
        &mut composition,
        &all_kinds_query(1),
        &MemoryPolicy::default(),
        MemoryRejection::ViewerMemoryDisabled,
        &mut observed,
    );
    let mut no_scope = all_kinds_query(1);
    no_scope.scope.session_id = String::new();
    observe_request(
        &mut composition,
        &no_scope,
        &open_policy(),
        MemoryRejection::InvalidScope,
        &mut observed,
    );
    let no_grant = MemoryQuery::new(
        MemoryScope::new(WORKSPACE, SESSION, VIEWER_SCOPE),
        vec![MemoryKind::Viewer],
        8,
    );
    observe_request(
        &mut composition,
        &no_grant,
        &open_policy(),
        MemoryRejection::ViewerReadWithoutAuthorization,
        &mut observed,
    );
    let no_purpose = no_grant
        .clone()
        .with_viewer_grant("authorization-ref-1", "");
    observe_request(
        &mut composition,
        &no_purpose,
        &open_policy(),
        MemoryRejection::ViewerReadWithoutPurpose,
        &mut observed,
    );
    observe_request(
        &mut composition,
        &all_kinds_query(8).allowing_approximate(),
        &open_policy(),
        MemoryRejection::ApproximateSubstitutionRequested,
        &mut observed,
    );

    // —— 遗忘面
    observe_forget(
        &mut composition,
        "memory-kept",
        PurgeState::NotStarted,
        None,
        true,
        MemoryRejection::ErasedClaimWithoutCompletePurge,
        &mut observed,
    );
    observe_forget(
        &mut composition,
        "memory-kept",
        PurgeState::Complete,
        Some("云端副本保留 30 天"),
        true,
        MemoryRejection::ErasedClaimWithThirdPartyRetention,
        &mut observed,
    );
    observe_forget(
        &mut composition,
        "memory-kept",
        PurgeState::BlockedThirdParty,
        None,
        false,
        MemoryRejection::ThirdPartyRetentionUndisclosed,
        &mut observed,
    );

    observed.sort();
    observed.dedup();
    assert_eq!(
        observed, READ_FORGET_FACE,
        "读取+遗忘面可达的拒因 MUST 恰为这些：多了说明有未登记的，少了说明某条守卫没有对象可守"
    );
}

// ---------------------------------------------------------------------------
// W110：控制面接缝（`MemorySurface`）调到装配根
//
// 上一轮（W109）把读取面与遗忘阻断接上了真实存储，但接缝**之上**仍零调用方：
// `read_memory` / `forget_memory` 只有测试在调。本段钉的是「经接缝走一遍」——
// 且 MUST 由**观测到的结果**证明，MUST NOT 由「响应成功」证明。
// ---------------------------------------------------------------------------
use nai_control_api::{MemoryControlResult, MemoryForgetRequest, MemorySurface};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// 把装配根包成控制面拿得到的后端。策略由**装配点**给出（接缝签名不带它）。
fn seam(
    composition: Composition,
    policy: MemoryPolicy,
) -> (Arc<Mutex<Composition>>, MemoryAdapter) {
    let root = Arc::new(Mutex::new(composition));
    let adapter = MemoryAdapter::new(root.clone(), policy);
    (root, adapter)
}

/// §42.4 一次**最保守**的遗忘陈述：只阻断、尚未清理、无第三方留存、不宣称已擦除。
fn forget_request(record_id: &str) -> MemoryForgetRequest {
    MemoryForgetRequest {
        record_id: record_id.to_owned(),
        trigger: ForgetTrigger::UserRequest,
        purge_state: PurgeState::NotStarted,
        third_party_retention: None,
        claims_fully_erased: false,
    }
}

#[test]
fn the_seam_reads_through_the_runtime_and_out_of_scope_stays_decidable() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    open_session(&composition, OTHER_SESSION);
    write(
        &mut composition,
        &admissible("memory-in-scope", SESSION_KIND),
    );
    let mut elsewhere = admissible("memory-elsewhere", SESSION_KIND);
    elsewhere.record.session_id = OTHER_SESSION.into();
    write(&mut composition, &elsewhere);

    let (_shared, surface) = seam(composition, open_policy());
    let outcome = match surface.read(&session_query(SESSION, 8)) {
        MemoryControlResult::Ok(outcome) => outcome,
        _ => panic!("一次请求级合法的读取 MUST 成功"),
    };
    assert_eq!(ids(&outcome.records), vec!["memory-in-scope"]);
    let out = outcome.refused_for(MemoryRejection::OutOfScope);
    // 经接缝走一遍后，越作用域判据 MUST 仍有对象：若取数层按会话预筛，这里是 0。
    assert_eq!(out.len(), 1, "越作用域 MUST 仍可达");
    assert_eq!(out[0].record_ref, "memory-elsewhere");
}

#[test]
fn the_seam_forget_blocks_the_read_side_and_reports_the_effective_moment() {
    let root = tempdir().unwrap();
    let mut composition = prepared(root.path());
    write(&mut composition, &admissible("memory-kept", SESSION_KIND));

    let (_shared, surface) = seam(composition, open_policy());
    let first = match surface.forget(&forget_request("memory-kept")) {
        MemoryControlResult::Ok(moment) => moment,
        _ => panic!("最保守的陈述 MUST 被接受"),
    };
    // 阻断后**再读** ⇒ 该条进 `refused(revoked)`：阻断真的生效了，不是记了一笔就完。
    let outcome = match surface.read(&session_query(SESSION, 8)) {
        MemoryControlResult::Ok(outcome) => outcome,
        _ => panic!("读取 MUST 成功"),
    };
    assert!(outcome.records.is_empty());
    let revoked = outcome.refused_for(MemoryRejection::Revoked);
    assert_eq!(revoked.len(), 1);
    assert_eq!(revoked[0].record_ref, "memory-kept");

    // 重复遗忘 ⇒ **同一个**撤销时刻（撤销时刻是一条事实，重写它等于篡改）。
    // ⚠️ 这段等待不是装饰：同毫秒内「覆盖写」会与原值碰巧相等而绿着过（假阴性）。
    std::thread::sleep(Duration::from_millis(20));
    let second = match surface.forget(&forget_request("memory-kept")) {
        MemoryControlResult::Ok(moment) => moment,
        _ => panic!("已撤销的记录再次遗忘 MUST 返回原有时刻"),
    };
    assert_eq!(first, second);
}

#[test]
fn a_missing_record_is_a_named_failure_through_the_seam() {
    let root = tempdir().unwrap();
    let composition = prepared(root.path());
    let (_shared, surface) = seam(composition, open_policy());
    // 记录不存在 MUST 具名失败，MUST NOT 静默成功。
    match surface.forget(&forget_request("memory-absent")) {
        MemoryControlResult::Fault(code) => assert_eq!(code, ErrorCode::InvalidArgument),
        _ => panic!("记录不存在 MUST 是具名失败"),
    }
}

#[test]
fn a_busy_assembly_root_is_a_conflict_not_a_permission_verdict() {
    let root = tempdir().unwrap();
    let composition = prepared(root.path());
    let (shared, surface) = seam(composition, open_policy());
    // 另一条线程持有装配根：这是**故障**，MUST NOT 被报成「不允许」。
    let (held_tx, held_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let blocker = shared.clone();
    let holder = std::thread::spawn(move || {
        let _guard = blocker.lock().unwrap();
        held_tx.send(()).unwrap();
        let _ = release_rx.recv();
    });
    held_rx.recv().unwrap();
    match surface.read(&session_query(SESSION, 8)) {
        MemoryControlResult::Fault(code) => assert_eq!(code, ErrorCode::Conflict),
        _ => panic!("装配根忙 MUST 是冲突，MUST NOT 报成权限不足或判定拒绝"),
    }
    release_tx.send(()).unwrap();
    holder.join().unwrap();
}

#[test]
fn the_request_level_refusal_survives_the_seam_as_a_refusal() {
    let root = tempdir().unwrap();
    let composition = prepared(root.path());
    // 观众记忆**默认关** ⇒ 请求观众记忆是请求级拒绝，且经接缝后仍是 `Rejected`
    // （三态映射 MUST NOT 把「判定说不可以」折成「路径故障」或「空结果」）。
    let (_shared, surface) = seam(composition, MemoryPolicy::default());
    match surface.read(&viewer_query(8)) {
        MemoryControlResult::Rejected(rejection) => {
            assert_eq!(rejection, MemoryRejection::ViewerMemoryDisabled)
        }
        _ => panic!("请求级拒绝 MUST 经接缝后仍是 Rejected"),
    }
}

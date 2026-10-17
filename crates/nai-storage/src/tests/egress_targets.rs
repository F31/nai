//! W131 / S10 子句一：**外发清单的执行体**（§13.2、INV-17、§281、§563）。
//!
//! 判定输入 MUST 从**存储**读出：调用方给不了、也改不了那份「上传范围」。
//! 这一组钉三件事：
//!
//! 1. 被标记 `local_only` 的文档落进**具名扣留桶**（不是从清单里消失）。
//! 2. 它**仍然**被本地词法检索返回——§281 说的是「只走本地词法检索或人工」，
//!    不是「不可用」。不外发 ≠ 不可检索，这两半必须同时成立。
//! 3. 清单的域是**活动快照**：未发布、已撤销、发布之后才导入的文档都不在里面。
use super::*;

/// 一份可以指定身份与标记的文档（标记走导入入口，不是测试直接改库）。
///
/// ⚠️ 分块与分词走**真实路径**（`split_document` + `tokenize`），不用
/// `synthetic_chunk` 那种按尺寸手搓的块——后者的 `tokens` 是 `t0` 这种占位值，
/// 于是「本地词法检索还能不能命中」这条断言会在**夹具**上失败，而不是在被断言
/// 对象上失败。判据的输入必须等于它要断言的那件事。
fn marked_document(document_id: &str, body: &str, local_only: bool) -> crate::ImportedDocument {
    let chunks =
        nai_knowledge_core::split_document(document_id, body, "sku-kb", "acl-read", 256, 64)
            .unwrap();
    crate::ImportedDocument {
        workspace_id: "test-workspace".into(),
        document_id: document_id.into(),
        source: "fixture".into(),
        parser_version: "txt-1".into(),
        product_scope: "test-workspace".into(),
        acl_ref: "acl-read".into(),
        original_artifact_ref: None,
        valid_from_ms: None,
        valid_until_ms: None,
        content: body.into(),
        local_only,
        chunks: chunks
            .into_iter()
            .map(|chunk| crate::ImportChunk {
                chunk_id: chunk.chunk_id,
                content: chunk.content.clone(),
                source_span: chunk.source_span,
                product_scope: "test-workspace".into(),
                acl_ref: "acl-read".into(),
                tokens: nai_knowledge_core::tokenize(&chunk.content),
            })
            .collect(),
    }
}

fn open_document(storage: &mut Store, document_id: &str, body: &str) {
    storage
        .import_document(
            &op(),
            "operator",
            &marked_document(document_id, body, false),
        )
        .unwrap();
}

fn local_document(storage: &mut Store, document_id: &str, body: &str) {
    storage
        .import_document(&op(), "operator", &marked_document(document_id, body, true))
        .unwrap();
}

/// 读那份「上传范围」。域是活动快照 + 未撤销。
fn egress(storage: &Store) -> nai_knowledge_core::egress::EgressPlan {
    storage
        .remote_egress_targets(&op(), "test-workspace", "test-workspace", "operator")
        .unwrap()
}

fn document_ids(candidates: &[nai_knowledge_core::egress::EgressCandidate]) -> Vec<&str> {
    candidates
        .iter()
        .map(|item| item.document_id.as_str())
        .collect()
}

fn retrieval(storage: &mut Store) -> Vec<nai_contracts::v1::Evidence> {
    storage
        .retrieve_local(
            &op(),
            "operator",
            "test-workspace",
            "test-workspace",
            &[],
            &nai_knowledge_core::tokenize("保修"),
            None,
            8,
        )
        .unwrap()
        .1
}

#[test]
fn a_local_only_document_stays_out_of_the_release_list_and_inside_local_retrieval() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-open", "保修可外发");
    local_document(&mut storage, "doc-local", "保修不外发");
    publish_prepared(&mut storage, None);

    let plan = egress(&storage);

    // 闭世界：两个桶加起来恰好是域里的候选数。
    assert_eq!(plan.scanned(), 2, "域里有两份文档");
    assert_eq!(
        document_ids(plan.releasable()),
        vec!["doc-open"],
        "未被标记的文档可外发"
    );
    assert_eq!(plan.withheld().len(), 1);
    let (held, reason) = &plan.withheld()[0];
    assert_eq!(held.document_id, "doc-local");
    assert_eq!(reason.label(), "local_only");
    assert!(
        !plan.releasable().iter().any(|item| item.local_only),
        "被标记的文档 MUST NOT 出现在可外发桶里"
    );

    // 正向半：不外发**不等于**不可用——被扣留的那一份仍必须被本地检索命中。
    let found = retrieval(&mut storage);
    assert!(
        found.iter().any(|item| item.document_id == "doc-local"),
        "被扣留的文档 MUST 仍被本地词法检索返回，实际命中：{:?}",
        found
            .iter()
            .map(|item| (item.document_id.as_str(), item.text.as_str()))
            .collect::<Vec<_>>()
    );
    assert!(
        found.iter().any(|item| item.document_id == "doc-open"),
        "未被标记的文档同样可本地检索"
    );
}

#[test]
fn re_marking_a_document_changes_the_release_list_because_the_mark_is_read_from_storage() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-open", "保修可外发");
    publish_prepared(&mut storage, None);
    assert_eq!(egress(&storage).releasable().len(), 1);

    // 同一个 id 重新导入并改标记：清单必须跟着走（标记由存储读出，不是调用方给）。
    local_document(&mut storage, "doc-open", "保修改标记");
    publish_prepared(&mut storage, None);
    let plan = egress(&storage);
    assert!(plan.releasable().is_empty(), "改标记之后 MUST 不再可外发");
    assert_eq!(plan.withheld().len(), 1);
    assert_eq!(plan.withheld()[0].0.document_id, "doc-open");
}

#[test]
fn a_revoked_document_is_in_neither_bucket() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-open", "保修可外发");
    local_document(&mut storage, "doc-local", "保修不外发");
    publish_prepared(&mut storage, None);
    assert_eq!(egress(&storage).scanned(), 2);

    storage
        .revoke_knowledge(&op(), "operator", "test-workspace", None, Some("doc-local"))
        .unwrap();

    let plan = egress(&storage);
    assert_eq!(plan.scanned(), 1, "已撤销的文档不属于外发域");
    assert_eq!(document_ids(plan.releasable()), vec!["doc-open"]);
    assert!(plan.withheld().is_empty(), "撤销不是扣留，是两个不同的原因");
}

#[test]
fn a_stored_value_that_is_neither_zero_nor_one_is_read_as_withheld() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-open", "保修可外发");
    publish_prepared(&mut storage, None);
    assert_eq!(egress(&storage).releasable().len(), 1);

    // 列上是 `NOT NULL INTEGER`，SQLite 不把取值限死在 0/1。一个读不出来的值
    // **MUST NOT** 被读成「可以外发」：外发是危险方向，存疑就选不放行那一边。
    storage
        .conn
        .execute(
            "UPDATE documents SET local_only=2 WHERE document_id='doc-open'",
            [],
        )
        .unwrap();

    let plan = egress(&storage);
    assert!(
        plan.releasable().is_empty(),
        "非 0 的存疑取值 MUST 按「被标记」读，MUST NOT 按「它看起来像 0」读"
    );
    assert_eq!(plan.withheld().len(), 1);
    assert_eq!(plan.withheld()[0].1.label(), "local_only");
}

#[test]
fn nothing_published_reports_zero_rather_than_an_error_or_a_skip() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-open", "保修可外发");

    // 还没有活动快照：域是**空集**，报 0，不是错误、也不是「跳过」。
    let plan = egress(&storage);
    assert_eq!(plan.scanned(), 0);
    assert!(plan.releasable().is_empty());
    assert!(plan.withheld().is_empty());
}

/// ⚠️ 为什么必须**单独**有这一条：域里的 `JOIN knowledge_active_snapshots` 这一句，
/// 只有「同一文档在**两个**快照里、且修订没变」时才走到它那一层——修订一变，
/// 旧快照的行就被 `s.revision=d.current_revision` 挡掉了，那句 JOIN 当次无覆盖，
/// 怎么改都不会红（W127：注入不变红 ≠ 注入坏了，也可能是判据没有覆盖那一层）。
#[test]
fn the_release_domain_is_the_active_snapshot_not_any_snapshot_ever_published() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-a", "保修第一版");
    publish_prepared(&mut storage, None); // snapshot-1 = [doc-a]
    // 第二次发布：doc-a 的修订**没变**，于是它在两个快照里各有一行。
    open_document(&mut storage, "doc-b", "保修第二版");
    publish_prepared(&mut storage, None); // snapshot-2 = [doc-a, doc-b]，成为活动快照

    let plan = egress(&storage);
    assert_eq!(
        plan.scanned(),
        2,
        "域是**活动**快照的成员：doc-a 只能被数一次，MUST NOT 被数两次"
    );
    assert_eq!(document_ids(plan.releasable()), vec!["doc-a", "doc-b"]);
    assert!(plan.withheld().is_empty());
}

/// 同一个 `kb_scope` 过滤条件也只有**两个作用域都有活动快照**时才被走到。
#[test]
fn a_document_outside_the_queried_scope_is_outside_the_release_domain() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-a", "保修同域");
    publish_prepared(&mut storage, None); // test-workspace 的活动快照 = [doc-a]
    open_document(&mut storage, "doc-b", "保修异域");
    publish_prepared_in_scope(&mut storage, None, "scope-other"); // scope-other = [doc-a, doc-b]

    let plan = egress(&storage); // 问的是 test-workspace 这个域
    assert_eq!(
        document_ids(plan.releasable()),
        vec!["doc-a"],
        "外发域按 kb_scope 界定：另一个域的文档 MUST NOT 混进来"
    );
    assert_eq!(plan.scanned(), 1);
}

#[test]
fn a_document_imported_after_the_active_snapshot_is_outside_the_release_domain() {
    let dir = tempfile::tempdir().unwrap();
    let mut storage = store(dir.path());
    setup(&mut storage);
    open_document(&mut storage, "doc-open", "保修可外发");
    publish_prepared(&mut storage, None);
    open_document(&mut storage, "doc-late", "保修发布后才导入");

    let plan = egress(&storage);
    assert_eq!(
        document_ids(plan.releasable()),
        vec!["doc-open"],
        "外发域是**活动快照**的成员，不是「库里现有的全部文档」"
    );
    assert!(plan.withheld().is_empty());
}

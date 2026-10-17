//! W131 / S10 子句一：文档级 `local_only` 的外发判定（V3.11 §13.2、INV-17、§281、§563）。
//!
//! 这一面存在的理由是一条**否定式**不变量：标记为 `local_only` 的文档
//! MUST NOT 出现在任何面向远端的清单里（§281：只走本地词法检索或人工）。
//! 它有两个失败模式，都被具名挡住：
//!
//! 1. 把被扣留的文档**静默丢掉**——清单看起来只是更短，没人知道少了什么。
//!    所以扣留是一个**带稳定标记的桶**，不是「它不在可外发清单里」这种缺席。
//! 2. 把「同一个文档」按**名字**去重——`document_id` 相同的两个修订是两份东西
//!    （判定单元是 `(document_id, revision)`，名字不是身份）。
//!
//! 这里**不**回答「上传是否被允许」：那是 `RemoteGateway::sync` 的 `allow_document_upload`
//! 判定。同一个开关答两次会漂移（W88），所以本面只判**标记**，不判策略。
use nai_knowledge_core::egress::{EgressCandidate, EgressError, EgressWithholding, plan_egress};

fn candidate(document_id: &str, revision: &str, local_only: bool) -> EgressCandidate {
    EgressCandidate {
        document_id: document_id.into(),
        revision: revision.into(),
        local_only,
    }
}

fn targets(candidates: &[EgressCandidate]) -> Vec<(&str, &str)> {
    candidates
        .iter()
        .map(|item| (item.document_id.as_str(), item.revision.as_str()))
        .collect()
}

#[test]
fn a_local_only_document_is_withheld_by_name_and_every_candidate_lands_in_exactly_one_bucket() {
    let candidates = vec![
        candidate("doc-a", "r1", false),
        candidate("doc-b", "r1", true),
        candidate("doc-c", "r2", false),
    ];
    let plan = plan_egress(candidates.clone()).expect("判定本身应当成功");

    // 闭世界：两个桶加起来必须**恰好**是输入，一个不多一个不少。
    assert_eq!(
        plan.releasable().len() + plan.withheld().len(),
        candidates.len(),
        "每个候选必须恰好落进一个桶"
    );
    assert_eq!(
        targets(plan.releasable()),
        vec![("doc-a", "r1"), ("doc-c", "r2")],
        "未被标记的文档按原序可外发"
    );
    assert_eq!(plan.withheld().len(), 1);
    let (held, reason) = &plan.withheld()[0];
    assert_eq!(
        (held.document_id.as_str(), held.revision.as_str()),
        ("doc-b", "r1")
    );
    // 具名：扣留原因是一个稳定标记，不是「它不在可外发清单里」这种缺席。
    assert_eq!(reason.label(), "local_only");
    assert_eq!(*reason, EgressWithholding::LocalOnly);

    // 反向：被标记的文档 MUST NOT 同时出现在可外发桶里，否则「扣留」是装饰。
    assert!(
        !plan.releasable().iter().any(|item| item.local_only),
        "被标记的文档 MUST NOT 出现在可外发桶里"
    );
}

#[test]
fn an_empty_candidate_set_is_reported_as_zero_rather_than_skipped() {
    let plan = plan_egress(Vec::new()).expect("空集是值，不是错误");
    assert_eq!(plan.releasable().len(), 0);
    assert_eq!(plan.withheld().len(), 0);
    assert_eq!(
        plan.scanned(),
        0,
        "空集 MUST 清点并报 0，MUST NOT 短路成「跳过」"
    );
}

#[test]
fn the_same_document_id_under_a_different_revision_is_a_different_judgement_unit() {
    // 「同名」≠「同一个」：判定单元是 (document_id, revision)。
    let plan = plan_egress(vec![
        candidate("doc-a", "r1", false),
        candidate("doc-a", "r2", true),
    ])
    .expect("同一个文档的两个修订是两份东西");
    assert_eq!(targets(plan.releasable()), vec![("doc-a", "r1")]);
    assert_eq!(plan.withheld().len(), 1);
    assert_eq!(plan.scanned(), 2);
}

#[test]
fn a_repeated_identity_is_refused_rather_than_counted_twice() {
    // 计数正确 ≠ 清单正确：重复身份会让「桶和 == 输入」成立而清单仍然是错的。
    let error = plan_egress(vec![
        candidate("doc-a", "r1", false),
        candidate("doc-a", "r1", true),
    ])
    .expect_err("同一个 (document_id, revision) 出现两次必须被拒");
    assert_eq!(error, EgressError::DuplicateDocument);
    assert_eq!(error.label(), "duplicate_document");
}

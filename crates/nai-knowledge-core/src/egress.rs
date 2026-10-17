//! 外发计划（V3.11 §13.2、INV-17、§281、§563）：把「哪些文档会离开本机」这件事
//! 从一句声明变成一份可失败的清单。
//!
//! # 这一面判什么、不判什么
//!
//! 判**标记**：一个候选文档若被标为 `local_only`，它 MUST NOT 出现在可外发清单里
//! （§281「标记 `local_only` 的文档只走本地词法检索或人工」）。
//!
//! **不判策略**：「文档上传是否被允许」是 [`crate::remote::RemoteGateway::sync`] 的
//! `allow_document_upload` 判定。同一个能力开关答两次会漂移（W88、PD-31：读取面 /
//! 写入面 / 准入面共用一处判定），所以本面**没有** policy 参数——它只回答标记问题，
//! 不替 `sync` 再答一次策略问题。
//!
//! **不设上限**：可外发集合的规模由快照自己的名册上限界定（一份快照最多选
//! `MAX_SNAPSHOT_DOCUMENTS` 份文档，见 `nai-storage::publication`），所以这里再造一个
//! 数就是同一件事的第二处定义，而且那个闸门**不可能开火**（域里永远到不了界）。
//! 一个永远开不了火的守卫是装饰，不是判据。
//!
//! # 两个被具名挡住的失败模式
//!
//! 1. **静默丢弃**：被扣留的文档如果只是「不在可外发清单里」，清单看起来只是更短，
//!    没人知道少了什么、为什么少。所以扣留是一个**带稳定标记的桶**。
//! 2. **按名字去重**：`document_id` 相同的两个修订是两份东西。判定单元是
//!    `(document_id, revision)` 这个**身份**，不是它的名字——重复身份 MUST 被拒，
//!    而不是被计数两次（「计数正确 ≠ 清单正确」）。

/// 一个候选文档在**外发判定时刻**的事实。
///
/// `local_only` MUST 由存储读出（`documents.local_only`），MUST NOT 由调用方给——
/// 判定输入来自被断言的对象，不来自旁观者的说法。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressCandidate {
    pub document_id: String,
    pub revision: String,
    pub local_only: bool,
}

/// 一份文档被挡在可外发清单**之外**的原因。具名，因为「缺席」不是一个原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgressWithholding {
    /// §13.2 / §281：被标记的文档不上传，只走本地词法检索或人工。
    LocalOnly,
}

impl EgressWithholding {
    /// 稳定标记。它进日志、进审计、进判据的输出比对。
    pub fn label(self) -> &'static str {
        match self {
            Self::LocalOnly => "local_only",
        }
    }
}

/// 只有 [`plan_egress`] 能造出它。
///
/// 字段私有且不给 `Default`：可外发清单是**判定的产物**，不是可以随手拼出来的值。
/// 这与 `MemoryPurgeReport`（`nai-control-api`）同形——「结论由执行得出，
/// 调用方 MUST NOT 参与构造」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressPlan {
    releasable: Vec<EgressCandidate>,
    withheld: Vec<(EgressCandidate, EgressWithholding)>,
}

impl EgressPlan {
    /// 会离开本机的那一份（§281/§563 的「上传范围」口径）。
    pub fn releasable(&self) -> &[EgressCandidate] {
        &self.releasable
    }

    /// 被扣留的那一份，**带原因**。
    pub fn withheld(&self) -> &[(EgressCandidate, EgressWithholding)] {
        &self.withheld
    }

    /// 域里被看过多少候选。空集合 MUST 报 `0`，MUST NOT 短路成「跳过」。
    pub fn scanned(&self) -> usize {
        self.releasable.len() + self.withheld.len()
    }
}

/// 判定没有产物时的具名理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EgressError {
    /// 同一个 `(document_id, revision)` 在候选集里出现了不止一次。
    ///
    /// 它不是「多了一份」这种计数问题：重复身份会让「两个桶之和 == 输入」这条判据
    /// 照样成立，而清单本身是错的。
    DuplicateDocument,
}

impl EgressError {
    pub fn label(self) -> &'static str {
        match self {
            Self::DuplicateDocument => "duplicate_document",
        }
    }
}

/// 把候选集分成「可外发」与「扣留（带原因）」两桶。
///
/// 闭世界是这条判定的**结论**而不是它的前提：两个桶的元素**恰好**是输入，
/// 每个输入恰好落一个桶。空输入 ⇒ 两个桶都是空的（报到 `0`）。
pub fn plan_egress(candidates: Vec<EgressCandidate>) -> Result<EgressPlan, EgressError> {
    let mut releasable = Vec::new();
    let mut withheld = Vec::new();
    // 身份是 `(document_id, revision)`，不是 `document_id`。
    let mut seen = std::collections::BTreeSet::new();
    for candidate in candidates {
        if !seen.insert((candidate.document_id.clone(), candidate.revision.clone())) {
            return Err(EgressError::DuplicateDocument);
        }
        if candidate.local_only {
            withheld.push((candidate, EgressWithholding::LocalOnly));
        } else {
            releasable.push(candidate);
        }
    }
    Ok(EgressPlan {
        releasable,
        withheld,
    })
}

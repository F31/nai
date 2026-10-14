//! Memory write-gate runtime wiring: judge a memory candidate against §42.2's source gate and, only
//! when it passes, land the record in the **real** `memory_records` table (V3.8 §42.2; W107,
//! acceptance H03).
//!
//! W57 landed both halves and wired neither. `nai_kernel::evaluate::memory::admit_write` decides
//! whether a write may become a fact or a promise, and `memory_records` (from `0001-foundation.sql`)
//! is where such a record belongs — but `admit_write` had no production caller at all, and no
//! production path ever touched the table. So "does a **refused** write still land?" was a question
//! only a code reader could answer. This module is the runtime half.
//!
//! Two invariants, both testable:
//!
//! * **Judged admissible ⇒ the judged record lands.** The nine columns are copied from the record
//!   the gate saw. The runtime is a *transport*, not an enricher: it recomputes no column, defaults
//!   none, and lands nothing the gate did not vet.
//! * **Judged inadmissible ⇒ a named refusal, and not one column written.** The refusal carries the
//!   gate's own label, so "which §42.2 rule refused this" stays decidable at the call site.
//!
//! Judgement and persistence are one entry point on purpose. Offering a separate `admit` and
//! `persist` would move the "never persist an unjudged write" duty onto every caller, where one
//! omission breaks the invariant and no test can see it.
//!
//! The judgement is answered **once**. [`Store::record_memory`] checks its own layer (scope
//! identifiers, session permission, primary key) and MUST NOT re-run §42.2 — a gate answered twice
//! drifts (W88).
//!
//! **Boundary (disclosed, not implied):** the §42.3 **read** judgement
//! ([`nai_kernel::evaluate::memory::read_records`]) is **not** wired here. Its entry-level inputs are
//! `ScopedRecord`'s `revoked` / `permitted` / `approximate_of`, and `memory_records` carries no
//! revocation column — asserting `revoked: false` would fabricate a judgement input, which this
//! project forbids (W94's shape). The storage accessor
//! [`Store::memory_records`] is a per-column fetch and MUST NOT be read as having answered it.

use crate::composition::short;
use crate::{Composition, Result, err};
use nai_contracts::v1::{ErrorCode, MemoryRecord};
use nai_kernel::evaluate::memory::{
    ForgetOutcome, ForgetTrigger, MemoryCandidate, MemoryPolicy, MemoryQuery, MemoryReadOutcome,
    MemoryRejection, PurgeState, ScopedRecord, VIEWER_KIND, admit_write, forget, read_records,
};

/// Why a memory write was refused: which record, and which §42.2 rule.
///
/// Both halves are required, for the same reason [`MemoryRejection`] exists at all — a refusal that
/// named neither the record nor the rule would be indistinguishable from a silent drop.
///
/// [`MemoryRejection`]: nai_kernel::evaluate::memory::MemoryRejection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWriteRefusal {
    /// The candidate's own record id, so a caller can act on the right record.
    pub record_id: String,
    /// The gate's stable label for the rule that refused it.
    pub reason: &'static str,
}

/// The outcome of offering one memory candidate to the real memory store.
///
/// Two arms, and that is the whole classification: H03 is **one** judgement (§42.2's source gate),
/// so there is one axis — admitted or not. A third arm would name a state no fact corresponds to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryWriteOutcome {
    /// The gate admitted it; the record now sits in `memory_records`.
    Written(MemoryRecord),
    /// §42.2 refused it. **Nothing** was written.
    Refused(MemoryWriteRefusal),
}

impl Composition {
    /// Judge `candidate` against §42.2 and, only when it passes, persist it to `memory_records`.
    ///
    /// The persisted columns are the judged record's own nine fields and nothing else — see the
    /// module docs for why the gate's other inputs (`source`, `claim`, `source_ref`,
    /// `written_at_utc_ms`, `publish_ref`) are *not* columns here and so are not persisted.
    ///
    /// Returns `Err` (not a refusal arm) for the store's own failures — a duplicate record id, a
    /// missing permission, a bad identifier. Those are not §42.2 verdicts, and folding them into
    /// [`MemoryWriteOutcome::Refused`] would put a storage fault under the gate's name.
    pub fn write_memory(
        &mut self,
        candidate: &MemoryCandidate,
        policy: &MemoryPolicy,
    ) -> Result<MemoryWriteOutcome> {
        if let Err(rejection) = admit_write(candidate, policy) {
            return Ok(MemoryWriteOutcome::Refused(MemoryWriteRefusal {
                record_id: candidate.record.record_id.clone(),
                reason: rejection.label(),
            }));
        }
        let record = candidate.record.clone();
        let operation = short()?;
        let workspace = self.workspace().to_owned();
        let actor = self.actor().to_owned();
        let mut store = self
            .store()
            .try_lock()
            .map_err(|_| err(ErrorCode::Conflict, "memory store busy"))?;
        store
            .record_memory(&operation, &workspace, &actor, &record)
            .map_err(crate::RuntimeError::from)?;
        Ok(MemoryWriteOutcome::Written(record))
    }
}

/// §42.3: 一次读取的失败——**谁的失败**必须分开。
///
/// 请求级拒绝是 §42.3 的**结论**（作用域不完整、请求用近似条目顶替、观众记忆未启用/
/// 缺授权/缺用途）；存储层故障是**故障**（权限、冲突、超时）。把后者折进前者，
/// 就是把一个存储故障记在判定名下——`write_memory` 明确拒绝这么做，读取面同理。
#[derive(Debug)]
pub enum MemoryReadError {
    /// §42.3 请求级拒绝：本次读取本身不成立，一条记录都没有被考虑。
    Rejected(MemoryRejection),
    /// 读取路径自身的故障，不是 §42.3 的结论。
    Fault(crate::RuntimeError),
}

impl From<crate::RuntimeError> for MemoryReadError {
    fn from(value: crate::RuntimeError) -> Self {
        MemoryReadError::Fault(value)
    }
}

/// §42.3: 这条记录**有没有它所属类别要求的授权**在场。
///
/// 观众长期记忆是 §42.3 里**唯一**被要求「读取 MUST 有授权引用」的类别，
/// 其承载物是记录**自己**的 `authorization_ref`；其余类别 §42.3 未设记录级授权要求
/// ⇒ 无所缺，与 `approximate_of` 恒为「无」同属**结构蕴含**，故不为它造承载物。
///
/// ⚠️ **刻意不查 `permission_grants`**（W107 的 notes 曾建议查它）：取数入口
/// `Store::memory_scoped` **已经**要求 actor 持有一条 `session.manage` 授权，
/// 而条目级若再查**同一条** grant，二者**恒一致** ⇒ `PermissionDenied` 将永远
/// **不可达**——守卫还在，却没有任何对象可供它守卫（本项目明令禁止的形状）。
/// 记录级授权要求只针对观众长期记忆，其承载物是记录自己的授权引用。
fn memory_permitted(record: &MemoryRecord) -> bool {
    record.kind != VIEWER_KIND || !record.authorization_ref.trim().is_empty()
}

/// §42.4: 一次遗忘的失败——同样是「谁的失败」。
#[derive(Debug)]
pub enum MemoryForgetError {
    /// 陈述不实：判定拒绝了这次遗忘的**说法**（不是故障）。
    Rejected(MemoryRejection),
    /// 阻断路径自身的故障。
    Fault(crate::RuntimeError),
}

impl From<crate::RuntimeError> for MemoryForgetError {
    fn from(value: crate::RuntimeError) -> Self {
        MemoryForgetError::Fault(value)
    }
}

impl Composition {
    /// §42.3: 读取——**先**从真实存储取数，**再**诚实派生每一条的三个条目级输入，
    /// 然后交给判定。
    ///
    /// 派生 MUST 是事实，MUST NOT 是默认值：`revoked` 取自存储里的撤销列，
    /// `permitted` 取自该记录所属类别要求的授权是否在场（见 [`memory_permitted`]），
    /// `approximate_of` 恒为「无」（存储里**结构上**不存在顶替条目）。
    /// 写死其中任何一项，都是**注入判定而非注入事实**（W94 的形状）。
    ///
    /// 判定**一次**：存储层只取数、不施加 §42.3 的六类判定；`now_ms` 由调用方给，
    /// MUST NOT 由本入口另立一个时间源。
    pub fn read_memory(
        &mut self,
        query: &MemoryQuery,
        policy: &MemoryPolicy,
        now_ms: u64,
    ) -> std::result::Result<MemoryReadOutcome, MemoryReadError> {
        let operation = short()?;
        let workspace = self.workspace().to_owned();
        let actor = self.actor().to_owned();
        let stored = {
            let store = self
                .store()
                .try_lock()
                .map_err(|_| err(ErrorCode::Conflict, "memory store busy"))?;
            store
                .memory_scoped(&operation, &workspace, &actor)
                .map_err(|e| MemoryReadError::Fault(crate::RuntimeError::from(e)))?
        };
        let scoped: Vec<ScopedRecord> = stored
            .into_iter()
            .map(|entry| {
                let revoked = entry.revoked_at_ms.is_some();
                let permitted = memory_permitted(&entry.record);
                ScopedRecord {
                    record: entry.record,
                    revoked,
                    permitted,
                    approximate_of: None,
                }
            })
            .collect();
        read_records(&scoped, query, policy, now_ms).map_err(MemoryReadError::Rejected)
    }

    /// §42.4: 遗忘——**先**阻断读取（真的把该行标为已撤销），**再**判定这次遗忘的
    /// 陈述是否如实。
    ///
    /// 顺序不可交换：§42.4 原话是「先阻断检索与读取，再异步清除副本」。
    /// `read_side_blocked` **不是**调用方可断言的字段——它由「阻断真的发生了」这一事实
    /// 得出；调用方只陈述**它自己**知道的部分（清理进展、第三方留存、是否宣称已完全擦除）。
    /// 否则本入口就会自己构造一个恒真的 `ForgetOutcome`，判定变成装饰。
    ///
    /// 返回**生效中的**撤销时刻（已撤销则返回原有时刻，不覆盖）。
    /// 清除副本不在本入口——那需要另一条工作。
    pub fn forget_memory(
        &mut self,
        trigger: ForgetTrigger,
        record_id: &str,
        purge_state: PurgeState,
        third_party_retention: Option<String>,
        claims_fully_erased: bool,
    ) -> std::result::Result<i64, MemoryForgetError> {
        let operation = short()?;
        let workspace = self.workspace().to_owned();
        let actor = self.actor().to_owned();
        let revoked_at_ms = {
            let mut store = self
                .store()
                .try_lock()
                .map_err(|_| err(ErrorCode::Conflict, "memory store busy"))?;
            store
                .block_memory_read(&operation, &workspace, &actor, record_id)
                .map_err(|e| MemoryForgetError::Fault(crate::RuntimeError::from(e)))?
        };
        let outcome = ForgetOutcome {
            trigger,
            record_ref: record_id.to_owned(),
            read_side_blocked: true,
            purge_state,
            third_party_retention,
            claims_fully_erased,
        };
        forget(&outcome).map_err(MemoryForgetError::Rejected)?;
        Ok(revoked_at_ms)
    }
}

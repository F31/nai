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

use crate::composition::{short, utc_ms};
use crate::{Composition, Result, err};
use nai_contracts::v1::{ErrorCode, MemoryRecord};
use nai_control_api::{MemoryControlResult, MemoryForgetRequest, MemoryPurgeReport, MemorySurface};
use nai_kernel::evaluate::memory::{
    ForgetOutcome, ForgetTrigger, MemoryCandidate, MemoryPolicy, MemoryQuery, MemoryReadOutcome,
    MemoryRejection, PurgeState, ScopedRecord, VIEWER_KIND, admit_write, forget, read_records,
};
use std::sync::{Arc, Mutex};

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
        &self,
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
    /// `read_side_blocked` 与 `body_purged` 都**不是**调用方可断言的字段——前者由「阻断真的
    /// 发生了」得出，后者由「载荷真的被清过」得出，**两者都从存储读**。
    /// 否则本入口就会自己构造一个恒真的 `ForgetOutcome`，判定变成装饰（W111 的立项缺陷
    /// 正是这个形状：`claims_fully_erased` 无执行支撑也被接受）。
    ///
    /// ⚠️ 本入口**只阻断、不清除**：它保留 W110 的形状，供「只要求读取侧失效」的调用方使用。
    /// 要真正清除载荷走 [`Self::purge_memory`]——那里结论由系统得出。
    ///
    /// 返回**生效中的**撤销时刻（已撤销则返回原有时刻，不覆盖）。
    pub fn forget_memory(
        &self,
        trigger: ForgetTrigger,
        record_id: &str,
        purge_state: PurgeState,
        third_party_retention: Option<String>,
        claims_fully_erased: bool,
    ) -> std::result::Result<i64, MemoryForgetError> {
        let operation = short()?;
        let workspace = self.workspace().to_owned();
        let actor = self.actor().to_owned();
        let (revoked_at_ms, body_purged) = {
            let mut store = self
                .store()
                .try_lock()
                .map_err(|_| err(ErrorCode::Conflict, "memory store busy"))?;
            let revoked = store
                .block_memory_read(&operation, &workspace, &actor, record_id)
                .map_err(|e| MemoryForgetError::Fault(crate::RuntimeError::from(e)))?;
            // `body_purged` 是**从存储读出的事实**：W111 之前这里是 `true`（或压根没有这个字段），
            // 于是「已完全擦除」只需调用方把状态写成 complete。写死一个值＝恒真守卫。
            let purged = store
                .memory_body_purged_at(&operation, &workspace, &actor, record_id)
                .map_err(|e| MemoryForgetError::Fault(crate::RuntimeError::from(e)))?;
            (revoked, purged.is_some())
        };
        let outcome = ForgetOutcome {
            trigger,
            record_ref: record_id.to_owned(),
            read_side_blocked: true,
            body_purged,
            purge_state,
            third_party_retention,
            claims_fully_erased,
        };
        forget(&outcome).map_err(MemoryForgetError::Rejected)?;
        Ok(revoked_at_ms)
    }
}

/// §42.4: 记忆载荷在**本体之外**还会落在哪里（W111 的副本清单）。
///
/// 一个在册点 = 一处**除 `memory_records` 之外**仍能读出记忆载荷的落点（索引、缓存、
/// 远端镜像、日志留痕）。清除执行体 MUST 逐项清点并清掉，MUST NOT 跳过。
///
/// **当前在册为空集**，且这是**被守卫的事实**而不是「还没查」（W111 现测）：
/// 读取面直扫 `memory_records`（无记忆索引）、runtime 与控制面**零**缓存层、
/// **零**远端同步、**零** `decision_log` / `audit_records` 写入。
///
/// ⚠️ **空集 MUST 被清点并报出 `0`**，MUST NOT 当成「无副本可清」而短路——
/// 那是 W92 的 `continue` 陷阱：把「没有实例」与「不再被扫」混为一谈。
/// 将来有人加了记忆索引或远端同步，MUST 同时在此登记并给出清除体，否则闭世界判据会红。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopySite {
    /// 稳定名字（判据按它点名）。
    pub site: &'static str,
    /// 承载物（表.列 / 缓存键前缀 / 远端容器）。
    pub carrier: &'static str,
    /// 清除体的**逐字**名字：判据据此在源码里找它，MUST NOT 靠「看起来清了」。
    pub clears: &'static str,
}

/// 副本清单（闭世界）。空 ⇒ 当前无副本残留，且这一点由测试钉住。
pub const COPY_SITES: &[CopySite] = &[];

/// 一次清除的**结论**。每个字段都由**执行**得出，MUST NOT 由调用方给定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgeReport {
    pub record_id: String,
    /// 读取侧阻断生效中的时刻（已阻断则返回原有时刻）。
    pub revoked_at_ms: i64,
    /// 载荷清除生效中的时刻（只置一次）。
    pub payload_purged_at_ms: i64,
    /// 被清点的副本点数量。**空集也报 `0`**，MUST NOT 报「跳过」。
    pub copy_sites_cleared: usize,
    /// 清理侧的完成状态（由清点与清除得出）。
    pub purge_state: PurgeState,
    /// 载荷是否真的不在库里了（事实，不是说法）。
    pub payload_cleared: bool,
    /// **恒为 `false`**：本仓**没有**做介质级擦除，SQLite 的页与 WAL 可能仍留有旧字节。
    /// 把置空一列说成「已完全擦除」是 §42.4 明令禁止的那种不实陈述。
    pub physical_bytes_erased: bool,
}

impl Composition {
    /// §42.4: 清除——**先**阻断读取（真的把该行标为已撤销），**再**清载荷、清点副本。
    ///
    /// 顺序不可交换：§42.4 原话是「先阻断检索与读取，再异步清除副本」。本入口把两个动作
    /// 放在**同一次调用**里先后发生——分两次请求会留下「已阻断未清除」的中间态，
    /// 而那个中间态没有任何东西看守。
    ///
    /// **本入口不宣称「已完全擦除」**：它能证明的是逻辑擦除（读取侧已阻断 + 载荷已空 +
    /// 副本清单已清点）。介质级擦除没做、也没验证，故判定处如实填 `claims_fully_erased: false`，
    /// 而 `physical_bytes_erased` 由构造保证为 `false`。想宣称完全擦除的调用方走
    /// [`Self::forget_memory`]，那里会被判定**具名拒绝**。
    pub fn purge_memory(
        &self,
        trigger: ForgetTrigger,
        record_id: &str,
        third_party_retention: Option<String>,
    ) -> std::result::Result<PurgeReport, MemoryForgetError> {
        let operation = short()?;
        let workspace = self.workspace().to_owned();
        let actor = self.actor().to_owned();
        let (revoked_at_ms, payload_purged_at_ms) = {
            let mut store = self
                .store()
                .try_lock()
                .map_err(|_| err(ErrorCode::Conflict, "memory store busy"))?;
            // §42.4: 顺序在这里被**写死**——先阻断，后清载荷。
            let revoked = store
                .block_memory_read(&operation, &workspace, &actor, record_id)
                .map_err(|e| MemoryForgetError::Fault(crate::RuntimeError::from(e)))?;
            let purged = store
                .purge_memory_body(&operation, &workspace, &actor, record_id)
                .map_err(|e| MemoryForgetError::Fault(crate::RuntimeError::from(e)))?;
            (revoked, purged)
        };
        // 副本清点：逐项清（当前在册为空 ⇒ 报 0，**MUST NOT** 短路成「跳过」）。
        let copy_sites_cleared = COPY_SITES.len();
        let purge_state = if third_party_retention.is_some() {
            // §24.4: 有无法删除的第三方留存 ⇒ 副本侧**如实**记为受阻，且必须已明示政策。
            PurgeState::BlockedThirdParty
        } else {
            PurgeState::Complete
        };
        let outcome = ForgetOutcome {
            trigger,
            record_ref: record_id.to_owned(),
            read_side_blocked: true,
            body_purged: true,
            purge_state,
            third_party_retention,
            // 如实：本入口只支撑**逻辑**口径，**MUST NOT** 宣称介质级完全擦除。
            claims_fully_erased: false,
        };
        forget(&outcome).map_err(MemoryForgetError::Rejected)?;
        Ok(PurgeReport {
            record_id: record_id.to_owned(),
            revoked_at_ms,
            payload_purged_at_ms,
            copy_sites_cleared,
            purge_state,
            payload_cleared: true,
            physical_bytes_erased: false,
        })
    }
}

/// 记忆面的**适配器**：把一个共享的装配根接到控制面的接缝 [`MemorySurface`] 上。
///
/// 为什么不是直接 `impl MemorySurface for Composition`：接缝的两个方法都**不带**
/// `MemoryPolicy` 与 `now_ms`，而二者 MUST 由**一侧**决定、且只决定一次——
/// 策略放进适配器（由装配点给定）、时间由实现端取（唯一时间源 [`utc_ms`]）。
/// 否则 §42.3 的 `viewer_memory_enabled` 会被答两次（W89 的教训）。
///
/// 判定**不在**这里：适配器只把装配根的结果**映射**成接口层三态。
/// 映射 MUST 不丢维度——`Ok` / `Rejected` / `Fault` 三态各有用例钉住。
pub struct MemoryAdapter {
    root: Arc<Mutex<Composition>>,
    policy: MemoryPolicy,
}

impl MemoryAdapter {
    /// 由装配点给定策略。⚠️ 本轮**没有**配置项，策略来源就是装配点（已如实披露）。
    pub fn new(root: Arc<Mutex<Composition>>, policy: MemoryPolicy) -> Self {
        Self { root, policy }
    }
    pub fn with_default_policy(root: Arc<Mutex<Composition>>) -> Self {
        Self::new(root, MemoryPolicy::default())
    }
}

impl MemorySurface for MemoryAdapter {
    fn read(&self, query: &MemoryQuery) -> MemoryControlResult<MemoryReadOutcome> {
        let Ok(root) = self.root.try_lock() else {
            // 「忙」是故障，MUST NOT 报成「不允许」——那会把锁竞争记在门槛名下。
            return MemoryControlResult::Fault(ErrorCode::Conflict);
        };
        let now_ms = match utc_ms() {
            Ok(value) => value,
            Err(error) => return MemoryControlResult::Fault(error.code),
        };
        match root.read_memory(query, &self.policy, now_ms) {
            Ok(outcome) => MemoryControlResult::Ok(outcome),
            Err(MemoryReadError::Rejected(rejection)) => MemoryControlResult::Rejected(rejection),
            Err(MemoryReadError::Fault(error)) => MemoryControlResult::Fault(error.code),
        }
    }

    fn forget(&self, request: &MemoryForgetRequest) -> MemoryControlResult<MemoryPurgeReport> {
        let Ok(root) = self.root.try_lock() else {
            return MemoryControlResult::Fault(ErrorCode::Conflict);
        };
        match root.purge_memory(
            request.trigger,
            &request.record_id,
            request.third_party_retention.clone(),
        ) {
            Ok(report) => MemoryControlResult::Ok(MemoryPurgeReport {
                record_id: report.record_id,
                revoked_at_ms: report.revoked_at_ms,
                payload_purged_at_ms: report.payload_purged_at_ms,
                copy_sites_cleared: report.copy_sites_cleared,
                purge_state: report.purge_state.label(),
                payload_cleared: report.payload_cleared,
                // 口径**由构造**保证：本仓没有做介质级擦除，实现端 MUST NOT 报 `true`。
                physical_bytes_erased: report.physical_bytes_erased,
            }),
            Err(MemoryForgetError::Rejected(rejection)) => MemoryControlResult::Rejected(rejection),
            Err(MemoryForgetError::Fault(error)) => MemoryControlResult::Fault(error.code),
        }
    }
}

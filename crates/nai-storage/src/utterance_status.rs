//! `utterances.status` 的取值域 —— 全仓**唯一**的声明处。
//!
//! ## 为什么需要这个模块
//!
//! 该列在 `migrations/sqlite/0001-foundation.sql` 里是
//! `status INTEGER NOT NULL DEFAULT 1`：**没有 CHECK 约束、没有枚举、没有常量**。
//! 在落本模块之前，它的取值只以**裸字面量**出现在 SQL 字符串里，而且是四套互相不一致的集合
//! （W139 §二第 5 个结论，本轮逐条复核吻合）：
//!
//! | 集合 | 处数 | 出处 |
//! | --- | --- | --- |
//! | `NOT IN(11,12,13,14,15)` 级联保护 | 6 | `store.rs` 4 处 + `knowledge.rs` + `publication.rs` |
//! | `NOT IN(12,13,14,15)` 可用性 | 3 | `store.rs:955` / `:1027` / `:1122` |
//! | `BETWEEN 1 AND 5` 待批准带 | 3 | `store.rs:604` / `:1060` / `:1164` |
//! | `SET status=<N>` 写入 | 9 | 1 / 6 / 12(×6) / 14 |
//!
//! 分歧点在 **11**：它在级联保护集里（不被改写）、却不在可用性黑名单里（被当成可用）。
//! 本模块不裁定 11 的语义（那需要它的写入者先存在），而是：
//!
//! 1. 把四套集合收成**一处声明**，SQL 里的每个数字都由这里派生；
//! 2. 可用性从**黑名单**改成**白名单** —— 未被声明的取值一律不可用，
//!    因为「可用」是危险方向（可用 ⇒ 可播出/可外发），不对称时选安全一侧；
//! 3. 把「不知道它是什么」的取值**具名**为 [`StatusClass::Unassigned`] /
//!    [`StatusClass::ProtectedOnly`]，而不是把它们写进「活跃」或「终态」里冒充已裁定。

/// 待批准：候选已登记、尚未被批准或拒绝。`INSERT` 时写入，也是列默认值。
pub(crate) const STATUS_PENDING: i64 = 1;
/// 已批准：`approval.recorded` 之后写入。仍**可用**，但不可再被拒绝
/// （`store.rs` 另有 `cannot reject approved candidate` 拒绝）。
pub(crate) const STATUS_APPROVED: i64 = 6;
/// 已失效：六条级联（知识失效 / 文档再版 / 全局 / 工作区 / 会话撤销 / 事实版本变更）写入。
pub(crate) const STATUS_INVALIDATED: i64 = 12;
/// 已拒绝：`reject` 写入。
pub(crate) const STATUS_REJECTED: i64 = 14;

/// **待批准带** `1..=5`：可被批准、可被拒绝、可被级联失效。
pub(crate) const PENDING_BAND_LO: i64 = 1;
pub(crate) const PENDING_BAND_HI: i64 = 5;
/// **不可用带** `12..=15`：终态，不再参与任何「可用」判定。
pub(crate) const UNUSABLE_BAND_LO: i64 = 12;
pub(crate) const UNUSABLE_BAND_HI: i64 = 15;
/// **级联保护带** `11..=15`：级联失效**不会**改写这些行。
/// 比不可用带多一个下界值 **11** —— 那正是 W139 记下的分歧点，本模块把它留在
/// [`StatusClass::ProtectedOnly`] 而不是并进终态，因为「合并」与「放宽」都不是裁定。
pub(crate) const PROTECTED_BAND_LO: i64 = 11;
pub(crate) const PROTECTED_BAND_HI: i64 = 15;

/// 取值域的分类。**闭世界**：[`classify`] 对任意 `i64` 都返回其中一格，
/// 不存在「不知道该归到哪」这种第四种结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusClass {
    /// 待批准带内：活跃。
    Pending,
    /// 已批准：可用，但不可再被拒绝。
    Approved,
    /// 不可用带内：终态。
    Unusable,
    /// 只被级联保护、不在不可用带内的那一格（今天只有 **11**）。
    /// 它**不是**终态，也**不是**活跃 —— 它在两处口径里的归属本就矛盾，
    /// 本模块保留这个矛盾而不是替它选一边。
    ProtectedOnly,
    /// 取值域里没有任何一处声明它的语义（如 `0`、`7..=10`、`16..`）。
    Unassigned,
}

/// 把任意取值归到一格。全域覆盖：末尾的 `else` 是 `Unassigned`，
/// 所以「未归类」有一个**可表示的取值**，调用方不会在「撒谎」与「不调用」之间被迫选择。
pub(crate) fn classify(value: i64) -> StatusClass {
    if (PENDING_BAND_LO..=PENDING_BAND_HI).contains(&value) {
        StatusClass::Pending
    } else if value == STATUS_APPROVED {
        StatusClass::Approved
    } else if (UNUSABLE_BAND_LO..=UNUSABLE_BAND_HI).contains(&value) {
        StatusClass::Unusable
    } else if (PROTECTED_BAND_LO..=PROTECTED_BAND_HI).contains(&value) {
        StatusClass::ProtectedOnly
    } else {
        StatusClass::Unassigned
    }
}

/// 一个取值是否**可用**。
///
/// 这是**白名单**而不是「不在黑名单里」：黑名单会把「还没人声明过的取值」默认判成可用，
/// 而可用是危险方向（可用 ⇒ 可播出/可外发），不对称时选安全一侧。
/// 今天它与旧写法在**已写入**的取值上结果相同（`1`/`6` 可用、`12`/`14` 不可用），
/// 差别只落在幽灵值上 —— `11` 与 `7..=10`、`16..` 由「可用」变「不可用」。
///
/// 生产路径上的消费者是 [`usable_values`]（它派生 SQL 的可用性守卫），
/// 所以这**不是**只被判据引用的装饰。
pub(crate) fn usable(value: i64) -> bool {
    matches!(
        classify(value),
        StatusClass::Pending | StatusClass::Approved
    )
}

/// **可用**的取值 —— 由 [`usable`] 派生，而不是把数字再抄一遍。
///
/// 扫描区间是**整个已声明的取值域** `[0, 不可用带上界]`，逐个问 `usable`。
/// 区间必须是全域的，不能只扫 `[待批准带下界, 已批准]`：那样 [`usable`] 一旦退化成
/// 黑名单写法，`1..=6` 两种写法结果相同 ⇒ guard 一字不变 ⇒ 行为判据**不会红**
/// （注入验证实测到的盲区）。全域扫描让「没有声明过的取值」在任何写法下都进不来。
pub(crate) fn usable_values() -> Vec<i64> {
    (0..=UNUSABLE_BAND_HI)
        .filter(|value| usable(*value))
        .collect()
}

/// 级联失效语句的守卫：`status NOT IN(<级联保护带>)`。
///
/// 由 [`PROTECTED_BAND_LO`] / [`PROTECTED_BAND_HI`] 派生 —— SQL 里**不再**有裸数字，
/// 所以「改了常量而 SQL 没跟着变」不可能发生。
pub(crate) fn cascade_guard() -> String {
    band_list(PROTECTED_BAND_LO, PROTECTED_BAND_HI)
}

/// 可用性判据的守卫：`status IN(<白名单>)`。
pub(crate) fn usable_guard() -> String {
    usable_values()
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn band_list(lo: i64, hi: i64) -> String {
    (lo..=hi)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::{
        PROTECTED_BAND_HI, PROTECTED_BAND_LO, STATUS_APPROVED, STATUS_INVALIDATED, STATUS_PENDING,
        STATUS_REJECTED, StatusClass, UNUSABLE_BAND_HI, UNUSABLE_BAND_LO, classify, usable,
        usable_guard, usable_values,
    };

    /// 闭世界第一道：域内每个取值都归到一格，且**每一格都真的被某个取值命中**
    /// （反方向也要查 —— 否则加一格永不命中的分类没人会发现）。
    #[test]
    fn every_class_is_reachable_and_every_value_is_classified() {
        let mut seen = [false; 5];
        for value in -2_000..=2_000 {
            let slot = match classify(value) {
                StatusClass::Pending => 0,
                StatusClass::Approved => 1,
                StatusClass::Unusable => 2,
                StatusClass::ProtectedOnly => 3,
                StatusClass::Unassigned => 4,
            };
            seen[slot] = true;
        }
        assert!(
            seen.iter().all(|hit| *hit),
            "每一格都须有取值命中：{seen:?}"
        );
        // 逐格钉住代表值，防止「分类被重新排列但计数不变」。
        assert_eq!(classify(STATUS_PENDING), StatusClass::Pending);
        assert_eq!(classify(STATUS_APPROVED), StatusClass::Approved);
        assert_eq!(classify(STATUS_INVALIDATED), StatusClass::Unusable);
        assert_eq!(classify(STATUS_REJECTED), StatusClass::Unusable);
        assert_eq!(classify(PROTECTED_BAND_LO), StatusClass::ProtectedOnly);
        assert_eq!(classify(0), StatusClass::Unassigned);
        assert_eq!(classify(PROTECTED_BAND_HI + 1), StatusClass::Unassigned);
        // 全域覆盖（i64 两端也不能漏）。
        assert_eq!(classify(i64::MIN), StatusClass::Unassigned);
        assert_eq!(classify(i64::MAX), StatusClass::Unassigned);
    }

    /// 可用性是**白名单**：只有待批准带与已批准可用。
    /// 幽灵值（11、7..=10、16..、0、负数）一律不可用 —— 这条是今天的**新增**判定，
    /// 改动前 `NOT IN(12..15)` 会把 11 与 7..=10 当成可用。
    #[test]
    fn usable_is_an_allow_list_not_a_blocklist() {
        for value in 1..=STATUS_APPROVED {
            assert!(usable(value), "{value} 须可用");
        }
        for value in [0, 7, 8, 9, 10, PROTECTED_BAND_LO] {
            assert!(!usable(value), "{value} 不得可用（未分配或仅受保护）");
        }
        for value in UNUSABLE_BAND_LO..=UNUSABLE_BAND_HI {
            assert!(!usable(value), "{value} 不得可用（终态）");
        }
        assert!(!usable(-1));
        assert!(!usable(1_000));
        // 白名单逐项列举，且正好等于「可用」谓词判为真的那些取值。
        let listed = usable_values();
        assert_eq!(listed, vec![1, 2, 3, 4, 5, 6]);
        for value in &listed {
            assert!(usable(*value));
        }
        assert_eq!(usable_guard(), "1,2,3,4,5,6");
    }

    /// 两条守卫的一致性：级联保护带**完整包含**不可用带，且与可用白名单**不相交**。
    /// 少了这条，两处常量可以各改一半而无人发现。
    #[test]
    fn protected_band_contains_unusable_band_and_excludes_usable_values() {
        // 编译期成立的包含关系：改常量破坏它 ⇒ 编译不过，而不是等到运行时才发现。
        const {
            assert!(PROTECTED_BAND_LO <= UNUSABLE_BAND_LO);
            assert!(PROTECTED_BAND_HI >= UNUSABLE_BAND_HI);
        }
        for value in UNUSABLE_BAND_LO..=UNUSABLE_BAND_HI {
            assert!(
                (PROTECTED_BAND_LO..=PROTECTED_BAND_HI).contains(&value),
                "{value} 须被级联保护"
            );
        }
        for value in usable_values() {
            assert!(
                !(PROTECTED_BAND_LO..=PROTECTED_BAND_HI).contains(&value),
                "可用值 {value} 不应落在级联保护带里"
            );
        }
    }
}

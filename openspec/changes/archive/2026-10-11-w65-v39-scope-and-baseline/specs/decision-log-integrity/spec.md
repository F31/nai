# decision-log-integrity 变更（w65）

## ADDED Requirements

### Requirement: Claims about the decision log are bounded

决策日志的宣称 SHALL 限于三项。① 哈希链 MUST NOT 被宣称构成「防篡改证明」：它只能发现**部分**改动，
MUST NOT 声称能单独证明本机管理员未重写整条链（跨机留存才构成证据）。② **确定性回放**（同一录制输入）
与**重调在线模型**（结果本可不同、且产生费用）MUST 分开验收，R07 的验收项 MUST 指明是其中哪一种。
③ 业务状态的恢复 SHALL 允许「事务状态表 + 快照 + 关联日志」的**混合**，MUST NOT 绑定单一机制。
限定宣称 MUST NOT 改变 R07 的发布线（仍为 Slice-0），也 MUST NOT 移除哈希链本身。

#### Scenario: A tamper-proofness claim is made

- **WHEN** 文档、报告或对外材料把哈希链描述为「能证明未被篡改」或等价的证明性表述
- **THEN** 该表述 MUST 被订正为「能发现部分改动」；MUST NOT 作为验收通过的依据

#### Scenario: Replay consistency is claimed without naming the kind

- **WHEN** 某项验收声称「回放与原决策一致」，却未指明是确定性回放还是重调在线模型
- **THEN** 该验收项视为不可判定，MUST 先指明属于哪一种；两者 MUST NOT 用同一条验收覆盖

#### Scenario: Recovery is bound to a single mechanism

- **WHEN** 恢复设计被写成「全部业务状态由日志投影重建」这一单一机制，或据此排除事务表与快照
- **THEN** 该设计 MUST 被拒绝：投影表是恢复材料的**一部分**，不是全部

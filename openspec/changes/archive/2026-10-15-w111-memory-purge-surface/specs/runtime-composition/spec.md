## ADDED Requirements

### Requirement: Purge is an entry that blocks first, then clears, and derives its own conclusion

组合根 SHALL 提供一处清除入口：**先阻断读取、再清载荷**，顺序不可交换（§42.4）。
该入口 SHALL **自己**得出副本状态与载荷清除时刻，MUST NOT 接受调用方传入的结论。
`ForgetOutcome` 的 `body_purged` SHALL 由「载荷真的被清过」这一事实得出，MUST NOT 是入参。

#### Scenario: The conclusion is derived, not accepted

- **WHEN** 调用方在清除前自称副本已清
- **THEN** 该入口不接受这个说法，结论由清点与清除动作得出

#### Scenario: A claim of erasure without a purge is refused by name

- **WHEN** 一次遗忘宣称已完全擦除，而载荷清除这件事没有发生
- **THEN** 判定以具名拒因拒绝，MUST NOT 接受一个零执行支撑的宣称

#### Scenario: The order is observable, not just documented

- **WHEN** 清除抛错（例如载荷不存在）
- **THEN** 读取侧的阻断**已经**发生；顺序反了的实现会让阻断一起消失
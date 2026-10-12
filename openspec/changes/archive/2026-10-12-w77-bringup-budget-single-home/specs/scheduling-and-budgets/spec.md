## ADDED Requirements

### Requirement: One budget, one definition per work class
每一类有界工作（bring-up、逐次调用、有界重试、轮询）SHALL 有且只有一个预算定义点。
该定义 SHALL 位于**该类工作的执行者**所在的 crate；其他 crate SHALL 通过引用该定义获得预算，
MUST NOT 复制其数值。`Operation` 的**上界**（拒绝阈值）SHALL 是公开常量并由 `Operation::new`
自身使用；调用方 SHALL 以上界常量而非字面量表达「取上界」这一意图。

#### Scenario: A bring-up budget has exactly one definition
- **WHEN** 任意 crate 需要为「全新数据根 + 全链迁移」这类工作取预算
- **THEN** 它引用执行者 crate（`nai-storage`）导出的 bring-up 预算常量；
  全仓不存在该常量的第二处定义；且该常量的值**引用** `Operation` 的上界常量，而不是字面量

#### Scenario: The ceiling is the rejection threshold
- **WHEN** 以恰好等于上界的时长构造 `Operation`
- **THEN** 构造成功；而以「上界 + 1ms」构造则被拒绝 ⇒ 上界与拒绝阈值是**同一个**常量

#### Scenario: A short per-call budget is not reused for bring-up
- **WHEN** 某个夹具执行 bring-up 工作量（在全新数据根上 `Store::open`，含全链迁移）
- **THEN** 它使用 bring-up 预算；逐次调用的短预算 MUST NOT 被用于该工作量，
  且在并发创建多个数据根时不出现因预算不足而生的 `DeadlineExceeded`

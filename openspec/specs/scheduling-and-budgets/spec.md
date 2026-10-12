# scheduling-and-budgets Specification

## Purpose
为 nai 提供可验证的过载处理、公平调度、播放债务与全局共享多维预算，防止未入选事件启动付费请求，以及多会话超售同一份额度。

## Requirements

### Requirement: Bounded ingress with protected state
系统 SHALL 使用每会话有界输入队列、候选队列和播报队列，拒绝过期/旧 epoch，P0 SHALL 保持独立。关键状态 SHALL 使用独立有界槽位，溢出 SHALL 报告原因；合并礼物 SHALL 保留 delta/cumulative/unknown 语义，未知数量不得累加。

#### Scenario: Burst preserves critical state
- **WHEN** 普通队列满载后收到权限/连接/房间结束消息
- **THEN** 关键消息不等待普通队列，普通溢出计数可观察，未知礼物数量不被推算

### Requirement: Fair paid-work selection and speech debt
调度 SHALL 先去重/主题合并/过期，再按 P1–P5、冷却、单观众占比和欠服务选择；低优先级 SHALL 可在可配置欠服务阈值后获得机会。未入选任务 SHALL 不触发付费准入，入选 SHALL 同时满足 TTL、任务 deadline、待播债务与预算预留。

#### Scenario: Dominant viewer and expired work
- **WHEN** 同一观众持续输入且存在其他观众的有效任务
- **THEN** 冷却/占比阻止垄断，过期任务丢弃，有效低优先任务在阈值后可入选

#### Scenario: Paid request is admitted only after selection
- **WHEN** 队列存在多个任务而本轮仅能选一条
- **THEN** 只有最终选中的任务调用预算预留，拒绝准入不出队或启动 Provider

### Requirement: Shared atomic multidimensional reservation
账本 SHALL 同时检查全局、session 和 Provider 的金额/用量/并发限额；未知范围或单位 SHALL 拒绝，金额 SHALL 使用整数最小单位。重复请求 SHALL 幂等；任一限额失败 SHALL 不产生部分预留。

#### Scenario: Two sessions share one balance
- **WHEN** 两个会话的单独限额均足够但共享余额不足以预留两条请求
- **THEN** 第二条拒绝，第一条保持，不能透支共享预算

### Requirement: Honest settlement and durable recovery
未知实际 usage SHALL 保留估算且标记待对账；派发后的取消 SHALL 不清零已可能计費用量。结算/对账 SHALL 替换已有记账而不是重复相加；实际超估算 SHALL 如实入账并阻止后续超额准入。storage SHALL 原子保存账本，重启不释放未知用量或自动重试请求。

#### Scenario: Cancelled billed request and reconciliation
- **WHEN** 已派发请求取消后缺少实际 usage，随后取得账单
- **THEN** 原估算保留直到对账，以实际量替换，重复同一账单不会双扣

#### Scenario: Durable reservation after restart
- **WHEN** 预留已持久化但进程在 Provider 派发前后退出
- **THEN** 恢复继续占用该请求额度且不自动重新派发，重复同 ID 返回已有状态

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

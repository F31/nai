# D07 调度与预算实测报告

## 实现状态

- 已实现：kernel 有界 ingress/关键状态/去重/礼物语义、候选与播报队列、P1–P5/老化/冷却/份额、TTL/epoch/债务。
- 已实现：budget-core 全局/session/provider 原子预留，金额/请求数/单位/并发，多阶段取消、未知量保留、实际结算/幂等对账。
- 已实现：SQLite 持久账本与审计、独占派发 claim（重复 ACK acquired=false）、批量恢复、重启不释放未明额度。
- 本轮新增 19 项 Rust 测试（调度 8、budget 9、storage 2），新增 1 项 Node schema 测试和 1 种三语言账本 fixture。
- 未开始：真实 Provider 流/账单/取消（D08）、节目内容/checkpoint（D10）、真实播放债务/设备反馈（D11）。
- Blocked：无 Windows 原生验收环境；SQLite Windows 交叉工具缺失延续 D06，未把该检查标通过。
- NotApplicable：数据库列迁移、常驻服务、付费 API/凭据消费。

## 契约与修改

先建立并 strict 验证 D07 OpenSpec，再改 schemas/core/base.proto（BudgetCharge/UsageLedgerEntry/BillingState）、
runtime/provider 可选配置，运行 cargo xtask schema，之后实现消费者。
新增 4 个 descriptor 消息（含 map entry），总 107；能力目录仍 27，schema_version=1 旧配置兼容。
生成 Rust/TS/C++/JSON Schema 类型未手改。
新增 nai-budget-core shared 职责，正常依赖只 contracts；kernel 不依赖 Provider 或 storage。
store 复用 usage_ledger，无新迁移；账本编码/格式见 scheduler 指南和 ADR 0008。
新增 workspace 成员与本地依赖，Cargo.lock 离线更新；没有新增第三方包版本、运行服务或云成本。

## 实际检查

| 命令 | 结果 |
|---|---|
| `cargo xtask check --profile portable` | 9 包依赖边界、fmt、Clippy -D warnings、106 项 Rust 测试通过 |
| `cargo xtask test --suite scheduler` | kernel 31 + budget 9 + storage 15 = 55 项通过 |
| `cargo xtask test --suite contracts` | Rust 6 项、Node 10 项、三语言编译/无漂移、9 种消息的两条互通路径通过 |
| `cargo check --locked -p nai-kernel -p nai-budget-core --tests --target x86_64-pc-windows-gnu` | Windows 分支/测试编译通过；非原生执行 |
| `pnpm typecheck`、`pnpm build` | desktop/contracts 类型与构建通过 |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate d07-scheduling-and-budgets --strict --no-interactive` | 实施前及验证中工件校验通过 |
| 收尾 `cargo clippy --locked -p nai-storage -p nai-contracts --all-targets -- -D warnings` + scheduler suite | 独占派发 claim 补充后通过 |

全量 106 = contracts 6、kernel 31、budget 9、media 7、hosts 9、SDK 14、security 4、storage 15、xtask 11。
互通账本验证 u64::MAX 金额、大于 JS 安全整数的用量，以及 cancelled + NeedsReconciliation 且 actual 缺失。

## 重要验证

输入普通队列/去重满时关键状态仍入独立槽位；有价值评论替换点赞并报告丢失；delta/cumulative/unknown 区分。
未入选事件不预留，准入失败保持候选/债务；旧 TTL 不被话题合并延长；公平/冷却/老化实测。
实际 speech_ms 超估计保留真实债务，未确认结束不自动免费清账；epoch 撤销拒绝旧任务。
两个 session 不超售一个全局余额；scope/单位/币种/配额/并发/容量/溢出失败均不部分预留。
已派发取消保留估算和并发，真实结束才释放并发；超估算结算如实入账；同账单不双扣、冲突修订拒绝。
批量恢复重复/溢出失败保持原空账本；SQLite 重启仍保留未知用量，旧格式不按零解析；未授权/冲突不写入。
派发 ACK 丢失后相同 expected_revision 重试 acquired=false，不自动开始第二次 Provider I/O。

## 失败与修复

- 测试的 panic-only admission 闭包无法推断泛型，补显式 select::<()> 后通过。
- Clippy collapsible_if 拒绝嵌套点赞淘汰分支，改为 let-chain 后通过。
- 审查发现逐条 restore 会重复扫描账本，改为有界批量恢复与一次总量校验，读取循环增加取消/期限检查。
- 审查发现单纯返回 Dispatched 记录不足以证明获得新派发权，增加事务内 acquired claim 与负面重试测试。

## 成本/范围与下一项

单 workspace 账本由可信 owner 配置范围与限额，requests 是尝试计数，不被 token-only usage 清零。
每日周期/账单导入/真实价格解析仍需 D08，当前不自动清空历史、释放未知预留或恢复旧请求。
本轮实测为逻辑/本地 SQLite/三语言契约，不是已获得真实平台授权或已验证 Provider 计费。
下一项 D08：API Provider 的流解析、期限/取消、429/重试/usage 与具备凭据时的真实集成。
未提交、推送或调用付费 API。

## OpenSpec 归档

`d07-scheduling-and-budgets` 完成 8/8 任务，合并 4 条需求为
`openspec/specs/scheduling-and-budgets/spec.md`。
归档目录：`openspec/changes/archive/2026-10-07-d07-scheduling-and-budgets/`。
归档后 `pnpm spec:validate` 与 `pnpm exec openspec validate --archived --no-interactive`
（OPENSPEC_TELEMETRY=0）验证 9 个主规格与 D01–D07 七项归档全部通过。

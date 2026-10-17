# 升级器：校验、事务、门控与成本

`nai-upgrade-core`（shared 层）把 V3.11 §14.3 的九步升级流程落成可执行编排。它只持有**判定逻辑**；
每一项「世界状态」都通过可注入接缝进入，所以它不依赖宿主、平台 SDK 或网络，能在 portable 下完整测试。

```bash
cargo xtask test --suite upgrade      # nai-upgrade-core + nai-storage
cargo test -p nai-upgrade-core --offline
cargo xtask check --profile portable
```

## 为什么在 shared 层

`xtask/src/deps.rs` 早已把 `crates/nai-upgrade-core` 预留为 shared 层成员。升级器一旦放进 `nai-runtime`
（runtime 层），portable 下就无法单独测它（runtime 会拉起宿主与监督）；放进 `nai-storage` 则会逼 storage
认识 `semver` 依赖闭包、签名与成本口径。shared 层禁止依赖 plugin/runtime/app/tool，天然保住
「升级器不认识宿主」的边界。详见 [ADR 0026](adr/0026-upgrade-core-boundary.md)。

## 接缝：所有世界状态都可注入

| 接缝 | 表示的世界事实 | 默认 / 替身 |
|---|---|---|
| `SignatureVerifier` | 包签名是否有效 | `DeterministicVerifier`（**演练替身，非真实签名链**） |
| `RevocationList` | 哪些身份已被撤销、缓存是否仍可信 | 显式构造，带期限 |
| `SpaceProbe` | 安装目标的空闲字节 | `NoSpaceProbe`（永远「未知」→ 拒绝） |
| `ActiveSessionGate` | 当前有几个活动会话 | `RehearsalSessions` |
| `SessionDrain` | 哪些会话依赖某能力、能否排空 | `RehearsalDrain` |
| `UpgradeHost` | 暂存 / 备份 / 迁移 / 自检 / 换清单 / 回滚 | `RehearsalHost`（内存） |
| `UpgradeJournal` | 事务推进到哪一步 | `RehearsalJournal`；生产为 `Store`（`upgrade_journal` 表） |
| `PackageFetcher` | 一个窗口的字节到不到（HTTP `Range`、本地镜像…） | 无默认；`tests/download.rs` 是脚本替身 + 真实 loopback HTTP/1.1 peer |
| `ResumeLedger` | 本地**已持久化**多少字节 | 无默认；生产为包存储，测试为内存台账（含违约变体） |

把任一条写死都会让升级器**绑定一个具体环境**；这正是「不要硬编码、不要绑定太多依赖」要避免的。

## 九步流程

```
prepare : verify → preflight → stage            → journal(prepared)
activate: backup → journal(migrated) → migrate
          → self-check → journal(checked) → 原子换清单 → journal(committed)
recover : 读 journal 当前 step → discard | restore | 确认已提交
```

- **校验先于暂存**：JSON Schema → 文件声明路径规范且唯一 → 内容集合与声明**精确匹配**（缺失/额外/重复
  一律拒绝）→ 逐文件字节数与 SHA256 → 签名 → 撤销。任一步失败**不产生暂存物**，也不改动当前版本清单。
- **预检先于暂存**：拒绝的代价是零字节，且不可能扰动正在运行的版本。
- **换清单原子**：写新文件 → `flush_durable` → 同目录改名覆盖。读者要么看到旧清单、要么看到新清单。

`activate` 与 `prepare` 是两次调用：开播中可以暂存、不可激活。**会话事实归调用方**，事务层不猜。

## 下载阶段：短读是值，误差是终局

`download_package` 在**校验与暂存之前**跑，只做决定：窗口从哪个偏移开始、短读要不要重问、何时按名放弃。
传输（`PackageFetcher`）与「本地已持久化多少」（`ResumeLedger`）都是接缝，core 不持有网络。

- **短读是值**：到达字节少于所请求窗口是**可存活常态**，因此被清点并按预算重问；
  传输返回的 `Err` 是**终局**（`PermissionDenied` 再问一次只是烧预算），不被重问。
- **偏移每次从台账读回**，append 之后**回读确认**；哈希闸门的输入也**读自台账**。
  于是「工件与声明相符」是关于**本地介质**的陈述，而不是关于本次调用持有过哪些缓冲的陈述。
- **超出声明的续传点 ⇒ 拒绝**（不夹取）：它描述的是另一个工件。
- **完整但哈希不符 ⇒ 拒绝 + 先丢弃其持久化字节**。理由不是洁癖而是续传点本身——坏前缀若留下，
  下一次会在坏文件的末尾续传，而那个位置永久不会被重取（重启也带不走）。
- 重问上界 `FetchPolicy::max_attempts_per_window`（默认 3）耗尽 ⇒ `RESOURCE_EXHAUSTED`（具名）。
- 失败**不产生任何东西**：本模块无法暂存、无法写 journal、无法换清单，成功时唯一的输出是
  `VerifiedPackage`——与 `verify_package` 返回的是同一个值。这就是「网络断开时原版本继续运行」的**结构性**一半。

## journal 状态机与恢复

`upgrade_journal` 表（`0001-foundation.sql` 既有）单条记录**原地推进**，不是追加日志：

`prepared → migrated → checked → committed`，失败置 `rolled_back`（保留记录，审计需要知道发生过什么）。
`backup_ref` 与 `error_code` 一经写入不被后续步骤抹除。

**journal 写前记录**（先记 step，再执行对应效果）。崩溃因此只留下「至多领先一步」的记录，恢复永远
**向后收敛**。反例：若「效果后记录」，迁移中途崩溃会留下 `prepared` 压在半迁移的目录树上——那是唯一
无法安全恢复的状态。

| 当前 step | 恢复动作 |
|---|---|
| `prepared` | 丢弃暂存，原版本从未被动过（`Abandoned`） |
| `migrated` | 用 `backup_ref` 恢复，**导出**升级后新增事实与审计 |
| `checked` | 比较 `current_manifest` 与记录的目标：已落地即确认 `committed`，未落地则回滚 |
| `committed` / `rolled_back` | 终态重入**零副作用** |

`checked` 的清单身份比较消除了「换清单成功但 journal 未写」的歧义窗口——不靠猜。

## 撤销列表：带期限的可信缓存

`RevocationList { issued_at, expires_at, entries }` 是**三态**，不是集合：

- 命中 → **拒绝**；
- 未命中且缓存**未过期** → 放行；
- 未命中但缓存**已过期** → **拒绝**（`RevocationUnavailable`，不是放行）。

过期即拒绝的理由：过期的撤销缓存意味着「不知道现在有没有被撤销」。把它当「无撤销」放行，正好让撤销
机制**在最需要它的时候静默失效**。安装（`check_install`）与启动（`check_startup`）是两个独立入口——
规格要求两者都查。

## 预检：三态资源，未知即拒绝

每个检查答 `Sufficient | Insufficient(reason) | Unknown(reason)`。`Unknown` 与 `Insufficient` **同等拒绝**，
只是错误码不同（`CAPABILITY_NOT_READY` / `RESOURCE_EXHAUSTED`）——「不知道够不够」离「够」很远，
渲染成「够」就是伪报（R09 的「不伪报可用」）。

磁盘按 §14.3 的字面意思要**双份**：`install_bytes`（暂存）+ `migrated_bytes`（新数据目录）**同时存在**。
预检只产出报告，**不触碰当前版本清单**。

## 会话门控与降档

- `SessionGate::activation` 返回 `Allowed | Deferred { active }`（不是裸 `bool`——为什么被延后是答案的一部分）。
- `require_idle` 在存在活动会话时以 `CAPABILITY_NOT_READY` 拒绝激活；**已暂存的包不失效**，会话结束即用同一次
  暂存激活，不重复暂存。
- `downgrade` 逐能力**先排空、后停用**：`retire` 只在 `drain` 报告剩余为 0 后调用；排空不彻底即
  `CONFLICT` 停止，该能力保持运行，不把会话搁浅在已经不存在的东西上。
- `retire` 契约**禁止触碰存储数据**：降档退掉的是代码，不是事实。
- `require_rollback_compatible` 在目标超出包 `rollback_range` 时以 `VERSION_INCOMPATIBLE` 拒绝——
  不兼容是拒绝，不是可以推翻的偏好。

## 成本口径：未知 ≠ 0

`CostEstimate` 的每个分项是 `Known(amount) | Unknown(reason)`。0 是**可被核对的断言**（「这块免费」），
我们给不出；「未知」是可支持的真相。`total()` 因此是 `Complete(sum)` 或 `Partial { floor, unknown }`——
把部分和当作总额，就是分项层面已经避免的同一个「不伪报」违规，只是上升了一层。清单未声明的分项
即 `Unknown`。

**成本的对外来源是 `POST /api/v1/upgrades/plan`**（W137，权限 `runtime.upgrade`）：控制面收下请求里的
**包清单**，交给 `CostEstimate::from_package`（定义处只有一处）并把四个分项同步搬出去——不下载、
不暂存、不迁移，因此**不铸造作业 id**（没有可轮询的进行中状态）。清单结构由
`schemas/config/upgrade-package.schema.json` 校验，控制面读的就是这一份，不在 OpenAPI 里重述。

两处口径不合并，差异挡在控制面（方向取拒绝）：清单 schema 允许币种 1–8 字符、金额任意大，
而呈现契约要三字母大写码与 `2^53-1`（消费端 `Number.isSafeInteger` 会悄悄取整）⇒ 这两种包
被具名拒绝，而不是发一个自己契约不合格的 200。**本路径不验签**（请求里只有清单、没有包体）：
验签属于 execute 侧的 `UpgradeTransaction`，界面必须把这一点与金额一同呈现。

## 未做边界

- **没有真实签名链**。`DeterministicVerifier` 是基于 `sha2` 的演练替身，只证明「校验被调用、失败会拦住
  暂存」。真实非对称验证（Ed25519 / 代码签名）是留待 update-helper 的注入点。
- **没有真实安装器**。`RehearsalHost` 不持有字节、不落盘；真实 Windows 安装（暂存包、快照数据目录、
  改名换清单）是后续注入。
- **下载阶段只有编排与接缝，没有真实分发客户端**：`PackageFetcher` 是注入点，core 只决定「向哪一段要、
  是否重问、何时放弃」；真实分发端点与 Windows 文件锁的独立 update-helper 仍是后续注入。
  下载故障语义（断流 / 续传 / 重问预算 / 下载期校验）已在**真实 loopback TCP/HTTP/1.1** 上实测，
  见 [W126 报告](reports/2026-10-16-w126-download-resume-and-budget.md) 与 [ADR 0104](adr/0104-download-resume-and-budget.md)。
- **`semver` 只做交集判断**，不实现完整 SAT 求解：单层 `requires` 足够，多包冲突本轮未覆盖。
- `nai-upgrade-core` 的第一个 app 级消费者是**控制面**（W137：`nai-control-api` 依赖它，为
  `/upgrades/plan` 提供成本）；**装配根（`apps/engine`）仍不持有升级事务**——plan 是纯读，
  不需要运行时状态；execute/rollback 的接线仍是后续工作项。

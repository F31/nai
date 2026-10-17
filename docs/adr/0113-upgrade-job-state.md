# ADR 0113：O02 的另一半——作业状态接上真实 journal

- 状态：已接受（W138，2026-10-17）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§30.1 第 4 条「升级与回滚提示」、§662 O02、§900 数据根）
- 相关：ADR 0112（W137 `plan` 同步化，本 ADR 接其「只缺 journal 那一半」的结论）、
  ADR 0063（W85 OpenAPI 操作闭世界）、`docs/control.md`、`docs/upgrade.md`、`docs/ui.md`

## 背景

W137 把 §30.1 第 4 条的前置收窄成一句可判的话：**只缺 journal/执行状态那一半**。本轮去补那半时，
现测出三件事，合起来决定了本轮的形状：

| 现测 | 结论 |
| --- | --- |
| `upgrade_journal` 表、`UpgradeStep` 五态机、`record_upgrade`/`upgrade_record` 都在 `nai-storage`，`Store` 就是 `UpgradeJournal` 的生产实现 | **状态来源是真的**，「执行状态」这一半仓库内可解 |
| `impl UpgradeHost for` 全仓只有 `rehearsal.rs:153` 的 `RehearsalHost`（「holds no bytes and touches no disk」） | **执行者不存在**，`/upgrades/execute` 无法真的推进任何一步 |
| 契约 `/jobs/{id}` 声明了 `job.read` + `*readResponses`，`handle` 里零分支；execute/rollback 复用 `*jobResponses`（202 + `job_id`），也零分支 | **两头都是空的**：202 那头没人铸 id，`/jobs/{id}` 那头没人读 |

第三条是本轮要解决的**形状**：`plan` 的教训不是「job 一律不好」，而是**铸一个永远查不到的 id 就是伪造
可观测面**。同样的判据套在这里，得到的不再是「改成同步」，而是「先让读者存在，再让不肯执行的写者说清楚
它为什么不肯」。

## 决定

1. **`GET /api/v1/jobs/{id}` 读 journal，不猜状态。** `JobBackend` 持 `Arc<Mutex<nai_storage::Store>>`，
   照 `ApprovalBackend` 同形（同一把锁的 `try_lock` 失败就是 409，不阻塞请求线程）。`step` 五值直接来自
   `UpgradeStep`，`terminal` 由枚举自己的 `terminal()` 派生——控制面**不写第二份判定**。
   判据把契约的 `step` 枚举与 `UpgradeStep::ALL` 逐字比对，于是「枚举加了一态、契约没跟上」会当场红。

2. **「没有这条事务」与「作业面未接入」必须能分开。** 现有 16 个错误码里没有「所命名的对象不存在」这一格，
   而这两件事在界面上的含义完全不同：前者是我们把一条事务弄丢了，后者是我们还没有这个功能。
   照「不可测的状态 MUST 有可表示的取值」的纪律，加 `NOT_FOUND = 17`——**拒绝**比含糊更诚实。
   `descriptor.test.mjs` 已有「openapi 错误枚举 == proto 枚举」的耦合断言，改一处漏一处会立刻红，
   所以这不是随手加一个值，是一条被两边钉住的值。

3. **`/upgrades/execute` 与 `/upgrades/{id}/rollback` 具名拒绝，且不铸 `job_id`。**
   没有平台安装器就一步都推不动；此时发 202 等于承诺一个永远不会到来的终态。拒绝用
   503 `CAPABILITY_NOT_READY`、`category` 指明升级面、`message` **点名缺的是平台安装器**。
   判据对它做**内容锚点**断言——只判「非空字符串」等于没判。
   契约增 `503` 声明，但这两条**仍不标** `implemented-loopback`：在本仓库的闭世界里
   「已声明未实现」就是「没有该标记」，不新增第三个状态值（那要先放宽 `operationErrors`）。

4. **拒绝不等于把 12 条同族路由一起改。** `*jobResponses` 被 14 条路由复用，除 upgrades 外还有 12 条
   （`knowledge/import`、`knowledge/snapshots/{id}/publish`、`knowledge/evaluate`、
   `knowledge/documents/{id} delete`、`providers/{id} put`、`providers post`、`avatar post`、
   `scene put`、`scene post`、`scene delete`、`devices post`、`backups post`、`restore/preflight`）
   同样铸 `job_id` 而没有状态来源。它们各自缺的是**自己的状态表**，本轮没有为它们设计状态形状；
   一起改等于用同一条拒绝掩盖 12 个不同的缺口。故只**具名登记**，不修、也不新造一条会当场变红的判据。

5. **桌面只加读者，不加渲染。** `parseJobView` 严格解析（未知步骤**不猜**，呈名为缺口），
   `ControlClient.fetchJob` 提供请求形状。`apps/desktop` 仍无 DOM，故这**不**构成 O02 的 UI 验收。

## 后果

- **`O02` 仍保持 `pending`。** 本轮补上的是「执行状态」的**读者**，但它**没有生产者**：
  `/upgrades/execute` 拒绝执行，journal 里不会有控制面写入的行。`/jobs/{id}` 现在能如实回答
  「没有这条事务」，这比 404 `CapabilityAbsent` 诚实，但**不等于**「升级进行到哪一步、能否回滚」已经可呈现。
  registry 与 `docs/ui.md` 必须写这一句，MUST NOT 记作「作业语义已跑通」。
- §30.1 第 4 条的状态从「只缺 journal/执行状态」变成**读者已有、写者仍缺**。这是**更窄**的缺口，
  但仍然是缺口——把它写成「已满足」就是把「装好了仪表盘而发动机不在」说成能开。
- 契约生成物两处变更，本机再生并核 `--check`（W136 装好的 protoc 3.21.12 使这条路径本机可闭环）。

## 残余风险（具名）

- **`UpgradeStep::ALL` 与枚举可能不一致**：判据能钉住「契约 == `ALL`」和「每个值都能 `parse`」，
  但**钉不住**「有人给枚举加了一态却忘了改 `ALL`」——那时契约、`ALL`、判据三者一起保持旧值，全绿。
  这是构造性盲区，只能靠注释「改枚举 MUST 同时改 `ALL`」+ 本 ADR 具名，MUST NOT 记作「已注入验证」。
- **两处 `try_lock` 语义**：`JobBackend` 与 `ApprovalBackend` 共用同一把 `Store` 锁，
  并发压测下会互相把对方判成 409。这是既有形状的继承，不在本轮改（改它要重设计锁的粒度）。
- 12 条同族缺口未修（见决定 4），它们仍是「202 + 查不到」的形状。

## 未做（具名）

- 平台安装器（`UpgradeHost` 生产实现）：stage 字节、快照数据目录、rename 换 manifest——一行都没写。
- 作业的**生产者**：没有控制面路径会写 journal，故 `/jobs/{id}` 目前只有测试夹具造的记录。
- 提交后的回滚语义：journal 的 `permits` 拒绝一切终态迁移，故「已提交的升级再回退」必然是**一笔新事务**
  （`from` = 旧记录的 `to`，`to` = 旧记录的 `from`），而不是对旧记录的状态迁移。本轮未实现该语义，
  只在 ADR 记下这个读法，免得后来者以为 journal 漏了一个状态。
- 12 条同族路由的状态表与拒绝路径。

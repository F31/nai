# W145 报告：`blockedBy` 的取值由路由器自己作证

- 日期：2026-10-18
- 起点：W144 报告 §五第 1 条 / §八第 2 条（「`blockedBy` 取值缺执行面判据」）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§30.1 第 4 条 / §662 O02）
- ADR：[0119](adr/0119-blockedby-proved-by-router.md)；变更：
  `openspec/changes/w145-blockedby-behavioural-proof`（归档后位于 `openspec/changes/archive/`）

## 一、勘定与现测

### 1. 缺口到底在哪一层

W144 之后，每条在册项的 `blockedBy` 都带着一个 `anchor`：`{file, text}`，判据要求那段文本还在
被跟踪的文件里。锚点能证明的是「这句话还在源码里」，证明不了「这条路由真的没有分支」。
把这两件事分开的是本轮的第一件工作。

| 事实 | 实测口径 |
| --- | --- |
| 在册项 | **15**（13 × `no_handler`、2 × `refuses_without_installer`） |
| 路径字面量出现在 `crates/nai-control-api/src/tests.rs` 的在册项 | **1**（`/api/v1/upgrades/execute`） |
| 其中作为**分类证据**出现的 | **0** |
| 路由器权限条件 / 路由形状 | **10 / 13** |
| 「分类」与「行为」之间的连接 | 仅 `JOB_BLOCKERS[*].anchor` 的文本锚点 |

那唯一的 1 条来自 `execute_and_rollback_refuse_by_name_and_mint_no_job_id`（`:1748`），其意图是
「拒绝必须点名缺安装器」，不是「这两个端点被分类为 `refuses_without_installer`」。⇒ **13 条
`no_handler` 从未被任何执行面判据碰过**，而这 13 条恰恰是「这条路由不存在」这个断言的全体。

### 2. 路由器的执行面（逐条读源码得出）

`handle()` 里两段链子：

- 权限链（`:508–:539`）——`(method, path)` 先被认领成某个 `x-required-permission`；
- 派发链（`:621–:653`）——认领之后决定调哪个处理函数。

两段合起来共 **13 条路由形状**：`GET health`、`GET runtime/status`、`GET sessions/{id}`、
`POST sessions/{id}/mute`、`POST sessions/{id}/takeover`、`GET approvals`、`GET jobs/*`、
`POST approvals/*/approve|reject`、`POST memory/read`、`POST memory/forget`、
`POST upgrades/plan`、`POST upgrades/execute`、`POST upgrades/{id}/rollback`。

其中只有后两条是作业面在册项。其余 11 条都是非作业面路由，**没有一条**与 15 条在册项重叠。

### 3. 为什么不能靠文本解析补这一半

这是本轮最重要的取舍，写进 ADR 0119 决定 1。三条理由：

1. `path == "..."` 只是**条件文本**；它是否被执行、执行后走到哪一步，正是「有没有分支」这个问题本身。
2. 链子有**两处**，且都用 `starts_with` / `ends_with` 拼路径（`/upgrades/{id}/rollback` 根本没有
   完整字面量）。要把两处拼成「路由集」，拼接规则本身就是一条新的、没人验证过的判定。
3. 失败信号的质量不同：行为探针红的时候直接给出「期望 404，得到 503」和响应体；文本解析红的时候
   只给出「集合不等」，排查者还得回去确认哪一侧错了。

## 二、收口方式

两件事，各自独立：

1. **单一枢纽**：15 条分类从 `job-surface.mjs` 的字面量挪进 `tests/job-surface.json`。契约侧
   （`schema.test.mjs` 用 `JSON.parse`）与执行侧（`tests.rs` 用 `include_str!` + `serde_json`）
   读**同一份字节**。
2. **行为探针**：`the_router_treats_every_registered_job_surface_as_declared` 逐条把
   `(method, path)` 打进真实路由器（`reqwest` 打本机 loopback，与既有用例同一套 `harness`），
   断言分类型与响应一致。

判据对「分类」给出三类可观测形状：

| `blockedBy` | 期望 | 变红的含义 |
| --- | --- | --- |
| `no_handler` | `404` + `CAPABILITY_ABSENT` + 无 `job_id` | 路由被接上了分支（或改成了「名字查不到」） |
| `refuses_without_installer` | `503` + `CAPABILITY_NOT_READY` + `category=upgrade` + 无 `job_id` | 不再拒绝，或开始铸 id |
| 其他（今天含 `kind`） | **硬失败** | 出现了判据没有行为期望的类别 |

第三条是有意的：`kind` 意味着「真能被 `/jobs/{id}` 轮询」，而今天正面样本数 = 0。写一条
「`kind` ⇒ 不得答 404」的宽松期望能让分支看起来完整，但那是一条**没有样本的期望**——永远绿，
永远不证明任何事。硬失败把「第一次真正接上作业状态」变成一笔必须付的账。

配套的两条细节：

- `{id}` 之外的路径参数**具名失败**（`assert!(!route.contains('{'))`）：悄悄把 `{slug}` 留在
  路径里，探针就会去打一条客户端永远不会发的路径，而它照样 404——分类看着通过，实际什么都没测。
- 权限给足（`harness(&["runtime.upgrade"])`）：分类说的是**能力**这一层，请求 MUST NOT 先被
  403 挡在门口。

## 三、单一枢纽为什么需要一条判据

两侧各读一份数据是本轮**新引入**的失败模式：契约侧判「登记集 ⇔ 202 集」、执行侧判「分类 ⇔ 行为」，
两边都绿，但一份是 `tests/job-surface.json`、另一份是别的文件。它比任何一处锚点断裂都安静。

所以 `JOB_REGISTRY` 不只记路径，还记执行侧**逐字嵌入的那一句**：

```js
consumer: {
  file: 'crates/nai-control-api/src/tests.rs',
  text: 'include_str!("../../../tests/job-surface.json")',
}
```

只判「有个 `include_str!`」是不够的——指向别的文件同样满足「读了个文件」。`jobSourceErrors`
因此多了三条：登记表 MUST 被跟踪、消费者 MUST 被跟踪、消费者 MUST 逐字包含那句锚点。

另一处配套：`jobSurfaceErrors(documents, surfaces)` 的 `surfaces` **去掉默认值**。有默认值时，
「没给登记表」会静默变成「用仓库里那份」；去掉之后它落到既有缺参分支上，与「空登记表」分开报。

## 四、判据与注入

### 4.1 新增/改动的判据

| 位置 | 内容 |
| --- | --- |
| `tests/job-surface.json`（新） | 15 条分类，两侧共读 |
| `scripts/contracts/job-surface.mjs` | 删 `JOB_SURFACES`；加 `JOB_REGISTRY`；`surfaces` 去默认值；`jobSourceErrors` +3 条 |
| `crates/nai-control-api/src/tests.rs` | +1 条用例（15 次真实 HTTP 探测 + 闭世界） |
| `tests/contracts/schema.test.mjs` | +1 条用例（单一枢纽）+ 3 个注入；既有 4 条改为从盘上读登记表 |

### 4.2 注入验证（六次，全部逐字节还原）

| 编号 | 注入 | 观测 |
| --- | --- | --- |
| I1 | `post /upgrades/execute` 由 `refuses_without_installer` 翻成 `no_handler` | 红：`left: 503 / right: 404`，报 `post /upgrades/execute` |
| I2 | `post /knowledge/import` 翻成 `refuses_without_installer` | 红：期望 503 得 404，响应体带 `CAPABILITY_ABSENT` |
| I3 | 类别改成 `deferred`（未定期望） | 红：硬失败分支命中 |
| I4 | 把 `post /devices/pair` 改名成 `post /upgrades/plan`（真有分支） | 红：期望 404 得 400 ⇒ **分支能被看见** |
| J1 | 执行侧 `include_str!` 改指 `job-surface-alt.json` | 红：`does not embed "include_str!(...)"` |
| J2 | `JOB_REGISTRY.path` 改指 `tests/other-surface.json` | 红：`is not tracked`（且真实仓库那条一并红） |

I4 是本轮最要紧的一条：它证明探针**能区分「没分支」与「有分支但拒绝」之外的一切**——
如果一条路由只是被接上了分支（哪怕它返回 400），`no_handler` 这个分类立刻站不住。

还原方式：`cp` 备份 → 改 → 跑 → `cp` 回来 → `sha256sum` 比对；六次全部 `MATCH`，备份已删除。

## 五、诚实边界

1. **`kind` 那一半仍无真实样本。** 执行侧对 `kind` 是硬失败，不是通过。这与 W144 §五第 3 条
   一致，本轮没有改变它；ADR 0119 把它登记为具名残余第 1 条。
2. **探针证明的是「今天的路由器这样处理」。** 将来注入平台安装器后 `upgrades/execute` 若开始铸 id，
   探针会先红（期望 503 得到 202/200）——那是设计意图：那时该条目应从 `refuses_without_installer`
   改成 `kind`。
3. **`no_handler` 的兜底措辞仍由文本锚点单独守。** 执行侧只钉 `CAPABILITY_ABSENT`；措辞改名不会让
   Rust 侧红（JS 侧会）。这是「同一件事不在第三处定义」的代价。
4. **`JobAccepted` 仍然零生产者。** 本轮一个 202 都没有接上真实实现——这个事实没有变化。
5. **符号迁移的账**：`JOB_SURFACES` 在 W144 的 ADR / 报告 / 归档变更里继续存在（历史记录不就地改）；
   活规格 `openspec/specs/local-control-surface/spec.md` 的两处已 1:1 订正；代码里已无该符号。

## 六、门控

`cargo xtask check --profile full` → **GATE_EXIT=0**（`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1`，
`PROTOC` 指向 3.21.12）。逐项：

| 项 | 实测 |
| --- | --- |
| `cargo fmt --all --check` | 通过（门控内执行；本轮改动后单独跑过一次 EXIT=0） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过，无告警输出 |
| `cargo test --workspace --locked --exclude nai-xtask` | `test result:` 行 **102** 条，合计 **1147 passed / 0 failed**（W144 的 1146 + 本轮 `the_router_treats_every_registered_job_surface_as_declared`） |
| Node 侧 15 组 | 合计 **372 passed / 0 failed**（W144 的 371 + 本轮「同一份字节」用例）；其中 `tests/contracts/schema.test.mjs` **35 / 35** |
| `openspec validate --all --strict` | **43 passed / 0 failed**（归档后；归档前为 44 / 0，含 `change/w145-blockedby-behavioural-proof`） |
| `openspec validate --all --strict --archived` | **142 passed / 0 failed**（含 `2026-10-18-w145-blockedby-behavioural-proof`） |

**第一次是 EXIT=1，且不是判据在响**：`cargo test` 阶段报 `LNK1104: 无法打开文件
…\deps\nai_control_api-b392fa443faf5bf3.exe`，链接失败。三条观测把它与代码缺陷分开：①链接失败的
目标文件当时**并不存在**（`ls` 为空），不是覆盖不了旧产物；②`tasklist` 里无任何 `nai_control*`
残留进程占用它；③同环境、同参数重跑即绿，两轮之间**没改一个字节**。属本机已知的链接期瞬态，
处置是重跑，**未改代码、未放宽任何判据**。

## 七、CI 观测

提交 **`b078105`**（12 文件 / +798 −47）→ run **`39114038861`**（`Engineering foundation`），
`completed / success`，`head_sha` 与推送 SHA **完整 40 位一致**
（`b078105f3415631e37f0ea2c245eee0dd334b395`）。

判结论三件齐：唯一查询参数 `cb=<纳秒>` + `Cache-Control: no-cache` + 回读 `head_sha`；
完成态查了 **5 次**（120s 间隔的有界轮询，第 5 次两平台同时 completed）：ubuntu 第 3 次
completed、windows 第 5 次。

逐平台看**完整步骤序列**（`jobs[].steps` 各 **20** 条，编号从 16 直接跳到 30——
17–29 是 `if` 未触发、不生成记录的步，所以条数不能与 workflow 的 `- name:` 相减）：

| 平台 | 状态 | 跳过的步 | 说明 |
| --- | --- | --- | --- |
| `foundation (ubuntu-24.04)` | success | 11 | 失败摘要注解步（仅失败时触发） |
| `foundation (windows-latest)` | success | 8 / 11 / 14 | 8 = Linux C++ protobuf 运行时；14 = 三语言契约套件（Linux 工具链） |

两平台都跑到的关键步全部 success：9 生成物无漂移核对、10 `check --profile full`（Rust 门控 +
前端门控）、12 诊断产物凭据扫描（W130）、13 schema 与 API 契约、15 `pnpm spec:validate`、
16 `openspec validate --archived`。**两平台红在不同步骤**这种情况本轮没有出现，也没有需要走
annotation 通道去取执行日志的地方。

两个平台的 `skipped` 集合与 W144 逐条相同——本轮既没改 workflow，也没新增 CI-only 步。

### 回填 run 的一次失败（如实登记，不改判据）

回填提交 **`2e3d049`** → run **`39114817313`**：ubuntu `success`，**windows `failure`**，红在
第 10 步 `Complete engineering foundation (portable steps + frontend workspace)`（随后 13–16
全部因前置失败被跳过；第 11 步失败摘要注解步这次触发了，执行日志正是从那 12 条 annotation 读到的）。
两条失败用例同在一个二进制里：

| 用例 | 位置 | 错误 |
| --- | --- | --- |
| `telemetry_is_buffered_when_persistence_fails_while_critical_is_reported` | `crates/nai-runtime/tests/composition.rs:412` | `StorageError { code: DeadlineExceeded, reason: "operation stopped" }` |
| `prepare_without_profile_is_rejected` | 同文件 `:124`（`create_workspace(..).unwrap()`） | 同上 |

`DeadlineExceeded` + `"operation stopped"` 是 `StorageError` 从 `nai_security::SecurityError`
原样搬 code 的那条路径（`nai-storage/src/lib.rs:105`），即**操作预算到期**；测试用的是
`BRINGUP_OPERATION = 30s`。

判它是**宿主瞬态而不是本轮回归**，四条：①该提交只改两份 docs（+25 −2），Rust 零变更；
②**同一份代码**的目标提交 run `39114038861` 在 windows 上是绿的；③本机
`cargo test -p nai-runtime --test composition` → **14 passed / 0 failed，整个文件 1.39s**——
离 30s 预算差 20 倍，说明不是预算定得太紧，而是那次 runner 卡了一下；④同一二进制里另外 12 条
同时通过（不是整文件系统性失败）。

处置：**不改代码、不放宽判据、不动 30s 预算**（宿主瞬态不得靠改被测系统的常量来吸收）。
按纪律这条只登记、不追绿：本段随一次 docs-only 提交入库，那一轮的结论另记；若后续再红在**同一对
用例**上、而本机仍在秒级通过，则升级为具名登记（§八 第 5 条）去查「预算 vs runner 卡顿」的归因，
而不是把常量往上抬。

## 八、具名登记（本轮发现但不做）

1. **`kind` 的正面路径仍需一次真实作业。** 探针对它是硬失败——这不是缺口被忽略，而是
   「没有样本的期望不写」的结果。第一次接上作业状态时，作者必须同时给出 202 + 可轮询 id 的期望。
2. **`blockedBy` 的类别表与执行侧的期望表是两处、由名字耦合。** 新增一个类别时，Rust 的 `match`
   兜底会硬失败（这是有意的），但「两处必须同时改」这件事本身没有判据能替你记住。
3. **W143 §八其余 4 项仍未接手**：2 份归档 proposal 结构性缺段、`13`/`15` 幽灵值、
   产物与扫描域的界线、真实补集「非空」从此不可断言。
4. **`tests/job-surface.json` 没有 schema。** 它的形状由两个读者各自假定（Node 侧规则 ③④ 会报
   「不是对象」「取值不在闭集」，Rust 侧会 `panic` 在 `blockedBy`/`split_once` 上）。一份 JSON Schema
   可以让失败更早、措辞更统一，但它会引入第三个读者与第三个形状定义，本轮不动。
5. **`BRINGUP_OPERATION = 30s` 在 CI 的 windows runner 上偶发不够（一次样本）。** 事实与否定证据见
   §七 末段：两条 `composition.rs` 用例同时撞到 `DeadlineExceeded`，而本机整文件 1.39s。今天**没有**
   足够样本判定「预算是紧的」还是「runner 卡了一下」，所以不做任何改动；下一次复现应当连同 runner
   的排队/冷启动信息一起记，再决定这条该落在常量、夹具还是 runner 选择上。**MUST NOT 直接抬常量**——
   那会把「谁该为慢负责」这个问题从台账里抹掉。

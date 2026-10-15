# W110：记忆读取面与遗忘落点接上控制面（H03/H04 控制面接线）

- 轮次：W110
- 日期：2026-10-15
- 变更：`openspec/changes/2026-10-15-w110-memory-control-surface/`
- ADR：[ADR 0088](../adr/0088-memory-control-surface.md)
- 前驱：[W109](2026-10-15-w109-memory-read-surface.md)（把读取面与遗忘阻断接上真实存储）

## 一、目标

给 W109 建成的两个 runtime 入口（`Composition::read_memory` / `Composition::forget_memory`）
接上**生产调用方**：本地控制面新增两个端点，经一处接缝调到装配根。

不是新增判定，也不是新增能力——本轮**零迁移、零契约字段改动、零 schema 改动**，是纯接线轮。
这与 W109 那轮「必须先造承载物、必须付一次迁移」形成对照。

## 二、缝在哪

### 2.1 直接证据（**现测**：从本轮改动前的状态实测，不是推断）

```
$ grep -rn "read_memory\|forget_memory\|write_memory" --include=*.rs crates apps \
    | grep -v "/tests/" | grep -v "src/tests/"
（零命中：只有测试在调）
```

三个入口在**生产代码**里全都是零调用方。W109 让 `read_records` / `forget` 有了第一处调用方，
但那个调用方自己（`Composition`）仍无调用者 ⇒ 链子的最上面一环是空的。

### 2.2 缝型归属

与 W103–W109 **连续第七轮同型**（判定面存在、执行体不存在）。**不同的一点**：
前几轮缺的是**执行体**，本轮执行体**已存在**，缺的是**调用方** ⇒ 本轮不需要造承载物，只需要接线。

### 2.3 分层决定了接缝的位置（不是偏好，是唯一可行形状）

`xtask/src/deps.rs` 禁止 shared 依赖 runtime（`nai-runtime` 反过来依赖 `nai-control-api`）。
故「控制面能调到装配根」只能是：**接缝在 shared 定义、实现在 runtime、由 app 注入**。

## 三、设计（详见 [ADR 0088](../adr/0088-memory-control-surface.md)）

九条决策，摘录最要紧的四条：

1. **接缝不带 `now_ms` 与 `MemoryPolicy`**——时间源只在一处（`utc_ms()`，实现端取）；
   策略由装配点给定、适配器持有。让路由挑策略会让 `viewer_memory_enabled` 被**答两次**。
2. **三态不丢维度**——`Ok` / `Rejected` / `Fault` 各有自己的 HTTP 形状；
   故障**复用**契约的 `ErrorCode`，不新造分类。
3. **作用域是控制面自己的绑定**，请求里的 `workspace_id` / `session_id` 一律忽略。
4. **读取端点走 `POST` + body**——本控制面有既有测试钉住「一律禁 query」，设计稿的 `GET` 服从它。

## 四、实现

| 位置 | 改动 |
|---|---|
| `crates/nai-control-api/src/lib.rs` | `MemoryControlResult<T>` 三态、`MemoryForgetRequest`、`trait MemorySurface`；`ControlApi` 增 `Option<Arc<dyn MemorySurface>>` 与 `with_memory`；两个路由 + 入参解析 + 三态映射 |
| `crates/nai-runtime/src/memory.rs` | `MemoryAdapter{root, policy}` 与 `impl MemorySurface for MemoryAdapter`（只转发） |
| `crates/nai-kernel/src/evaluate/memory.rs` | 给 `PurgeState` 补 `ALL`（与 `ForgetTrigger` / `MemoryKind` 对称，路由解析要遍历它） |
| `apps/engine/src/lib.rs` | 注入 `MemoryAdapter`；`CONTROL_PERMISSIONS` 增 `memory.read` / `memory.forget` |

**两个端点**：

| 端点 | 权限 | 要点 |
|---|---|---|
| `POST /api/v1/memory/read` | `memory.read` | 作用域由控制面填；`kinds` 缺 ⇒ 全部三类、显式 `[]` ⇒ 空集；`budget` 缺省 16、上限 256 |
| `POST /api/v1/memory/forget` | `memory.forget` | 先阻断、再判陈述；返回**生效中的**撤销时刻；记录不存在 ⇒ 具名失败 |

⚠️ **权限单独成权**（`memory.read` 与 `memory.forget` 分开）而不是复用既有宽权限：
目的是可以**只**吊销记忆访问而不连带会话读取。

## 五、测试与非空洞守卫

| 文件 | 总数 | 本轮新增 |
|---|---|---|
| `crates/nai-control-api/src/tests.rs` | **26** | **+11** |
| `crates/nai-runtime/tests/memory.rs` | **23** | **+5** |

**两个测试目标都已存在** ⇒ 由 `cargo test --workspace` 自动覆盖 ⇒ **无需** `engineering_checks` 登记。

控制面侧（真实 loopback + reqwest）：

- 后端未注入 ⇒ `503 CAPABILITY_NOT_READY`（两个端点各一）；无 token ⇒ `401`；有 `memory.read`
  但请求遗忘 ⇒ `403`，且**被拒的请求 MUST NOT 到达后端**（`forgets() == 0`）。
- **接线真的被调用**：`RecordingSurface` 记 `reads` / `forgets`，并留下路由构造出的**入参**
  （作用域 / 类别 / 配额）⇒ 「接线存在」由**调用发生了**证明，而不是由「响应是 200」证明。
- 请求级拒绝 ⇒ `422` + 判定自己的名字，且**响应体里没有 `records`**；故障 ⇒ `409`（≠ 422）。
- 入参各失败路径：`kinds` 非数组／未知类别名／非字符串元素／`budget` 越界／body 非 object（5 条）；
  遗忘侧 `record_id` 缺失／空白／非字符串、`trigger` 非字符串／未知、`purge_state` 未知、body 非 object（7 条）。
- `PurgeState::ALL` 与 `ForgetTrigger::ALL` 逐个**回环**（在册标签 MUST 全被认得）。
- 超长 body ⇒ `413`，且**未到达后端**。

runtime 侧（经接缝走一遍）：

- 播种两条（本会话 + 他会话）⇒ 他会话那条进 `refused(out_of_scope)` ⇒ **越作用域判据仍有对象**。
- 遗忘 ⇒ 阻断**真的生效**（再读该条 ⇒ `refused(revoked)`）；重复遗忘 ⇒ 撤销时刻**不变**。
- 记录不存在 ⇒ `Fault(InvalidArgument)`，MUST NOT 静默成功。
- **装配根忙 ⇒ `Fault(Conflict)`**（用**另一条线程持锁**触发——同线程 `try_lock` 在 Windows 上是
  未定义形状，MUST NOT 依赖它）。
- 请求级拒绝经接缝后仍是 `Rejected`（三态映射不丢维度）。

## 六、非空洞注入验证（真改文件 + 真跑 + 逐字节还原）

| 组 | 注入 | 结果 |
|---|---|---|
| I1 | 路由把请求级拒绝折成 `200 + []` | **1** 红 |
| I2 | 取数改回按 session 预筛 | **3** 红（判据**失去对象**；含新增的接缝用例） |
| I3 | `impl` 不调 `read_memory` 而直接返回空（**假契约**） | **3** 红 |

共 **7** 红，每组 ≥1，且每组 `sha256` **逐字节**还原 ✓（无注入残留）。

⚠️ **注入 MUST 用编辑工具做，MUST NOT 用 Python 读写往返**——Python 会把整文件换行符统一，
`sha256` 还原校验随之失效（W109 实测后写进记忆的硬规则，本轮全程遵守）。

I2 的信号最值得记：取数按 session 预筛后，越作用域用例红的是**「一条都没有」**而不是「少一条」——
守卫还在，但它守的对象已被上游拿走（**假阴性**，最危险的方向）。

## 七、门控与 validate

### 7.1 全量门控

`cargo xtask check --profile full` ⇒ **`EXIT=0`**（限并发 `RUST_TEST_THREADS=1`，见 7.2）：

- Rust：**95** 个测试目标 / **940 passed** / **0 failed**（`Running` 78 − 1 + `Doc-tests` 18 = 95）
- Node：13 个根 / **319** / **0**
- `error[` / `LNK1104` / OOM / `DeadlineExceeded` / `not ok` 计数**全为 0**

对照 W109（**924**）⇒ **+16** = `nai-control-api` **+11** + `tests/memory.rs` **+5**，与第五章逐项吻合。
两处新增都落在**既有测试目标内** ⇒ Rust 目标数与 Node 根数**均不变**。

### 7.2 本轮的一次即绿，但**不能据此推断**

本轮门控**一次即绿**（`EXIT=0`）。⚠️ 但按既定纪律，这不构成「无需限并发」的证据：
本轮在**写测试的过程中**已两次实测到宿主瞬态——`cargo test -p nai-runtime --test memory` 出现
`DeadlineExceeded`（`common/mod.rs:118`），与 W107／W109 记录的**同一用例同一症状**，
且相关文件一行未改 ⇒ 判据是「限并发下全绿」，不是「跑一次就绿」。故门控**直接**以限并发方式跑。

### 7.3 OpenSpec validate（归档前，**现测**）

| 校验 | 结果 |
|---|---|
| 变更级 `--strict` | **`is valid`** |
| `--specs --strict` | **43 passed / 0 failed** |
| `--all --strict` | **44 / 0** |
| `--changes` | **1 / 0** |

⚠️ 首版变更级校验**未通过**：一条 Requirement 里声明了两条行为（超过 500 字符上限被拦）。
按「一条 Requirement 只声明一条行为」拆开后通过。

### 7.4 归档与归档后复跑

归档 `openspec archive 2026-10-15-w110-memory-control-surface --yes` ⇒
`local-control-surface` **+4 added / ~0 / -0**。

归档后**立刻**跑的两条：

- `--archived` ⇒ **108 passed / 0 failed**（W109 为 107 ⇒ **+1**）
- `--all --strict` ⇒ **43 / 0**

⚠️ **`--archived` 首版报 1 红**：`1 incomplete task (39/40)`。原因是 §8 最后一条把
「归档 + 提交 + CI + 回填」捆成了一个 checkbox，而**回填需要 run id**、在归档那一刻不可能已完成
⇒ 那是一条**结构上无法在归档前勾**的任务。已就地订正：提交/CI/回填降级为**流程项（非 checkbox）**
（与 §9 同列），重跑 ⇒ 108/0。

归档改动了活规格 ⇒ 按纪律**复跑门控**：`EXIT=0`，计数与 §7.1 **逐项相同**
（95 目标 / 940 passed / 0 failed；Node 13 / 319 / 0；故障痕迹 0）。

## 八、归档、提交与 CI（**回填**）

待推送后回填：run id、`head_sha` 与推送 SHA 的核对、双平台矩阵结论。

## 九、如实披露、遗留与边界

### 9.1 如实披露

- **只到阻断，未到清除**：§42.4 要求「**先**阻断检索与读取，**再**异步清除副本」，本轮（与 W109 一样）
  只完成了前半。一条被遗忘的记录**仍在库里**，只是读不出来。端点响应里已如实写明这一点
  （`"safe_message"`：读取侧已阻断；副本清除未执行）。
- **`MemoryPolicy` 本轮无配置项**：来源是装配点，用默认值（观众长期记忆**默认关闭**）。
  这不是「配置缺失」，是「**有意**不在这轮扩张配置面」。
- **故障与判定分开是设计，不是兜底**：装配根忙报 `409 冲突`，MUST NOT 报成 `403`。

### 9.2 遗留

1. §42.4 **清除副本**一侧（需要副本清单与清理执行体）。
2. Pro 跨会话（§42.5：授权／审批／租约／设备身份不跨场继承）未覆盖。
3. 端点只到 loopback 控制面，**不含**桌面 UI 呈现。

### 9.3 边界（本轮**不做**的事）

- 不改 `read_records` / `forget` / `admit_write` 及任何 kernel 判定规则（**一行未改**）。
- 不改 `read_memory` / `forget_memory` 的语义（只新增调用方）。
- 不加迁移、不加契约字段、不改 schema。
- 不在控制面重跑 §42.3／§42.4 的任何一条判定。

**H03 / H04 的 registry `status` 仍为 `pending`**——本轮闭合的是「执行体 → 调用方」，
**不是验收达标**。

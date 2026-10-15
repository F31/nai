## 1 立项

- [x] 写 `proposal.md` / `design.md` / specs / tasks
- [x] `openspec validate 2026-10-15-w110-memory-control-surface --strict` 报 `is valid`
      （⚠️ 首版因「一条 Requirement 声明两条行为」被 >500 拦下 ⇒ 已按一条一行为拆开）

## 2 控制面接缝（定义在 shared，MUST NOT 依赖 runtime）

- [x] `MemoryControlResult<T>` 三态：`Ok(T)` / `Rejected(MemoryRejection)` / `Fault(ErrorCode)`
      （故障载体复用契约的 `ErrorCode`，**不新造**故障分类）
- [x] `MemoryForgetRequest`：`record_id` / `trigger` / `purge_state` /
      `third_party_retention` / `claims_fully_erased`
- [x] `trait MemorySurface { read(&MemoryQuery); forget(&MemoryForgetRequest) }`
      （`Send + Sync`；**不带** `now_ms`，时间源只在一处）
- [x] `ControlApi` 增 `Option<Arc<dyn MemorySurface>>` 注入点；**后端缺失 ⇒ `CAPABILITY_NOT_READY`**，
      MUST NOT 假成功（与 `approvals` 同形）
- [x] `cargo check -p nai-control-api`

## 3 两个路由

- [x] `POST /api/v1/memory/read` → permission `memory.read`：JSON body 解析
      `kinds` / `budget` / `authorization_ref` / `purpose` / `allow_approximate` / `viewer_scoped_id`
      ⚠️ **实现时改了设计**：原写 `GET` + query，但本控制面**一律禁止 query 参数**
      （有测试钉它，见「控制面一律禁 query」）⇒ 改 `POST` + body；规格已同步改 POST
- [x] `POST /api/v1/memory/forget` → permission `memory.forget`：JSON body 解析（复用 4KB 载荷上限）
- [x] **scope 由控制面填**（`workspace` / `session` 取自 `ControlApi` 自身绑定），
      请求里出现 `workspace_id` / `session_id` 一律**忽略**，MUST NOT 依它取数
- [x] `kinds` 空 ⇒ **全部三类**（「没给输入」≠「空集」）；未知类别名 ⇒ **具名失败**
- [x] `budget` 缺省 ⇒ 常量默认（16）；上限 256
- [x] 请求级拒绝 ⇒ **具名** 4xx，MUST NOT 折成 `200 + records: []`
- [x] 故障 ⇒ 与拒绝**不同的**状态码（MUST NOT 把存储故障/锁竞争记在判定名下）

## 4 runtime 实现接缝

- [x] `impl MemorySurface for MemoryAdapter`：`read` → `read_memory`、`forget` → `forget_memory`
      （**只转发**，MUST NOT 重跑 §42.3／§42.4）
      ⚠️ **实现时改了设计**：原写 `impl … for Composition`，但接缝不带 `MemoryPolicy` 与
      `now_ms`，二者 MUST 由一侧决定一次 ⇒ 新增 `MemoryAdapter{root, policy}` 适配器承载策略，
      时间由实现端取。`impl` 落在适配器上，转发到 `Composition` 的两个入口
- [x] `From<MemoryReadError>` / `From<MemoryForgetError>` 映射，三态**不丢维度**
- [x] `now_ms` 由实现端取（`utc_ms()`），MUST NOT 由控制面传
- [x] `MemoryPolicy` 由**装配点**给定并持有，路由 MUST NOT 挑（否则 `viewer_memory_enabled` 答两次）
- [x] 取不到装配根锁 ⇒ **冲突**（`ErrorCode::Conflict` ⇒ 409），MUST NOT 报成权限不足
- [x] `cargo check -p nai-runtime`

## 5 engine 注入

- [x] 装配时把 `Arc<Mutex<Composition>>` 包成适配器注入 `ControlApi`
- [x] `CONTROL_PERMISSIONS` 增 `memory.read` / `memory.forget`
- [x] `cargo check -p nai-engine`

## 6 测试与非空洞守卫

- [x] `crates/nai-control-api/src/tests.rs`：真实 loopback + reqwest
      —— 后端缺失 ⇒ `CAPABILITY_NOT_READY`；无 token ⇒ 401；无权限 ⇒ 403（**+11** 项，共 26）
- [x] **接缝真的被调用**：用一个**记录调用的测试实现**（`RecordingSurface` 记 `reads`/`forgets`
      并留下路由构造出的**入参**），MUST NOT 用恒真桩（否则「接线」可以是装饰——R1）
- [x] `crates/nai-runtime/tests/memory.rs`：播种**两条**（本会话 + 他会话）⇒ 经接缝读
      ⇒ 他会话那条进 `refused(out_of_scope)`（**证明越作用域判据仍有对象**——R2，**+5** 项，共 23）
- [x] 遗忘 ⇒ 阻断生效 + 再读该条 `revoked`；重复遗忘 ⇒ 撤销时刻**不变**
      （⚠️ 中间那 20ms 等待不是装饰：同毫秒则「覆盖写」会与原值碰巧相等而绿着过）
- [x] 三态各一例：`Ok` / `Rejected` / `Fault`（映射不丢维度——R3）
      「忙」用**另一条线程持锁**触发（同线程 `try_lock` 在 Windows 上是未定义形状，MUST NOT 依赖）
- [x] 请求解析的每条失败路径各一例；`PurgeState` / `ForgetTrigger` 的解析用**穷尽 match**
      （在册标签逐个回环：加变体而路由没跟上 ⇒ 红）
- [x] 顺带给 `PurgeState` 补 `ALL`（与 `ForgetTrigger` / `MemoryKind` 对称，解析要遍历它）

## 7 非空洞注入验证（真改文件 + 真跑 + 逐字节还原）

- [x] I1 路由把请求级拒绝折成 `200 + []` ⇒ **1** 红
- [x] I2 取数改回按 session 预筛 ⇒ **3** 红（判据**失去对象**；含新增的接缝用例）
- [x] I3 `impl` 不调 `read_memory` 而直接返回空 ⇒ **3** 红（**假契约**）
- [x] 每组 ≥1 红，逐组 `sha256` **逐字节**还原（共 **7** 红）
      ⚠️ **注入 MUST 用编辑工具做，MUST NOT 用 Python 读写往返**（会统一换行符使校验失效）

## 8 门控、文档与闭环

- [x] `cargo xtask check --profile full` ⇒ **`EXIT=0`**（含 `cargo fmt --all -- --check` 干净）
      —— 95 目标 / **940 passed** / 0 failed；Node 13 / 319 / 0；故障痕迹全 0
      ⚠️ **直接以限并发跑**：写测试过程中两次实测到宿主 `DeadlineExceeded`
      （与 W107/W109 **同一用例同一症状**，相关文件一行未改）
- [x] 新增 `docs/adr/0088-memory-control-surface.md`
- [x] 新增 `docs/reports/2026-10-15-w110-memory-control-surface.md`
- [x] 活文档**就地订正**（W109 写的「控制面接线未建」已过期）：`docs/runtime.md`（新增 W110 一节）、
      `docs/kernel.md`（「仍未接线」清单里划掉控制面接线）；`docs/development-plan.md`
      是**已提交的历史叙述** ⇒ **追加** W110 段（不就地改 W109 的原文）。`docs/control.md` 不存在
- [x] `tests/registry.yaml`：H03 / H04 notes 追加 W110 段，**`status` 仍 `pending`**
      ⚠️ **YAML 坑（本轮实测）**：notes 是**双引号标量** ⇒ 内嵌的 `"` 会截断、`\|` 是非法转义
      ⇒ 命令示例 MUST 避开裸引号与管道（`js-yaml` 现测解析通过）
- [x] `openspec archive 2026-10-15-w110-memory-control-surface --yes` ⇒ `local-control-surface`
      **+4 added**；归档后**立刻**跑 `--archived`
      ⚠️ **首版把「归档 + 提交 + CI + 回填」捆成一条 checkbox** ⇒ 归档后 `--archived` 报
      **1 incomplete task（39/40）**：回填需要 run id，**在归档那一刻不可能已完成**，
      把它写成 checkbox 就是一条**结构上无法在归档前勾**的任务 ⇒ 已把提交/CI/回填降级为流程项（§9）

## 9 流程（**非 checkbox**）

- 一次 commit + push → **单次**低频 CI 观测（三件齐：唯一 `cb` + `no-cache` + 完整 40 位 `head_sha`）
  → 回填报告 §八；回填提交**也观测一次**（先例 run 109 曾 `failure` ⇒ 不赌）
- 自主裁定、不询问；计数**现测**不推断；宿主瞬态不改代码；判定面 ≠ 验收达标。

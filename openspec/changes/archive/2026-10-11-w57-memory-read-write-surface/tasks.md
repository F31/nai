# 任务（w57 记忆读写面）

## 0 立项

- [x] 0.1 建立 `openspec/changes/w57-memory-read-write-surface/`（`.openspec.yaml` / `proposal.md` /
      `design.md` / `specs/agent-harness/spec.md` / `tasks.md`）
- [x] 0.2 `openspec validate w57-memory-read-write-surface --strict` 通过（**「Change is valid」**）
- [x] 0.3 实测四条需求正文长度为 235／300／191／231 字符（**均 ≤ 500**）——W56 的教训：
      >500 只在归档后的 `specs/` 报错，变更层的 `validate --strict` 看不出来

## 1 实现记忆读写面（`crates/nai-kernel/src/evaluate/memory.rs`，只增不改）

- [x] 1.1 写入面（H03）：`WriteSource`、`ClaimKind`、`MemoryCandidate`、`admit_write`
- [x] 1.2 读取面（H04）：`MemoryScope`、`MemoryQuery`、`ScopedRecord`、`MemoryReadOutcome`、`read_records`
      ——按「作用域 → 时效 → 撤销 → 权限 → 配额」固定链，超配额按「场次 > 工作 > 观众」裁剪且裁剪**可见**
- [x] 1.3 遗忘链（§42.4）：`ForgetTrigger`、`PurgeState`、`ForgetOutcome`、`forget`
- [x] 1.4 四操作（§42.6）：`MemoryOperation` 及其 `parse`
- [x] 1.5 新增具名拒绝类型 `MemoryRejection`（含 `label()`，**23** 个）；**既有四个函数与 `MemoryPolicy` 一字不改**
- [x] 1.6 实测「只增不改」：`git diff --numstat` = **+748 / −4**，三处 hunk 为
      模块文档注释重写（`-4/+21`）、新增 `SEMANTIC_KIND`（`+2`）、追加新面（`+725`）；
      **既有四个函数与 `MemoryPolicy` 的函数体零改动**（唯一的 −4 是文件头注释）

## 2 集成测试（`crates/nai-kernel/tests/memory.rs`）

- [x] 2.1 逐条构造**每一个** `MemoryRejection`（无遗漏）
- [x] 2.2 「拒绝名并集**恰为** 23 条」断言——既证每条可构造，也防未登记的名字
- [x] 2.3 正向不变量：合规候选的 `admit_write`／`read_records`／`forget` 零拒绝
- [x] 2.4 否定式不变量：未声明 ⇒ 不误判；未播出 ≠ 过期；未请求的类别不算「被拒」；观众默认关闭时**不写画像**
- [x] 2.5 **非空洞守卫**：注入违规（工具返回当事实、跨作用域、近似顶替、超配额、宣称擦除）必须分别被抓到；
      并含**顺序断言**（同时越作用域又过期 ⇒ 报 `out_of_scope`）与**优先级断言**
      （budget=2 的四条 `w1,v1,s1,w2`：按优先级保 `s1,w1`，按输入序则会保 `w1,v1` ⇒ 能区分）
- [x] 2.6 实测：**21 passed / 0 failed**；lib 单测 **43 passed / 0 failed**；
      `clippy -p nai-kernel --all-targets -- -D warnings` **EXIT=0**；`cargo fmt --all --check` **EXIT=0**

## 3 验收登记与文档

- [x] 3.1 `tests/registry.yaml`：H03/H04 补 `evidence` 与 W57 notes；**状态保持 `pending`** 并说明理由
      （判据是运行时行为，真实记忆子系统未建；沿用 A12／O01／O05 与 W32–W36 先例）
- [x] 3.2 `docs/kernel.md`：新增「W57 记忆读写面」节（来源门槛、读取固定链、删除链、四操作、边界）
- [x] 3.3 `docs/development-plan.md`：追加 W57 轮次记录，并把「下一主线」的 W56 轨道描述更新为「W57 已落下第一个判定面」
- [x] 3.4 `docs/reports/2026-10-11-w57-memory-read-write-surface.md`

## 4 闭环

- [x] 4.1 预热两个测试目标后跑 `cargo xtask check --profile full`：
      **EXIT=0**；测试目标 **87（69 `Running` + 18 `Doc-tests`）**、**761 passed / 0 failed**
      （W56 为 86 / 740 ⇒ +1 目标、+21 项 = 本轮集成测试）
- [x] 4.2 `cargo xtask check-deps` = **19** workspace packages（不变）
- [x] 4.3 `openspec validate --all --strict` = **43/43**（42 specs + 1 活动变更）
- [x] 4.4 全部 tasks 勾选后 `openspec archive -y w57-memory-read-write-surface`
      ——`Task status: ✓ Complete`；`agent-harness: +4 added`；归档为 `2026-10-11-w57-memory-read-write-surface`
- [x] 4.5 `openspec validate --all --strict`（归档后）= **42/42**（specs；活动变更 0）；
      `--all --strict --archived` = **62/62**（**只含已归档变更、不含 specs**）——
      W56 教训：两条口径**互不替代**，故两条都跑并分别记录
- [x] 4.6 一次 commit + push（提交信息用 Write 落 UTF-8 文件，避免 `\u` 字面转义）
- [x] 4.7 用缓存破坏参数匿名复核 CI：run **`38151252071`** 的 `head_sha` = 推送的 `4a35feb`（**一致**）、
      `conclusion: success`、`attempt: 1`；两 job（`foundation (ubuntu-24.04)` / `foundation (windows-latest)`）
      **均 success**、各 26 步、**无失败步骤**（该结果推送后测得，故本行随一次 docs-only 提交回填）

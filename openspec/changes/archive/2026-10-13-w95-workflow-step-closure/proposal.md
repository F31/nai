# w95：CI 工作流的每个 step 必须具名在册，并声明本机门控是否覆盖它

## Why

`.github/workflows/foundation.yml` 是**判定面**的一部分：它跑了 **9 个本机门控没有的判定步**
（`schema --check`、8 个 `cargo xtask test --suite …`（含 Linux-only 的 `contracts` 套件、
该套件含本机跑不了的 `descriptor.test.mjs`）、`pnpm spec:validate`、`openspec validate --archived`）。
而**这个集合没有任何在册台账**。

两个探针实证（本轮实测，改前/改后都真跑）：

| 探针 | 改动 | 本机门控（schema 30 / platform 22 / registry 52） |
| --- | --- | --- |
| 加法 | 往 `foundation.yml` 插一个 step（`run: node -e "…"`） | **全绿、零信号** |
| 减法 | 删掉 `Check generated contracts …`（`cargo xtask schema --check`）这个判定步 | **全绿**——一个判定**静默消失** |

Rust 侧不读 workflow（`grep -rn foundation.yml crates/ xtask/` 为空）⇒ 没有别的守卫。

⇒ 「哪些判定只在 CI 跑」目前**只存在于注释里**：`tests/contracts/schema.test.mjs` 里那句
「**本地门控从不读它**（唯一读它的 `generate.mjs` 只在 CI-only 的 `cargo xtask schema --check` 里跑，
而本机对该步 `EBUSY`）」是一句**解释**，不是判据。这与 W84 的「测试文件静默不跑」同族——
**判定的消失没有信号**：`cargo xtask check --profile full` 照旧 `EXIT=0`、目标数不变、用例数不变。

## What Changes

1. **新增 `scripts/contracts/workflow-steps.mjs`**：给 `.github/workflows/` 补**文件层 + 步层**两层闭世界。
   - **文件层**：`.github/workflows/` 下**已跟踪**的 workflow 文件 MUST 逐份在册，在册的每份 MUST 已被跟踪
     （两方向）。新增一份 workflow 而不登记 ⇒ 具名失败。
   - **步层**：每份 workflow 的**每一个 step** MUST 有且只有一条登记。步的**身份**由解析得出
     （有 `name` 用 `name`，否则用 `uses` 或 `run` 首行），MUST 唯一。
   - **四类闭集** `STEP_KINDS`：`infra`（环境/工具链准备，不产生判定结论）、`local`（本机门控同样执行）、
     `ci-only`（只有 CI 执行）、`diagnostic`（仅在失败时跑的诊断通道）。
   - **`local` 必须付代价**：每条 SHALL 引出一处**本机门控落点**（`{file, marker}`），且该 marker
     MUST 在门控源里**逐字存在**——否则「本机也跑」是一句空声明（W75「已声明 ≠ 已执行」）。
   - **`ci-only` 必须付代价**：每条 SHALL 给出**成因**，成因 SHALL 取自一个**闭集**
     （`host-subprocess`／`compiled-artifact`／`platform-toolchain`／`not-in-profile`）。
     域外成因 ⇒ 具名失败（W90：一个未知成因是一个标签，不是一个理由）。
   - **双向**：`local` MUST 有落点、`ci-only` MUST NOT 有落点（触发条件只由「字段存在」决定，W83）。
     `diagnostic` MUST 对应一个带 `if` 的 step（诊断通道不产生独立判定）。
2. **新增 `tests/contracts/workflow.test.mjs`**：承载上述判据的测试，并**在 `xtask/src/lib.rs` 的
   `engineering_checks` 登记**——同时**在 `scripts/contracts/test-reachability.mjs` 的
   `TEST_REGISTRATION_SITES` 登记站点**（否则 W84 的闭世界会立刻报「这个测试文件没人跑」，
   这正是它该有的行为）。
3. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。

## Impact

- 零 Rust **行为**改动（`xtask/src/lib.rs` 只加一行登记，不改任何判定逻辑）；零新依赖；零 workspace 包增减。
- 新增 1 个 Node 测试文件 ⇒ 门控目标数不变（91），Node 套件由 5 个变 6 个。
- **一处新增的耦合**：`test-reachability` 的站点表与 `workflow-steps` 的登记表都以
  `.github/workflows/foundation.yml` 为 host，但**断言的单元不同**（前者是「**测试文件**有没有被接上电」，
  后者是「**step** 有没有被分类在册」）。两者的**独有覆盖**在报告中逐条核对，MUST NOT 互相顶替。
- **明确不做**：不验「CI 真的跑了某步」（那要读 CI 的运行结果，不是静态判据的事）；不验
  「`ci-only` 的成因是否成立」（那是语义，仍由人读）；不改 `foundation.yml` 一个字符。

# w84：测试文件的可达性闭世界

## Why

门控靠**逐条列举**（`engineering_checks` 里手写 `node --test <path>`）加**两个 glob**
（`tests/ui/*.test.mjs`、`apps/desktop/tests/*.test.mjs`）把 Node 测试接上电。列举的盲区
恰好是**它的补集**：往仓库里放一个 `tests/kernel/foo.test.mjs`，没有任何地方会注意到它。

W84 探针实测：一个**已在 `.git/index` 里**、含 `assert.fail(...)` 的测试文件放进 `tests/kernel/`，
`cargo xtask check --profile portable` 仍然 **`EXIT=0`**、目标数不变（91）、用例数不变
（860 passed / 0 failed），且该文件的用例名在门控日志里**零出现**。⇒ 这不是「未登记 = 不跑」，
而是**静默的不跑**：连「忽略了一个文件」都没有任何信号。

这与 W80–W83 的洞同形——**「按清单做」的守卫，盲区是清单的补集**。补法也同形：给
「哪些测试文件在跑」补一条**闭世界**。

## What Changes

- 新增 `scripts/contracts/test-reachability.mjs`：
  - 导出 `TEST_EXTENSIONS`（`['.test.mjs']`）、`TEST_TIERS`、`TEST_REGISTRATION_SITES`
    （**七个站点**，逐字写死 `{id, host, marker, base, tier}`）、`LOCALLY_UNREACHABLE`（台账，当前为空）；
  - **覆盖集由 marker 原文解析得出**（`resolveSite`），MUST NOT 另行声明；
  - **假契约**：每个站点的 `marker` MUST 在其 `host` 里**逐字存在**；
  - **非空洞**：站点 MUST 至少覆盖一个**在册**文件；
  - **扩展名层闭世界**：每个「看起来像测试」的在册文件，扩展名 MUST ∈ `TEST_EXTENSIONS`；
  - **闭世界**：在册测试文件 MUST ∈ 覆盖集 ∪ 台账；**两集不相交**（台账条目若已可达即失效）；
  - **覆盖集非空**（与「台账为空时两集不相交退化成恒真」成对）。
- `tests/registry.test.mjs` 增补判据并接入 `readTrackedPaths`（W75 的纯 JS git-index 设施）。

不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边。

## Impact

- 新增 capability 差量：`spec-driven-delivery`（一条 ADDED Requirement）。
- 新增 ADR 0062。
- 新增活文档小节（`AGENTS.md`、`docs/development-plan.md`）。
- 台账数据一字未改；基线保持全绿（探针文件在归档前删除）。

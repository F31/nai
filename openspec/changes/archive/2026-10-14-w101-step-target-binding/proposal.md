# w101：在册判定步的**目标绑定**（这一步跑的到底是哪个目标）必须由 `run` 解析兑付

## Why

W95 给 workflow 的 step 集补了**步层闭世界**：身份由解析得出、与在册清单双向一致、`kind` 落在闭集里、`local` 要引出真的门控落点。W97 管住了包集合与「CI 调用哪些命令」。W100 管住了执行配置面（跑不跑／在哪儿跑／失败算不算数）。

但一条在册判定步**跑的是哪个目标**从来没被读过：在册条目只声明 `{host, identity, kind}`——**身份是名字，不是目标**。实测（真改 `.github/workflows/foundation.yml`、真跑 9 个 portable 套件、逐字节还原、`sha256` 一致）：W1 把 `hosts` 步与 `session` 步的 `--suite` 目标互换 ⇒ 零信号；W2 `scheduler` 步改跑 `--suite provider`（scheduler 在 CI 消失、provider 跑两遍）⇒ 零信号；W3 `knowledge` 步改跑 `--suite storage` ⇒ 零信号；**W4 把 provider 判定步整条换成 `cargo xtask check --secrets`（目标性质整个换掉）⇒ 零信号**。

W5/W6 确实红了，但红的是 W97 的一条**越界断言**（钉 `check --profile full` 与 `schema --check` 两个具体串）——它与 W97 自己的边界声明「不验签名的取值」相矛盾，既**假阳性**（合法改 profile 即红）又**假阴性**（换同形状目标看不见）。

⇒ 母题「声明 MUST 付代价」的第十次出现。

## What Changes

1. **新增 `scripts/contracts/step-target.mjs`**：目标形状闭集 `TARGET_SHAPES` + 解析器 `targetsOf(run)` + 主判据 `stepTargetErrors({host, workflowText, registered})`。
   - `targetsOf` 把一段 `run` 文本**解析**成目标列表 `[{shape, value}]`：`xtask`（`cargo xtask <签名>`）、`node-test`（`node --test <文件…>`）、`pnpm`（`pnpm <脚本>`，`pnpm install` 除外——那是环境准备，不是判定）。xtask 那一支**复用** W97 的 `ciXtaskCommands`（**同一件事 MUST NOT 两处定义**）。
   - **R1 绑定双向**：每个 `local`／`ci-only` 条目 MUST 声明 `target`；解析出的目标集合 MUST 与声明的**双向相等**（W1–W4 的守卫）。
   - **R2 角色与目标一致**：`infra`／`diagnostic` MUST NOT 声明 `target`，且其 `run` MUST 解析出**零个**目标——把一条判定塞进「环境准备」是整套检查的后门（W95 只看首行 + 5 个正则，本轮改成**整段解析 + 形状闭集**）。
   - **R3 目标形状闭世界**：`shape` 域外 ⇒ 具名失败；不可解析的输入 / 缺失输入 / 空注册表 ⇒ 具名失败，MUST NOT 静默降级。
2. **`WORKFLOW_STEPS` 的 13 个判定条目补 `target`**（gate 1 + 7 个 suite + 契约套件 1 + `schema --check` 1 + `node --test` 1 + `pnpm` 2）；`infra`（7）与 `diagnostic`（1）**不加字段**。`kind`／`identity`／`localEvidence`／`reason` 一个字符都不动。
3. **订正 W97 的越界断言**：`tests/contracts/gate-coverage.test.mjs` 那条钉具体串的断言改为**形状级**（「至少有一条 `check --profile …`」），把「取值绑定」交回 W101——按对象性质分，不按章号圈。
4. **新增 `tests/contracts/step-target.test.mjs`**：合成世界 + 真实仓库 + 逐条注入 + 非空洞 + 反向不红；登记进 `engineering_checks`、`test-reachability.mjs` 站点表（13 → 14）与 `tests/registry.test.mjs` 的逐字清单。
5. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。

## Impact

- **零产品代码改动**：`.github/workflows/foundation.yml` **一个字符都不动**（本轮只加判据，并把 W97 的一条断言从「钉取值」收回到「钉形状」）。
- `tests/`（`tests/ui` 除外）与 `apps/desktop/tests` 下的 Node 套件 **11 → 12**（portable 面用例 **229 → 245**）；
  登记站点 **13 → 14**；`engineering_checks` 多一行；零新依赖。
- **明确不做**：不验目标**能跑通**；不实现 shell 语义（变量展开、条件分支）；不判 `run` 里的脚本注释（W97 的既有边界，本轮沿用）；不判目标在矩阵各腿上的等价性（W100 已管执行面）。
- **如实披露**：`target` 钉的是**目标的签名串**（含 `--profile full` 这类取值）——这与 W96 的「签名不含值」口径**不冲突**：W96 管「入口的形状」，W101 管「这一步实际调用的是哪一个入口实例」。两者对象不同，见 design 决定 2。

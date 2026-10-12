# 提案（w74 消费守卫的诚实性：扫描集、派生来源与生成清单）

## Why

本轮从 W73 自己写下的消费守卫出发做**复核**（「先核实再裁定」），核实**推翻了 W73 三条声称中的两条**，
并牵出另外两条缺陷。四条全部有可复现的实测落点：

1. **F1 —— 非空洞断言自我满足。** W73 守卫写着「三条派生来源各自都被行使」，其实**只由 Ajv 清单**满足。
   实测关掉「路径字面量」来源后：**消费 13 份、`fail 0`**；那条来源**零独有覆盖**——它找到的
   `platform-matrix/platforms` 与 `tests/registry` 都也在 Ajv 清单里。注入 V1（把
   `for (const pattern of [rust, literal])` 改成 `[rust]`）后守卫**照旧 13/13 绿**
   ⇒ 整条来源可被删除而守卫毫无反应：它不是守卫，是装饰。
2. **F2 —— 扫描的是工作区，不是仓库（最严重）。** 放一个**未跟踪**的 `.tmp-fake-consumer.mjs`
   （内含一句 schema 路径字面量），一个真孤儿立刻被洗绿：`# tests 13 / # pass 13 / # fail 0`。
   「有一份 schema 没人读」是本守卫**唯一**要抓的东西，而一句临时脚本就能让它失效。
3. **F3 —— 两个缺陷互相掩盖。** `CODE_EXTENSIONS` 缺 `.yaml`/`.js`，而
   `stripCommentLines(text, file.endsWith('.yml'))` 只对 `.yml` 用 `#` 注释语法。
   **唯一暴露该缺陷的文件**（`tests/registry.yaml`，恰好正是 W73 写下「注释里的路径不算消费」
   那句注释的地方）**因为扩展名叫 `.yaml` 而根本不在扫描集内**——两个缺陷各自都在对方的盲区里。
4. **F4 —— 「有哪些 schema」有第二处清单。** `scripts/contracts/generate.mjs` 的 `jsonSources`
   （10 条）与 `scripts/contracts/validation.mjs` 的 `AJV_SOURCES`（12 条）是两份互不校验的清单；
   生成物 `packages/contracts-ts/src/generated/config/*.ts` **已入库**，所以「生成面 ⊆ 校验面」
   这条**可以判定却无人判**。

外加持性子进程约束：本机 node 对 `git`（裸名与绝对路径）、`process.execPath`、`cmd.exe`、`sh`
**一律 EBUSY** ⇒ 宿主禁止 node 派生**任何**子进程 ⇒「用 `git ls-files` 取仓库文件集」这条路**不可行**，
F2 只能靠**结构性收敛**处理（见 `design.md` 决策 1 与残余风险）。

## What Changes

1. **扫描集的形状改成结构性的**：从仓库根递归、**跳过所有以 `.` 开头的目录**，再加一份**具名排除**
   （恰 4 条：`node_modules`/`target`/`graphify-out`/`dist`）；每条具名排除 MUST 被 `.gitignore` 忽略
   ——**「它不是仓库的一部分」就是那条理由**，`.gitignore` 就是它的证据。加断言：**跳 dot 段与
   不跳 dot 段的消费集合 MUST 相同**（实测差集只有一个 `.github/workflows/foundation.yml`，
   且它不含任何 schema 路径）。
2. **语言闭包**：可扫描扩展名由 `CODE_LANGUAGES`（ext → 注释语言）**派生**，
   `CODE_EXTENSIONS = [...CODE_LANGUAGES.keys()]` ⇒「新增扩展名必须同时声明注释语言」变成
   **结构上不可能漏**的事，而不再是「靠人记得改两处」。
3. **每条派生来源 MUST 自负其责**：来源收为**两条**（Ajv 清单、Rust `include_str!`），
   并把「各自都被行使」换成**独有覆盖**判据——Ajv 独有 MUST 含 `schemas/tests/registry.schema.json`
   与 `schemas/platform-matrix/platforms.schema.json`，Rust 独有 MUST 恰为
   `schemas/config/upgrade-package.schema.json`。⇒ **零独有覆盖的来源 MUST 被移除或合并**；
   路径字面量来源因此被移除（它零独有覆盖，且是唯一会读任意扩展名文件的来源）。
4. **生成面与校验面共用一份清单**：`validation.mjs` 导出 `TS_GENERATED_SOURCES`；
   `generate.mjs` **删除硬编码清单**改为导入它。加断言：TS 生成面 MUST ⊆ Ajv 校验面、
   basename MUST 互异（否则生成物互相覆盖）、生成目录里的 `.ts` MUST 与清单**双向相等**。
5. **规格**：`versioned-contracts`（MODIFIED 一处 + ADDED 两处）。

### 不改什么

- **不改基线正文**：`… V3.10.md` 零改动。
- **不改 Rust 行为**，**不新增 xtask 步骤**（新断言写在既有 `tests/contracts/schema.test.mjs` 里，
  该文件已经在 `engineering_checks` 清单上）。
- **不改任何 schema 文件**（14 份原样）、**不改 `tests/registry.yaml` 的任何 `acceptance` 条目**。
- **不改生成物**：`packages/contracts-ts/src/generated/` 内容零漂移——本轮只把「谁生成它」的
  清单来源收成一份。

## Capabilities

- `versioned-contracts`（MODIFIED）：`Every schema under schemas/ is enforced or named` ——
  补三条约束：扫描集 MUST 是结构性规则 + 付得起理由的具名排除；判据 MUST 只依赖仓库内容
  （MUST NOT 因工作区里的未跟踪文件改变结论）；判据解析注释语法时 MUST 覆盖每个可扫描扩展名。
- `versioned-contracts`（ADDED）：**每条派生来源 MUST 找到至少一份只有它能找到的 schema**。
- `versioned-contracts`（ADDED）：**生成器与校验器读同一份 schema 清单**。

## Impact

- 交付：`scripts/contracts/{validation,generate}.mjs`、`tests/contracts/schema.test.mjs`、
  `docs/adr/0052-*.md`、`versioned-contracts` 规格差量、活文档与报告。
- 验证：每条新判据各自注入（可构造的坏实现被逐条抓住）；`cargo xtask check --profile full`；
  两条 `openspec validate` 口径。
- **门控路径不变**：新断言经既有的 `tests/contracts/schema.test.mjs` 自动进入 `portable` 与 `full`。

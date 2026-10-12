# 提案（w73 注册表形状契约接电与台账存活期）

## Why

本轮从 W72 遗留第 2 条（§56.2「紧接着」vs「跨轮次收录」未裁定）出发，核实后**改了计划**：

1. **三条既有规格要求以「schema 校验」为前提，而那句校验根本没跑。**
   `openspec/specs/admission-registry/spec.md` 写着「schema 校验 SHALL 拒绝…」「MUST NOT 出现
   『注册表写得进去、schema 认不出来』的状态」，而 `schemas/tests/registry.schema.json`
   **不被任何执行体消费**。实测把注册表喂给它：**只报一处错**——W72 新增的 `baseline_deviations`
   没被声明（其余 88 条、六条发布线、映射段全部通过）。**一次合法的新增就把一个无人检查的契约变成了假契约。**
2. **`schemas/` 下的孤儿不止一个**（`tests/registry`、`rag-golden/case-set`），而「有没有人读它」当前**没有守卫**。
3. **「紧接着」在历史上从未按字面发生过，且字面读法不可行**（基线不可变 + 就地订正 MUST 1:1 行替换
   ⇒ 改 §57.3 只能发生在一次专门的升基线轮次里）。三条实例（A12–A14 / 爬坡 / B07）**全部**在下一次
   升基线轮次被收录，**零跳过** ⇒ 可判定形式是**台账条目 MUST NOT 跨基线版本存活**。

## What Changes

1. **注册表接电**：`schemas/tests/registry.schema.json` 补 `baseline_deviations` 定义；
   `scripts/contracts/validation.mjs` 导出 `AJV_SOURCES` 并把注册表列入 ⇒ `tests/registry.yaml`
   MUST 满足其 schema（未知键一律拒绝）。
2. **形状层与语义层一致**：schema 与 `registry.mjs` 的三处重叠常量（id 家族 pattern、发布线枚举、
   状态枚举）MUST 逐字相同，否则失败并指名是哪一处。
3. **台账存活期**：条目扩为 `{id, since_baseline, registered, carried_at?, reason}`；
   不变式 **每条的 `since_baseline` MUST 等于当前基线版本**；跨版只有两种处置——**收录**（删条目）
   或**显式结转**（改版本 + 记 `carried_at` + 非空理由）。
4. **schema 消费守卫**：`schemas/**/*.schema.json` 的全集 MUST 等于「被消费的（从源码派生）」
   ∪「具名例外清单（每条附理由）」；本轮具名例外恰一条（`rag-golden/case-set`）。
5. **规格**：`admission-registry`（MODIFIED 两处 + ADDED 一处）、
   `versioned-contracts`（ADDED 一处）、`spec-driven-delivery`（ADDED 一处）。

### 不改什么

- **不改基线正文**：`… V3.10.md` 零改动。
- **不改 `tests/registry.yaml` 的任何 `acceptance` 条目**：88 条、六条发布线、全部 `status` 原样；台账**仍为空**。
- **不改 Rust 行为**，**不新增 xtask 步骤**（`engineering_checks` 已含两个 Node 测试文件）。
- **不改生成物**：能力目录与 `schemas/{config,plugin}` 契约零漂移。
- **不删** `rag-golden/case-set.schema.json`、**不删** `COMPLETE_FAMILIES`。

## Capabilities

- `admission-registry`（MODIFIED）：注册表 MUST 被其 JSON Schema 校验；对账需求的台账条目形状扩为
  含 `since_baseline`/`carried_at` 并加跨版不变式。
- `admission-registry`（ADDED）：**注册表的形状层与语义层只有一份定义**。
- `versioned-contracts`（ADDED）：**`schemas/` 下每份 schema 要么被执行体消费，要么被具名声明**。
- `spec-driven-delivery`（ADDED）：**已登记的偏差不跨基线版本存活**（「紧接着」的可判定形式）。

## Impact

- 交付：`schemas/tests/registry.schema.json`、`scripts/contracts/{validation,baseline,registry}.mjs`、
  `tests/registry.test.mjs`、`tests/contracts/schema.test.mjs`、`tests/registry.yaml`（台账段注释）、
  `docs/adr/0051-*.md`、三份规格差量、活文档与报告。
- 验证：注入验证逐条守卫分别开火；`cargo xtask check --profile full`；两条 `openspec validate` 口径。
- 门控路径不变：新守卫经 `engineering_checks` 自动进入 `portable` 与 `full`。

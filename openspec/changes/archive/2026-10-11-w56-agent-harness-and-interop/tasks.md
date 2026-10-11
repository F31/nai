# 任务（w56 智能体 Harness 与开放互操作）

## 0 立项

- [x] 0.1 建立 `openspec/changes/w56-agent-harness-and-interop/`（`.openspec.yaml` / `proposal.md` /
      `design.md` / `specs/agent-harness/spec.md` / `specs/admission-registry/spec.md` / `tasks.md`）
- [x] 0.2 `openspec validate w56-agent-harness-and-interop --strict` 通过

## 1 基线文档 V3.8

- [x] 1.1 复制 V3.7 为 `docs/直播数字人 + 智能体系统技术方案 V3.8.md`
- [x] 1.2 8 行 1:1 行替换（标题、副标题、§0.2 指引、§1.4 两行、§16 前导、§17.3 两行）；
      以 `diff` 实测「4 个 hunk、8 行、行号一一对应、总行数不变」——
      实测：前 1875 行仅行号 `[1,3,29,56,57,624,698,699]` 不同，共 **8 行**
- [x] 1.3 追加第四部分 §40–§48：Harness 总览（含六层映射与 INV-27–INV-32）、上下文装配与预算、
      记忆子系统、意图识别与路由衔接、工具调用与技能编排、语义缓存、开放互操作（MCP/A2A/连接器注册）、
      观测评估与门控（含 W56–W64）、V3.8 版本说明与汇总索引（含 T-20–T-24、H01–H18）
- [x] 1.4 实测第四部分规模与 §1–§39 的差异规模，写入 ADR 与本变更（不凭印象）：
      V3.7 = 1875 行 / 151,878 字节，V3.8 = 2366 行 / 193,438 字节 ⇒ 新增 **491 行 / 41,560 字节**

## 2 ADR 与契约

- [x] 2.1 新增 `docs/adr/0047-design-baseline-v38.md`：V3.8 升唯一实现基线；替代 ADR 0042 决策 1；
      决策 2/3 继续有效；决策 4（H 系列就地登记，不重复 ADR 0042 决策 4 的缺口）
- [x] 2.2 `docs/adr/0042-*.md` 追加「勘误与替代（ADR 0047）」段（**原文不改写**）
- [x] 2.3 **改为「不新增契约名」**（原计划新增 `mcp.tool@1`/`a2a.delegate@1`/`a2a.serve@1` 三个 Draft 到
      `schemas/catalog/capabilities.json`，**本轮不采纳**）：MCP 四面映射到既有 `skill.invoke@1`／`data.query@1`；
      A2A 协议版本与认证方式待验证 ⇒ 为未验证协议占契约名只会产生**半声明 draft**（§4.1 与 W52 起的目录纪律禁止）；
      H13/H14 的 `contract` **留空**（沿用 M10/M12 先例）；并如实登记 §20.6 与目录对 `memory.access@1`
      阶段的不一致（**以目录为准，不改 §20.6 原文**）。理由见 design §6 与 V3.8 §46.5。

## 3 注册表与门控

- [x] 3.1 `scripts/contracts/registry.mjs`：编号正则扩为 `(S|R|A|B|O|P|M|H|X)`；
      **实现**家族在册断言（M 18 条、H 18 条，分别计数、逐号比号、缺号指名、超范围拒绝）
- [x] 3.2 `schemas/tests/registry.schema.json`：`acceptance.id` 与 `mapping.targets` 两处 pattern 加 `H`；
      描述由 V3.7 §16 改为 V3.8 §16
- [x] 3.3 `tests/registry.yaml`：`release_lines` 标签改 V3.8；新增 H01–H18（全部 `pending`）；
      `mapping.notes`/`legacy_note` 同步说明 H 系列的来源与归属（注册表 **69 → 87** 条）
- [x] 3.4 `tests/registry.test.mjs`：新增家族在册断言的**非空洞守卫**测试——**四组注入**：
      删 `H05`（家族内洞）、整族删除 H、加 `H19`（越界）、删 `M18`（证 M/H 分别计数），各自必须被抓到；
      条目下限 `>=66` 提到 `>=87`

## 4 OpenSpec 规格

- [x] 4.1 新增 `openspec/specs/agent-harness/spec.md`（由本变更 ADDED 需求归档生成；10 条需求 / 27 个场景）
- [x] 4.2 `openspec/specs/admission-registry/spec.md` 家族清单扩为含 `H`，并给出家族在册断言（由 MODIFIED 需求生成）

## 5 活文档基线指针

- [x] 5.1 `AGENTS.md`、`README.md`、`docs/design.md`、`docs/engineering.md`、`docs/development-plan.md` 同步
- [x] 5.2 `docs/{cloud,deployment,mixing,multi-session,operations,upgrade,platform-matrix}.md` 版本前辍订正
- [x] 5.3 `openspec/config.yaml`、`openspec/specs/{admission-registry,decision-log-integrity,duty-lease,platform-matrix}/spec.md` 同步
- [x] 5.4 `xtask/src/registry.rs` 成功输出串改 V3.8（机器可见的当前状态断言）；
      `apps/desktop/src/main.tsx` 页脚的基线陈述改 V3.8；`schemas/platform-matrix/platforms.json` 的
      「基线命中数为 0」陈述随基线订正并**重新实测**（V3.8 亦为 0）
- [x] 5.5 **不改**：`docs/reports/`、`docs/adr/`（0042 的追加段除外）、`openspec/changes/archive/`、
      以及 `crates/`、`apps/`、`plugins/` 里 doc-comment 的版本前辍（沿用 ADR 0042 决策 3）
- [x] 5.6 `docs/reports/2026-10-11-w56-agent-harness-and-interop.md`

## 6 闭环

- [x] 6.1 预热两个测试目标后跑 `cargo xtask check --profile full`：记录 EXIT、测试目标数与 passed/failed，
      以及注册表、契约目录、`tests/ui`、`tests/platform` 的实测计数
      ——**EXIT=0**；测试目标 **86（68 `Running` + 18 `Doc-tests`）**、**740 passed / 0 failed**（与 W55 相同）
- [x] 6.2 `cargo xtask check-deps` = **19** workspace packages（不变）
- [x] 6.3 全部 tasks 勾选后 `openspec archive -y w56-agent-harness-and-interop`
- [x] 6.4 `openspec validate --all --strict --archived` 记录条数
- [x] 6.5 一次 commit + push（提交信息用 Write 落 UTF-8 文件，避免 `\u` 字面转义）
- [x] 6.6 用缓存破坏参数匿名复核 CI：断言 `head_sha` 与推送一致、记录 conclusion 与失败步骤

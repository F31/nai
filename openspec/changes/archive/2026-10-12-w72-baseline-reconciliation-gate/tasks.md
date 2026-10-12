# 任务（w72 冻结基线与注册表的对账门控）

## 0 立项与核实

- [x] 0.1 建立 `openspec/changes/w72-baseline-reconciliation-gate/`（`.openspec.yaml` / `proposal.md` / `design.md` / `tasks.md` / `specs/`）
- [x] 0.2 核实两侧声明：§48.5 家族索引 = **9** 族、§57.3 发布线正本 = **6** 线；与 `tests/registry.yaml` 的实测分布（**88** 条；`slice_0` 10 / `r1` 37 / `r2` 9 / `capability_pack` 29 / `r3` 1 / `experimental` 2）**逐 id 一致**
- [x] 0.3 核实**当前无待收录偏差**（W71 已收录 B07 与爬坡两处），故台账初始为空
- [x] 0.4 核实 §16 的验收注册表**不含 B07 是设计**（V3.10 §58.3：增行会位移行号）⇒ 新断言 MUST NOT 要求 §16 增行
- [x] 0.5 核实缺口②在注册表侧的既有守卫：`COMPLETE_FAMILIES` 只覆盖 M 与 H；`packLine.length === 29` 覆盖能力包集合
- [x] 0.6 裁定**缺口①的括号语义**：括号是**示例**，「基线已定义完整编号段」的取值 MUST 从 §48.5 逐行读出（当前 9 族）
- [x] 0.7 核实「规格 vs 基线」的残留：`spec-driven-delivery` 的需求与 §56.2 情形②**直接相反**；ADR 0048 勘误段与 ADR 0049「后果」行同源
- [x] 0.8 量差量需求正文长度（全部 ≤ 500 字符）

## 1 对账门控

- [x] 1.1 新增 `scripts/contracts/baseline.mjs`：定位基线（`docs/` 中版本号最大者）、解析 §48.5 与 §57.3、对账、指针自洽
- [x] 1.2 §57.3 **表内自洽**：计数行 == 表格展开；合计 == 各行之和；计数行不得漏线或多出未知线
- [x] 1.3 **逐 id 对账**：发布线归属（extra / missing / moved，两侧都给名）+ §48.5 家族完整性（stops short / 声明了未在册的 id）
- [x] 1.4 `tests/registry.yaml` 新增 `baseline_deviations` 台账段（当前 `[]`），并写明「台账不是豁免」的规则
- [x] 1.5 解析失败与设计漂移**分开报**：章节号被改 / 章节缺失 ⇒ 具名「该节找不到」，MUST NOT 退化为通过
- [x] 1.6 `registry.mjs` 导出 `RELEASE_LINES`（§57.3 的线键与 `release_lines` 必须是同一组键）

## 2 断言与非空洞注入

- [x] 2.1 `tests/registry.test.mjs` 新增主断言：指针自洽 + 对账一致 + **解析器形状钉死**（九族顺序与规模、六个线标签、计数行、合计 88）+ 台账为空
- [x] 2.2 新增「章节号被改必然失败」用例（并验证**只改标题措辞不影响**解析结果）
- [x] 2.3 新增「§57.3 表内自洽」用例（改计数、改合计、漏线、多出未知线）
- [x] 2.4 新增「发布线归属漂移逐 id 报出」用例（移线 / 新增 id / 删除 id / 基线侧落后）
- [x] 2.5 新增「§48.5 是家族完整性判据来源」用例（缩回范围 / 扩到未在册 id / 九族全覆盖）
- [x] 2.6 新增「台账具名且不可当豁免」用例（陈旧条目 / id 非法 / 缺轮次 / 缺理由 / 非列表 / 重复 / 张冠李戴；逃生通道可用）
- [x] 2.7 新增「指针自洽」用例（失配、缺失、候选不足）
- [x] 2.8 用例锚点用 `replaceOnce`（**恰好命中一次**），并记录它当场抓到的重复锚点（§57.3 计数行 vs §58.5 复述）
- [x] 2.9 注入验证：**21 组**分别验证各守卫会开火（19 组期望报错、2 组期望通过）

## 3 ADR 与规格

- [x] 3.1 新增 `docs/adr/0050-baseline-registry-reconciliation.md`：对账（而非相等）的形状、台账三性质、判据来源、四情形规格订正
- [x] 3.2 `docs/adr/0048-*.md` **追加**勘误段（原文不改写）：勘误段把 E5 描述为「版本归属引用不变」，与 §56.2 情形②不符
- [x] 3.3 `docs/adr/0049-*.md` **追加**勘误段：其「后果」行的同一措辞
- [x] 3.4 规格差量三处：`admission-registry`（MODIFIED 判据来源 + ADDED 对账需求）、`spec-driven-delivery`（MODIFIED 改写为四情形）
- [x] 3.5 **不改**基线正文（`… V3.10.md` 零改动）与 `tests/registry.yaml` 的任何 `acceptance` 条目

## 4 活文档与报告

- [x] 4.1 `AGENTS.md` 门控清单增「冻结基线↔注册表对账」一步与其非空洞守卫
- [x] 4.2 `docs/development-plan.md` 增 W72 条目（并把 W71 登记的两处门控缺口标注为**已关闭**）
- [x] 4.3 `docs/reports/2026-10-12-w72-baseline-reconciliation-gate.md`
- [x] 4.4 全仓检索：W71 报告里「缺口②未做」的表述在别处是否有副本（C6 教训）

## 5 闭环

- [x] 5.1 跑 `cargo xtask check --profile full`（预期测试目标数与通过项数与 W71 相同——本版零 Rust 改动；Node 用例数 4 → 11）
- [x] 5.2 全部 tasks 勾选后 `openspec archive -y w72-baseline-reconciliation-gate`
- [x] 5.3 `openspec validate --all --strict` **与** `--all --strict --archived`（两个不可互相替代的口径）
- [x] 5.4 清理 `.tmp-*` 脚手架；一次 commit + push（提交信息用 Write 落 UTF-8 文件）
- [x] 5.5 匿名复核 CI 两平台结论并按观测轮约定回填报告

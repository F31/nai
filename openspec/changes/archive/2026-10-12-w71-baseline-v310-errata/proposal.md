## Why

**V3.9 之后累积了两处「已登记、未收录」的基线偏差，而 §56.2 明写纯追加不允许它们长期存续。**

1. **W68 新增 B07**（基础讲品循环，`r1`）后，基线 §48.5 的家族索引仍写「B \| B01–B06」、§57.3 的计数行仍写
   「`r1` 36 … = **87**」⇒ **基线文本与 `tests/registry.yaml` 不再一致**。W68 登记为「留待下一轮升基线一并收录」，
   并指明「与 W54 新增 A12、W55 新增 A13 的处置一致」——而 A12/A13 正是在 **W65 升基线**时被收录进 §48.5 的。
2. **W67 登记**：§5.9／§47.3 的模式爬坡是**四段**（演练 → 影子 → 人工辅助 → 受监督自动），而 §1.4／§17.3 把
   「影子模式」列在 **R2**、契约 `SessionMode` 只有**三个**取值 ⇒ **H18 的默认启用集合在 R1 内有一个阶段没有落点**。

两者都不是「基线缺能力」，也不是「范围过宽」，而是**第三类**问题：**已声明与已实现/已登记互相矛盾**。
§56 前言给的处置是固定的——**「如实登记」之后 MUST 紧接着订正现行规范**。本轮就是这条规则在**跨轮次累积偏差**上的
第一次适用。

核实过程中另发现一处**易造成整批改错**的风险（E5）：全仓活文档「V3.9」命中 **179 行 / 206 处**（口径见 `design.md` §4）。就地订正必须区分四种情形——
① 声明「当前基线是哪一份」的位置（更新，或改为不写死版本号）；② `§NNN` 的版本前缀（**随基线更新**，W65 先例）；
③ **关于某一版的陈述**与该版专属版本章节（§39、§48.1/§48.2、§56、§57.1/§57.2/§57.4/§57.5）**保留原文**；
④ 代码 doc-comment 前辍（不改）。
E5 的**首版判据把「章节指针」混入保留类，写入基线前被 W65 先例否证并订正**（见 design.md §4）——
不做此区分会同时产生两种错：把「V3.9 做的收缩」误记为「V3.10 做的」，或让「`§NNN` 按当前基线解析」失去锚点。

## What Changes

- **V3.10 基线文档**（`docs/直播数字人 + 智能体系统技术方案 V3.10.md`）：由 V3.9 复制而来，**前 2647 行做 14 行
  1:1 行替换**（`diff` 实测：整文件 **15 hunk = 14 个 `c` 型 + 1 个 `a` 型**——该 `a` 型是 `2647a2648,2713` 的**尾部追加**；
  限定在**前 2647 行**则**只有 14 个 `c` 型、零 `a`·`d`**），并**追加 §58 版本说明与订正索引**。
  规模：V3.9 = 2647 行 / 216,450 字节；V3.10 = 2713 行 / 226,578 字节（追加 §58 共 66 行）。
- **订正清单 E1–E5**：
  - **E1**（5 行：§1.4、§17.3、§48.5 的 R1 汇总、§51.2、§57.3 的 R1 行）R1 清单增补 **B07**；
  - **E2**（1 行：§48.5 家族索引 B 行）「B01–B06」⇒「B01–B07」并补位置「第 16 章、53」；
  - **E3**（1 行：§57.3 计数行）`r1` 36 ⇒ **37**、合计 87 ⇒ **88**；
  - **E4**（3 行：§40.4 纪律 5、§47.3、§48.5 的 H18 行）把「默认只在演练、影子与人工辅助下启用」改为
    「默认只在**受监督自动以外的阶段**启用」——**与阶段枚举解耦**；
  - **E5**（1 行：§56.2）补明「就地订正」的**对象边界**：只订正**与当前事实不符的陈述**，**版本归属引用**不随基线号变更。
- **另有 3 行属版本标识**（标题、副标题、§0.2 的结构指针），非订正。
- **ADR 0049** 裁决基线由 V3.9 升为 **V3.10**，并**首次区分「订正版」与「范围版」**；ADR 0048 追加勘误段（原文不改写）。
- **活文档基线指针同步**为 V3.10；**版本归属引用保留**（E5 的判据）。
- **不做**：**不改任何 acceptance 的 `status`**（88 条原样，H18 与 B07 仍 `pending`）；**不新增契约名、不改能力目录**
  （零漂移）；**不实现任何代码**（Rust 行为零改动）；**不重排 W 编号**（本版为 W71）；
  **不改版本专属陈述**（§39、§48.1/§48.2、§57.1/§57.2/§57.4/§57.5 保留原文；**§57.3 是发布线正本，按 E1/E3 就地订正**）；**不给 §16 的验收注册表增行**
  （插入行会位移行号、破坏 `§NNN` 引用契约——与 A12–A14 的处置一致）。

## Capabilities

### Modified Capabilities

- `admission-registry`：**修改**「Registry covers every baseline acceptance family」——基线引用改为**按当前基线解析**，
  不再写死版本号（沿用 §56-C9 对 `spec-driven-delivery` 的同一处置）。
- `agent-harness`：**修改**两条需求。①「Harness layers fail with named degradations and are observable」末句改为
  「默认只在**受监督自动以外的阶段**启用」；②「Harness mechanisms ramp through stages…」把默认启用集合的判据
  由**前三段**改为**按爬坡位置判定**，并把「某阶段无契约落点」这一场景改写为「该阶段在本发布线**不存在**，
  不构成缺陷、也 MUST NOT 为对齐枚举而新增」。
- `spec-driven-delivery`：**新增**需求「Corrections distinguish stale statements from provenance」——把 E5 的
  对象边界变成可判定的要求（当前基线指针随升版同步；版本归属引用 MUST NOT 被改）。

## Impact

- 新增：`docs/直播数字人 + 智能体系统技术方案 V3.10.md`、`docs/adr/0049-design-baseline-v310.md`、
  `docs/reports/2026-10-12-w71-baseline-v310-errata.md`、本变更的差量规格。
- 修改：`docs/design.md`、`README.md`、`AGENTS.md`、`docs/development-plan.md`、`docs/engineering.md`、`docs/kernel.md`、
  `openspec/config.yaml`、`openspec/specs/{admission-registry,agent-harness,spec-driven-delivery,platform-matrix,duty-lease,decision-log-integrity,basic-selling-loop}/spec.md`、
  `schemas/tests/registry.schema.json`、`schemas/platform-matrix/platforms.json`、`xtask/src/registry.rs`、
  `apps/desktop/src/main.tsx`、`tests/registry.yaml`（`release_lines` 标签与 H18 条目的 notes）、
  `docs/adr/0048-*.md`（追加勘误，原文不改写）。
- **能力目录 `schemas/catalog/capabilities.json` 不动**：不新增契约名 ⇒ 全部生成物**零漂移**。
- **Rust 行为零改动**（仅 `xtask/src/registry.rs` 的一处输出串随版本号同步）。
- **本版发现的歧义（登记、不裁定）**：`admission-registry` 的需求说「SHALL 断言**每个**『基线已定义完整编号段』的家族逐号在册
  （当前为 M01–M18 与 H01–H18）」，而 §48.5 的家族索引把 **S01–S12、R01–R11、A01–A14、B01–B07、O01–O05、P01、X01–X02**
  也声明为连续编号段。门控目前只断言 M 与 H。两种读法（括号是**当前清单** vs 括号是**举例**）都能成立，且该歧义**先于本轮存在**
  （E2 之前 B 行就是「B01–B06」，同样是连续段）。**本轮只登记，不改门控**——扩到九族或收窄需求措辞都是规则语义裁定，
  应由专门一轮带各自注入守卫进行。
- **验证**：`cargo xtask check --profile full`（含注册表家族在册与能力包集合断言、`tests/contracts`、`tests/platform`、
  `tests/ui` 与前端门控）、`openspec validate --all --strict` 与 `--all --strict --archived`；推送后匿名复核 CI 两平台结论。

# 任务（w71 V3.10 订正版：跨轮次累积偏差的收录）

## 0 立项与核实

- [x] 0.1 建立 `openspec/changes/w71-baseline-v310-errata/`（`.openspec.yaml` / `proposal.md` / `design.md` /
      `specs/{admission-registry,agent-harness,spec-driven-delivery}/spec.md` / `tasks.md`）
- [x] 0.2 核实 registry 实测分布（**88** 条；`slice_0` 10 / `r1` **37** / `r2` 9 / `capability_pack` 29 / `r3` 1 / `experimental` 2）
      与 B 家族（**B01–B07**，B07 属 `r1`/`pending`）
- [x] 0.3 核实两处偏差的全部落点：B07 共 **7 行**（§1.4 L56、§17.3 L698、§48.5 L2306/L2316、§51.2 L2432、§57.3 L2624/L2630）；
      爬坡共 **3 行**（§40.4 L1931、§47.3 L2216、§48.5 L2342）
- [x] 0.4 核实**不改**的范围：§39.5（V3.7 版本专属索引）、§57.1/§57.2/§57.4/§57.5（V3.9 版本专属陈述；**§57.3 是发布线正本例外**）、§16 的验收注册表（增行会位移行号）
- [x] 0.5 核实先例：`V3.9.md` 与 `V3.8.md` **各只有一次提交**（即升基线那次）；W54 新增 A12、W55 新增 A13 于 W65 收录
- [x] 0.6 实测活文档「V3.9」命中规模（**179 行 / 206 处**，口径见 `design.md` §4）并按四情形分类；据 W65 先例**订正 E5 判据**
      （首版把「章节指针」误归保留类，写入基线前被先例否证）
- [x] 0.7 量差量需求正文长度（全部 ≤ 500 字符，留有余量）

## 1 基线文档 V3.10

- [x] 1.1 由 V3.9 生成 `docs/直播数字人 + 智能体系统技术方案 V3.10.md`
- [x] 1.2 **14 行 1:1 行替换**：E1（5 行，B07）、E2（1 行）、E3（1 行）、E4（3 行，爬坡）、E5（1 行，§56.2 边界）
      + 版本标识 3 行（标题 / 副标题 / §0.2）
- [x] 1.3 追加 §58（版本说明与订正索引：本版类型、订正清单 E1–E5、变化表四栏、发布线归属、本版不做、遗留）
- [x] 1.4 `diff` 实测并记录：整文件 **15 hunk = 14 个 `c` 型 + 1 个 `a` 型**（该 `a` 型为 `2647a2648,2713` 尾部追加）、`d` 型为 0；限定**前 2647 行**则只有 14 个 `c` 型、`a`·`d` 型为 0；变更行号恰为目标 14 行
- [x] 1.5 实测规模写入 ADR：V3.9 = 2647 行 / 216,450 字节；V3.10 = 2713 行 / 226,578 字节（总差 +10,128 字节 = §58 追加块 66 行 / 8,922 字节 + 14 行 1:1 替换净增 1,206 字节）
- [x] 1.6 自述计数自检（共四处）：副标题的行数、§58.2 标题的 E 编号区间、ADR 的「不改」清单（§57.3）、ADR 的规模栏（64 行→66 行、9,952→10,128 字节且非「全部为 §58」）与实测逐项一致

## 2 ADR 与规格

- [x] 2.1 新增 `docs/adr/0049-design-baseline-v310.md`：V3.10 升唯一实现基线；只替代 ADR 0048 的决策 1；
      决策 2–5 继续有效（决策 3 由 E5 补明对象边界）；决策 3 首次区分「订正版 / 范围版」；决策 4 明确不改什么
- [x] 2.2 `docs/adr/0048-*.md` 追加勘误段（**原文不改写**）
- [x] 2.3 三份能力差量：`admission-registry`（MODIFIED 基线引用改为按当前基线解析）、
      `agent-harness`（MODIFIED ×2：默认启用集合与阶段枚举解耦）、`spec-driven-delivery`（ADDED 归属 vs 现状的边界）
- [x] 2.4 **不新增契约名、不改能力目录** ⇒ `schemas/catalog/capabilities.json` 与全部生成物**零漂移**
- [x] 2.5 **不改任何 acceptance 的 `status`**（88 条原样；H18 与 B07 仍 `pending`）

## 3 活文档同步

- [x] 3.1 **声明「当前基线是哪一份」的位置**改为 V3.10（`docs/design.md`、`README.md`、`AGENTS.md`、
      `docs/development-plan.md`、`docs/engineering.md`、`openspec/config.yaml`、`xtask/src/registry.rs` 的输出串、
      `apps/desktop/src/main.tsx`）；`openspec/specs/*` 的**基线声明**改为**不写死版本号**的形式（§56-C9 先例）
- [x] 3.2 **`§NNN` 的版本前缀**随当前基线更新（E5 情形②，W65 先例）——含轮次记录标题、判据指针、
      `tests/registry.yaml` 的 `release_lines` 标签与各条 `notes`、`schemas/*.json`、门控脚本的字符串常量
- [x] 3.3 **保留 `V3.9`**（E5 情形③）：升基线链条、关于 V3.9 的陈述（「V3.9 收缩」「V3.9 新增」「V3.9 自称本版最重要的一条」）、
      按 V3.9 实测的数字（「V3.9 亦为 0」）、V3.9 专属版本章节（§39／§48.1–48.2／§56、§57.1/§57.2/§57.4/§57.5）；
      能力包标记改写为 `【V3.10 §51.3（V3.9 收缩：自 rN 降为能力包）】`；`crates/` 的 doc-comment 前辍不改（E5 情形④）
- [x] 3.4 **幸存者审计**：逐个核对残留 `V3.9` 是否全属情形③（本轮抓到 1 处误留：`openspec/specs/platform-matrix/spec.md` 的基线声明）
- [x] 3.5 H18 条目的 `notes` 更新为「爬坡默认启用集合已与阶段枚举解耦（V3.10 §58-E4），影子在本线内不存在」
- [x] 3.6 **顺带订正两处错引与两处失实前瞻陈述**：§57.4 ⇒ **§57.3**（计数行实际在 §57.3，散落 `tests/registry.yaml`、
      `AGENTS.md`、`docs/kernel.md`、`docs/development-plan.md` 四处）；W68/W70 记录里的「仍待升基线轮次收录」
      加「**已在 W71 收录**」闭环标记（原文保留）
- [x] 3.7 **不改**：`docs/reports/`、`docs/adr/`（勘误追加段除外）、`openspec/changes/archive/`、轮次记录里指向**该轮产物**的文件路径（如 W65 的 `…V3.9.md`，与 W56 记录仍留 `…V3.8.md` 同理）

## 4 报告与闭环

- [x] 4.1 `docs/reports/2026-10-12-w71-baseline-v310-errata.md`
- [x] 4.2 `docs/kernel.md`、`AGENTS.md`、`docs/development-plan.md` 增 W71 条目，并**关闭** W68 的 B07 偏差与
      W67 的两项待确认（其中「相关类 vs 全六类」核实为**不是偏差**）
- [x] 4.3 跑 `cargo xtask check --profile full`：测试目标数与通过项数与 W70 对照（本版**无代码改动**，预期**两者都不变**）
- [x] 4.4 全部 tasks 勾选后 `openspec archive -y w71-baseline-v310-errata`
- [x] 4.5 `openspec validate --all --strict` **与** `openspec validate --all --strict --archived`（两个不可互相替代的口径）
- [x] 4.6 一次 commit + push（提交信息用 Write 落 UTF-8 文件，避免 `\u` 字面转义）；匿名复核 CI 两平台结论并按观测轮约定回填

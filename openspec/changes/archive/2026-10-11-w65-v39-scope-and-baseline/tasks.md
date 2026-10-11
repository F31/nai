# 任务（w65 V3.9 范围重排与升基线）

## 0 立项

- [x] 0.1 建立 `openspec/changes/w65-v39-scope-and-baseline/`（`.openspec.yaml` / `proposal.md` / `design.md` /
      `specs/{admission-registry,agent-harness,decision-log-integrity,spec-driven-delivery}/spec.md` / `tasks.md`）
- [x] 0.2 `openspec validate w65-v39-scope-and-baseline --strict` 通过

## 1 核实与裁定

- [x] 1.1 逐条核实外部审视（五维批评）与用户反批评，产出 `docs/reports/2026-10-11-v38-review-adjudication.md`：
      对 K 的 K1–K18 逐条裁定、对 U 的 U1–U10 逐条核实（如 `181.9` 在全文 **0 命中**、§23.2 实测 **32 张**表、
      六进程里 `control-server` 标注为可选）
- [x] 1.2 实测**双方都未发现**的 7 处问题（D1–D7）：`A12–A14` 基线正文零命中、`W45` 全仓无记录、
      两处「冻结 `memory.access@1`」被证伪、§47.6 的 W64 单行跨线、§23.2 表数、
      `docs/control.md` 的 ADR 篇数、`spec-driven-delivery` 的 **V3.5 基线指针**

## 2 基线文档 V3.9

- [x] 2.1 复制 V3.8 为 `docs/直播数字人 + 智能体系统技术方案 V3.9.md`
- [x] 2.2 前 2366 行 **42 行 1:1 行替换**（**24 个 hunk、零增删行**），以 `diff` 实测：
      仅行号 `[1,3,29,56,57,174,578,624,698,699,825,2233,2237,2242–2247,2249,2251,2261,2305,2313,2316,2317,2319,2323,2329–2338,2340,2344,2355,2356]` 不同
- [x] 2.3 追加第五部分 §49–§57（定位与版本减法纪律、三个维度拆开、发布线与能力包、平台准入独立成轨、
      基础讲品循环前移、产品视角、决策日志责任再限定、规范冲突订正 C1–C9、V3.9 版本说明）
- [x] 2.4 实测规模写入 ADR：V3.8 = 2366 行 / 193,438 字节，V3.9 = 2647 行 / 216,450 字节 ⇒ 新增 **281 行 / 23,012 字节**
- [x] 2.5 声明**本版砍掉的：无**（范围重排，不是能力删除；由 INV-34 保证边界保留）

## 3 ADR 与契约

- [x] 3.1 新增 `docs/adr/0048-design-baseline-v39.md`：V3.9 升唯一实现基线；替代 ADR 0047 决策 1；
      决策 2–5 继续有效；**决策 3 首次确立「活文档就地订正 / 历史记录追加勘误」的边界**（含 1:1 硬约束）；
      决策 4 新增第六条发布线并给出 INV-33/INV-34 与三条用户约束的可判定对应
- [x] 3.2 `docs/adr/0047-*.md` 与 `docs/adr/0042-*.md` 追加勘误段（**原文不改写**）
- [x] 3.3 **不新增契约名**（沿用 §46.5 三条理由）⇒ `schemas/catalog/capabilities.json` 与全部生成物**零漂移**；
      `memory.access@1` 阶段以目录（`frozen`）为准，本轮只订正 §20.6 的**陈述**

## 4 注册表与门控

- [x] 4.1 `scripts/contracts/registry.mjs`：`RELEASE_LINES` 增 `capability_pack`；新增**三向断言**
      （能力包集合恰为 29 条 / 无归属外 id 被标为能力包 / **无能力包 id 落在 `slice_0`/`r1`/`r2`**，即 INV-33），
      失败时**同时给出 id 与当前线**
- [x] 4.2 `schemas/tests/registry.schema.json`：`release_lines.required`、`release_lines.properties`、
      `acceptance.release_line.enum` **三处**（两处硬编码）同加 `capability_pack`；描述改 V3.9 §51.1/§57.3
- [x] 4.3 `tests/registry.yaml`：**29** 条（M01–M18、H05–H14、H16）由 `r1`/`r2` 改为 `capability_pack`；
      实测分布 `slice_0` **10** + `r1` **36** + `r2` **9** + `capability_pack` **29** + `r3` **1** + `experimental` **2** = **87**
      （**只改归属，不加条目；不改任何 `status`**）
- [x] 4.4 `tests/registry.test.mjs`：新增**三种注入**的非空洞守卫（把 `M01` 提回 `r1`、把 `H17` 标为能力包、
      清空能力包线），三种必须分别被抓到——**4/4 通过**

## 5 OpenSpec 规格

- [x] 5.1 四条能力的差量：`admission-registry`（ADDED 发布线断言 + MODIFIED 两条）、`agent-harness`（ADDED 分界与 INV-34）、
      `decision-log-integrity`（ADDED 宣称限定）、`spec-driven-delivery`（MODIFIED 基线指针不写死版本）
- [x] 5.2 归档后 `openspec/specs/` 四条规格更新，且 `validate --all --strict` 与 `--archived` 均通过

## 6 活文档基线指针

- [x] 6.1 21 个活文档 **72 处**替换（`AGENTS.md`、`README.md`、`docs/*.md`、`openspec/config.yaml`、
      `openspec/specs/{admission-registry,agent-harness,decision-log-integrity,duty-lease,platform-matrix}/spec.md`、
      `schemas/platform-matrix/platforms.json`、`tests/registry.yaml`、`xtask/src/registry.rs`、
      `apps/desktop/src/main.tsx`）
- [x] 6.1b 另有 **5 处**因 D2/D6/D7 单独订正：`AGENTS.md` 与 `docs/development-plan.md` 的轮次范围、
      `docs/control.md` 的 ADR 篇数、`openspec/specs/spec-driven-delivery/spec.md` 的两行（Purpose 与需求正文）
- [x] 6.2 **同一条不实陈述会散落在多个活文档**：C6 的「W41–W55」实测出现在**四处**（§47.6 编号说明、§48.6、
      `AGENTS.md`、`docs/development-plan.md`），四处在同一轮一并订正
- [x] 6.3 两处**易失效陈述**订正为不写死的形式（C8 `docs/control.md` 的 ADR 篇数、C9 `spec-driven-delivery` 的 V3.5 指针）
- [x] 6.4 **不改**：`docs/reports/`、`docs/adr/`（勘误追加段除外）、`openspec/changes/archive/`，
      以及 `crates/`、`apps/`、`plugins/` 里 doc-comment 的版本前辍（ADR 0015 先例）

## 7 报告与闭环

- [x] 7.1 `docs/reports/2026-10-11-w65-v39-scope-and-baseline.md`
- [x] 7.2 `docs/development-plan.md` 增 W65 轮次条目（并补记此前缺记的 W57 指针）；`AGENTS.md` 增 W65 小节
- [x] 7.3 预热两个测试目标后跑 `cargo xtask check --profile full`：**EXIT=0**，测试目标 **87（69 `Running` + 18 `Doc-tests`）**、
      **761 passed / 0 failed**（与 W57 相同）、`check-deps` **19 包**、`tests/registry.test.mjs` **4/4**
- [x] 7.4 全部 tasks 勾选后 `openspec archive -y w65-v39-scope-and-baseline`
- [x] 7.5 `openspec validate --all --strict` **与** `openspec validate --all --strict --archived`（两个不可互相替代的口径）
- [x] 7.6 一次 commit + push（提交信息用 Write 落 UTF-8 文件，避免 `\u` 字面转义）；匿名复核 CI 两平台结论

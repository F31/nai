# Tasks: W134 — CI 上同一批测试被跑两遍

## 1. 勘定（先量，再改）

- [x] 1.1 读 `.github/workflows/foundation.yml` 全文，确认两平台腿跑同一套步（矩阵 `os`）
- [x] 1.2 读 `xtask/src/lib.rs`：`check --profile full` = `engineering_checks` + `frontend::check`，前者含
      `cargo test --workspace --locked --exclude nai-xtask`
- [x] 1.3 现测 7 条 `test --suite …` 的包集**全部**是 workspace 成员（dispatch 臂逐条读源码）
- [x] 1.4 计时：`cargo build --workspace --all-targets` 预构建后逐条计时，7 条 suite 合计 **517s 纯执行**
- [x] 1.5 勘定四层既有判据（W95/W97/W100/W101）对「重复」全部沉默
- [x] 1.6 勘定 `step-config.mjs` R3：非平台相关的 `ci-only` 判定被平台收窄 ⇒ 红 ⇒ 「按平台分层」需模型变更，本轮不做

## 2. 先写会红的判据

- [x] 2.1 `scripts/contracts/step-overlap.mjs`：覆盖类别闭集 + 门控覆盖并集 + suite 覆盖解析 + 两两包含
- [x] 2.2 `tests/contracts/step-overlap.test.mjs`：12 条（基线干净 / 并集而非第一条 / 重复具名 / 不覆盖的不算重复 /
      两条 suite 互不包含 / 域外类别具名 / 三类各有实例 / 缺输入具名 / 套件不可解析具名 / suite 包集本身 /
      空判定集具名 / 门控区域没有 cargo test 具名）
- [x] 2.3 对**未改动**的 workflow 跑该判据 ⇒ **9 通过 / 1 红**，红的那条正是 7 条重复

## 3. 实施

- [x] 3.1 `foundation.yml` 删 7 条分套件步，留 `contracts`（门控不覆盖的 C++ 夹具）
- [x] 3.2 `WORKFLOW_STEPS` 删对应 7 条（现测两侧 22 → 15，相等）
- [x] 3.3 `xtask/src/lib.rs` 的 `engineering_checks` 登记新判据
- [x] 3.4 重新勘定既有夹具锚点：`workflow` / `step-config` / `step-target` / `gate-coverage` /
      `workflow-triggers` 五套里锚在已删步名上的注入，全部改锚到单行幸存步，并补「锚点 MUST 命中」断言
- [x] 3.5 修一处自造缺陷：注释里连写 `cargo` + `xtask` 被 `ciXtaskCommands` 当成真实 CI 命令 ⇒ 改措辞

## 4. 门控与验证

- [x] 4.1 契约判据 10 套全绿（step-overlap 12 / workflow 14 / step-config 16 / step-target 16 /
      gate-coverage 24 / tier-payment 22 / workflow-triggers 15 / npm-scripts 34 / schema 30 / descriptor 2）
- [x] 4.2 可移植 node 判据 279/279（门控前的一次子集实跑）
- [x] 4.3 `cargo fmt --all --check` 干净（注入基线即最终字节）
- [x] 4.4 全量门控 `cargo xtask check --profile full`：`EXIT=0`，Rust 1085/0、Node 336/0（14 套件）、错误类 0
- [x] 4.5 注入验证 D1–D6 全真红，三文件 `sha256sum -c` 逐字节还原
- [x] 4.6 `openspec validate w134-ci-step-coverage --strict` 通过

## 5. 交付

- [x] 5.1 报告 `docs/reports/2026-10-17-w134-ci-step-coverage.md`
- [x] 5.2 回填 `docs/development-plan.md` / `docs/knowledge.md` / memory

归档后仍要做（**流程项，不是任务**）：两条 `openspec validate`（含 `--archived`）、一次 commit + push、
CI 观测（两平台结论 + 完成态确认 ≥2 次）、把 CI 结论回填进报告 §四。

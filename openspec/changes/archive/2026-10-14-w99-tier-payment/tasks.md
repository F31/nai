# w99 任务

## 1. 判据模块

- [x] `scripts/contracts/tier-payment.mjs`：`TIER_PAYMENTS`、`portableSurface`、`stripShellComments`、
      `tierErrors`
- [x] T1 规则闭世界：`TIER_PAYMENTS` 的键集 == `TEST_TIERS`，且每个 tier 取值 MUST 至少有一个站点
- [x] T2 `portable` 由 portable 区域兑付：`engineering_checks` **剥注释后**含 marker（host 为 `lib.rs`）
      或含该 host 模块的调用（`<module>::<fn>(root`）
- [x] T3 `full` 由驱动表兑付：`packageDirOf(host)` 是某个 STEP 的 `dir`，且该 STEP 的 `script` 脚本体含 marker
- [x] T4 `ci` 由具名 CI step 兑付：marker 在某个具名 step 的 `run`（剥整行注释）里逐字存在
- [x] T5 反向：`full`/`ci` 的 marker MUST NOT 落在 portable 区域里
- [x] 缺输入 / 区域不可解析 / tier 无兑付规则 ⇒ 具名失败，MUST NOT 静默降级

## 2. 接线与测试

- [x] `tests/contracts/tier-payment.test.mjs`（合成世界 + 注释陷阱 + 真实仓库 + 非空洞 + 反向不红）
- [x] 登记进 `xtask/src/lib.rs::engineering_checks`
- [x] 登记进 `scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES`（11 → 12）
- [x] 同步 `tests/registry.test.mjs` 的逐字站点清单

## 3. 验证

- [x] P1–P6 注入对真实文件执行，逐条产生具名错误，`sha256` 逐字还原（备份拷回，**不用 `git checkout`**）
- [x] 边界实测：与 W84（可达性闭世界）、W95（step 面）、W97（包集合）、W98（脚本消费者）互不顶替
- [x] 全量门控 `cargo xtask check --profile full` 通过

## 4. 文档与收尾

- [x] `docs/adr/0077-tier-payment.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.22，并更新 §10 计数索引
- [x] `AGENTS.md`、`docs/development-plan.md` 追加 W99 段
- [x] `docs/reports/2026-10-14-w99-tier-payment.md`

## 5. 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。

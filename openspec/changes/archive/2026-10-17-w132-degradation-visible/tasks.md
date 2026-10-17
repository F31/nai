# Tasks: W132 — H17 的 runtime 接线

- [x] 1. 勘定：逐项现测 H17 的落点（`evaluate_degradation` 与 `assert_no_free_form` **零生产调用点**；
      `DegradeVerdict::visible` **恒为 `true`** ⇒ 「无静默降级」在判定面是构造性事实；
      `CapabilityView` **无承载降级**的字段；`decision_log` 的 kind CHECK 13 个名字**无 `degradation`**；
      本机**无 protoc** ⇒ 不扩展契约）
- [x] 2. 先写会红的判据：kernel `crates/nai-kernel/tests/harness.rs`（**5 条**）+ runtime
      `crates/nai-runtime/tests/degradation.rs`（8 条）；
      首跑捕获 **19 个错误**（`E0432` ×3 / `E0425` ×2 / `E0599` ×14）⇒ 判据在测待建之物
- [x] 3. 承载物：`DecisionKind::Degradation`（**critical**，理由写进枚举注释：telemetry 会落缓冲
      ⇒ 悄悄消失的降级 = 静默降级）+ `migrations/sqlite/0011-decision-log-degradation-kind.sql`
      （逐列重建、逐字复制含四个哈希列、重建两个只追加触发器）；版本号成套**六处**同步 10→11
      （`migration::VERSION` / `SQL` 数组 + `include_str!` / `backup.schema.json` const /
      Rust 契约样本断言 / `schema.test.mjs` 备份样本含「上一版 MUST 被拒」+1 / TS 生成物）
- [x] 4. 实现：kernel 侧 `DegradationNotice` + `render_degradation_notices`（一条判定 ⇒ 一条通知、
      字段照抄、不去重不合并）；runtime 侧 `Composition::record_degradation`（入参是**事实**，
      判定在生产路径上真调用 `evaluate_degradation` 与 `assert_no_free_form`，违规零落盘）
      与 `Composition::read_degradation_view`（只从链上解码、不可解析 ⇒ 具名失败、空链报 `0`）
- [x] 5. 正向半与接线登记：确认新增的是 **Rust** 集成测试，由 `cargo test --workspace` 自动纳入，
      **不需**在 `engineering_checks` 登记（`test-reachability` 只管 `.test.mjs`）
- [x] 6. 判据绿 + fmt 基线：先 `cargo fmt --all --check` ⇒ 发现 3 处红 ⇒ 就地修掉**再**取基线
      （W129 的坑：基线 MUST 是 fmt-clean 的最终字节）。kernel **29/29**（**24 + 5**）、
      runtime **9/9**、`nai-storage --lib` **82/82**（81 + 1 迁移，`RUST_TEST_THREADS=4`）、
      `schema.test.mjs` **30/30**
- [x] 7. 注入验证：**9 条**注入真跑（kernel K1–K4 / runtime R1–R5），**8 条真红**；
      补一条 `a_failure_whose_preferred_level_is_unavailable_shows_the_level_it_walked_down_to`
      ——既有 8 条全用 `all_available()`，在该输入下首选级与回退级构造性相等 ⇒ K4 让 kernel 29/29
      **全绿**、runtime 既有 8 条**全绿**，**只有这条新补的红**（W131 §6.35 的第二个方向再实证）；
      **R2 首跑假绿**（`!cfg!(test)` 在生产 lib 里恒 false ⇒ 注入没改行为，改成 `&& verdicts.is_empty()`
      后红）⇒ 第三种成因：**注入自身坏了**。4 个被注入文件全部 `sha256sum -c` 逐字节还原
- [x] 8. 文档：`docs/adr/0110-degradation-visible.md` + 轮次报告 +
      `docs/development-plan.md` 小节 + `docs/knowledge.md` 轮次注记
- [x] 9. registry `H17`：notes 就地订正（补上本轮接线与「仍缺呈现层」的分界），
      `status` 判定**由证据决定** ⇒ 结论：**仍 `pending`**（四环：①②已建且有覆盖，
      ③调用方／④真实产生者未接线；与 H15/H01/H02 接线轮惯例一致）
- [x] 10. 全量门控 `cargo xtask check --profile full` ⇒ 绿
- [x] 11. 闭环：`openspec archive --yes` + 两条 validate（`--all --strict` / `--archived` +
      `--archived <change>`）+ 一次 commit + push + CI 观测（触发确认 1 次 + 完成态 ≥2 次，绝不轮询）
      + 报告回填 + docs-only 回填提交 + 临时文件清理 + memory 三处

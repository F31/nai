# Tasks: W128 — 运行时升级激活调用点（R10 收口）

- [x] 1. 夹具共享化：`nai-upgrade-core/src/rehearsal.rs` 提 `RehearsalPackage`/`demo_manifest`/`demo_facts`/`DEMO_CONTENT`，三个测试文件改用之；`nai-runtime/tests/common` 收敛 `hold_session` 的重复定义。core 全量 **99 passed / 0 failed**，三处重复夹具命中数 **0**
- [x] 2. 先写会红的判据：`crates/nai-runtime/tests/upgrade.rs` 5 条 → 首跑 **14 个编译错误**（`stage_upgrade`/`activate_staged`/`UpgradeActivation` 全不存在）
- [x] 3. 实现 `crates/nai-runtime/src/upgrade.rs`（`UpgradeActivation` + 两处入口）+ `lib.rs` 导出 + `From<UpgradeError> for RuntimeError` → **5 passed; 0 failed**
- [x] 4. 注入验证 4 组（I1 暂存先问门 ⇒ 4 红 / I2 推迟分支照样激活 ⇒ 3 红 / I3 吞掉门错误 ⇒ 1 红 / I4 抹掉导出计数 ⇒ 1 红），逐组用编辑工具注入、`cp` 备份、`sha256` 逐字节还原
- [x] 5. spec：`runtime-composition` 追加 2 条 Requirement（共 4 个 Scenario）
- [x] 6. registry：`R10` 记本轮进展并**维持 `implemented`**（三条理由见 design.md）
- [x] 7. 文档：ADR 0106 + W128 报告 + development-plan W128 段 + knowledge 注记
- [x] 8. 全量门控 `cargo xtask check --profile full`（必要时限并发）

归档 → `validate --all --strict` → `archive --yes` → `--archived --strict` → 一次 commit + push（SSH 443）
→ CI 双平台观测 → 回填报告。

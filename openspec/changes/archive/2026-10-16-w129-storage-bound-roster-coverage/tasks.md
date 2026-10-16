# Tasks: W129 — 存储上限名册的行为覆盖（R08 剩余 11 项）

- [x] 1. 勘定：11 项零覆盖逐个定位具名拒绝、可达性与夹具代价（全仓拒绝名计数 = 1 × 11）
- [x] 2. 先写会红的判据：`tests/storage_bounds.rs` 8 条 + 名册扩至 20 常量 + 闭世界判据；
      首跑捕获 **20 个 `E0425`**（12 个常量不存在）⇒ 证明判据真的在测待建之物
- [x] 3. 实现：12 个具名常量 + 拆 ⑥ 合并拒绝（新增 `candidate payload limit`）+
      两处 SQL `LIMIT` 派生成 `上限 + 1`；⑦ 改引常量
- [x] 4. 判据 8/8 绿；`nai-storage` 全量 **72 passed / 0 failed**（63 基线 + 8 + 1）
- [x] 5. 注入验证：**8 组 / 17 条**注入真跑，逐条确认「对应判据变红且可归因」；
      R1 对照证明「只动比较符不动常量值」时名册判据留绿；全部逐字节还原（`sha256sum -c`）
- [x] 6. spec：`local-persistence` +7、`knowledge-pipeline` +1、`scheduling-and-budgets` +3 Requirement
- [x] 7. 文档：ADR 0107 + W129 报告 + `development-plan.md` + `knowledge.md`；registry `R08` 按现测判定
- [x] 8. 全量门控 `cargo xtask check --profile full`
- [x] 9. 一次 commit + SSH 443 push + CI 双平台观测
- [x] 10. 报告回填（docs-only 提交）+ 临时文件清理 + memory 三处回填

归档 → 两条 `validate`（`--archived` 另跑）→ commit/push → CI 观测 → 回填报告。

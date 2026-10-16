# Tasks: W130 — S07 诊断面扫描（日志 / 诊断包 / 崩溃转储）

- [x] 1. 勘定：S07 四面的仓库内落点逐个现测（仓库面已建；日志面有真实产生者；诊断包/崩溃转储 0 命中）
- [x] 2. 先写会红的判据：`xtask/src/secrets.rs` 新增 9 条 `--lib` 用例；
      首跑捕获 **9 个 `E0422`/`E0425`**（`ArtifactLedger`/`ArtifactHit`/`scan_tree`/`check_artifacts` 不存在）
      ⇒ 证明判据真的在测待建之物
- [x] 3. 实现：`Artifact`/`ARTIFACTS` 登记表 + `ArtifactLedger` 闭世界台账 + `scan_tree`/`walk` +
      `check_artifacts`；`check(root)` 追加产物域（仓库域行为不变，只加具名弱点注释）；`tempfile` dev-dep
- [x] 4. 接线：CI 门控步 `tee` 改到 `.nai-diagnostics/`（前置 `mkdir -p`）；新增 `if: always()` 的
      `cargo xtask check --secrets` 步；`.gitignore` 增 `/.nai-diagnostics/`；W95/W100 门控登记
- [x] 5. 判据绿：`cargo test -p nai-xtask --lib` **60 passed / 0 failed**；
      `cargo xtask check --secrets` 通过并打印产物域结算；`clippy -D warnings` 净
- [x] 6. 注入验证：**11 条**注入（A1–A6 / B1、B1b、B2–B4）真跑，逐条确认「对应判据变红且可归因」；
      全部逐字节还原（`sha256sum` 相等）；`fmt` 基线在注入前即是 clean 的最终字节
- [x] 7. spec：`live-provider-evidence` MODIFIED（扫描域＝git 会发布的内容）+ ADDED
      `The registered diagnostic artifact domain is scanned by the same command`；
      `engineering-foundation` MODIFIED（执行配置＝解析出的平台集合）+ ADDED
      `A step that judges an artifact produced by an earlier step declares always()`
      （**归档后 `--all --strict` 曾报两条 `>500 characters`**⇒ 一个 Requirement 只声明一条行为，
      就地重分为 MODIFIED + ADDED 并重新归档）
- [x] 8. 文档：ADR 0108 + W130 报告 + `docs/development-plan.md` + `docs/knowledge.md`；
      registry `S07` 按现测判定
- [x] 9. 全量门控 `cargo xtask check --profile full`
- [x] 10. 一次 commit + SSH 443 push + CI 双平台观测
- [x] 11. 报告回填（docs-only 提交）+ 临时文件清理 + memory 三处回填

归档 → 两条 `validate`（`--archived` 另跑）→ commit/push → CI 观测 → 回填报告。

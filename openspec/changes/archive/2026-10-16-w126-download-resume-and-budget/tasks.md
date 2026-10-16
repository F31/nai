# Tasks: W126 — 升级下载阶段的续传、重试预算与哈希闸门

- [x] 1. 新增 `src/download.rs`：`PackageFetcher` / `ResumeLedger` 两个接缝 + `download_package` 编排（短读=值、`Err`=终局、偏移读回、哈希闸门）
- [x] 2. `verify.rs` 的哈希助手提为单一来源（`pub(crate) fn sha256_hex`），下载路径**引用**它而不是复制摘要
- [x] 3. 新增 `tests/download.rs` **20** 条判据（脚本传输 18 + 真实 loopback TCP/HTTP/1.1 **2**）；本地全绿
- [x] 4. 注入验证：**10** 条独立注入（偏移读回、append 回读、预算边界、短读清点、超量守卫、损坏丢弃、哈希输入、声明上界、循环内停止、入口停止）各自必须变红，且逐条可归因
- [x] 5. 删掉一条**重复守卫**（bound 之后那条 `checkpoint` 与入口同规则、无独立可观测差异）；为两条停止守卫各补一条判别性判据
- [x] 6. spec：`upgrade-transaction` +3 Requirement（续传读存储 / 重问有界且耗尽具名 / 哈希不符拒绝并丢弃）
- [x] 7. registry：`R09` 就地订正，写明本轮覆盖与剩余外部缺口，**维持 `implemented`**
- [x] 8. 文档：ADR 0104 + W126 报告 + `docs/upgrade.md`（接缝表与「未做边界」就地订正）+ development-plan/knowledge 注记
- [x] 9. 全量门控 + 一次 commit + push + CI 观测

归档 → `validate --archived` → commit/push → CI 双平台观测 → 回填报告。

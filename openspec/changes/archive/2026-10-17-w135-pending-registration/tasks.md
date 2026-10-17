# Tasks: W135 — 「未跟踪」不等于「未登记」

## 1. 勘定（先量，再写判据）

- [x] 1.1 读 `scripts/contracts/test-reachability.mjs`：`resolveSite` 的输入是 `.git/index`，
      且**刻意**不用 `readdir`（W74/W75 记录的理由）
- [x] 1.2 读 `git-index.mjs` 开头：模块明确「不依赖子进程」（W74 的 `EBUSY` 观测）
      ⇒ 补集读取器是**有意的例外**，必须在原地写清理由与代价
- [x] 1.3 现测补集规模：`git ls-files --others --exclude-standard -z` = **24,796** 条，
      其中 test-like **0** 条（全部来自未被忽略的 `target-check/`）
- [x] 1.4 现测派生进程可用：从 node 派生 `git ls-files` 成功（448ms）——但不把「这次能跑」当结论，
      失败路径仍按**具名失败**设计
- [x] 1.5 现测登记口径：`acceptance` **88** 条（tested 12 / implemented 26 / pending 42 /
      blocked 5 / not_applicable 3），与上轮逐项相同

## 2. 先写会红的判据

- [x] 2.1 `readUntrackedPaths`（`git-index.mjs`）：`ok: false` + `reason: 'spawn'`，MUST NOT 退化成空集
- [x] 2.2 `pendingTestFiles`（`test-reachability.mjs`）：返回 `{scanned, candidates}`，
      空集要**清点**成 0 而不是跳过
- [x] 2.3 `pendingRegistrationErrors`：补集里每个 test-like MUST 被某站点 marker 覆盖；
      扩展名不进闭集者具名；`present ∩ tracked ≠ ∅` 具名
- [x] 2.4 `tests/contracts/pending-registration.test.mjs` 11 条
- [x] 2.5 **现场证据（dogfood）**：先写测试文件、不登记、跑判据 ⇒ **红在它自己身上**，
      错误文本逐字点名 `tests/contracts/pending-registration.test.mjs` 并给出修法

## 3. 接线

- [x] 3.1 `TEST_REGISTRATION_SITES` 加 `xtask-pending-registration`（tier `portable`）
- [x] 3.2 `tests/registry.test.mjs` 逐字清单 16 → 17 个 id（注释同步）
- [x] 3.3 `xtask/src/lib.rs` 的 `engineering_checks` 加 `node --test` 行
- [x] 3.4 `tests/registry.test.mjs` 新增真实仓库用例 `the real repository reaches every test file
      that is about to be committed`（registry 56 → 57 条）
- [x] 3.5 修自造缺陷：新用例里的 `assert.deepEqual(candidates, [])` 把「清扫结果」当成「未登记集合」
      ⇒ 改成独立的覆盖复算（两处：契约测试 + registry 用例）

## 4. 注入与门控

- [x] 4.1 注入 I1（真实缺陷类，不打补丁）：放一个未登记未跟踪的探针文件 ⇒ 新判据红；
      **旧闭世界（F0）依旧全绿** —— 盲区被复现
- [x] 4.2 I2 判据的 `covered` 分支失效 ⇒ 1/2/9 红
- [x] 4.3 I3 清扫谓词惰性 ⇒ **1 与 F0 同时绿**（缺口）；补「清扫不惰性」前提后 ⇒ 1/2/5/9 红
- [x] 4.4 I4 扩展名分支失效 ⇒ 5 红；I5 重叠检查失效 ⇒ 7 红
- [x] 4.6 I6 读取失败退化成空集 ⇒ 10 红；I7 缺入参检查失效 ⇒ 6 红
- [x] 4.7 I8 负方向（把已登记判成未登记）⇒ 3/4/7 红 ⇒ 负方向有判别力，不是铁锤
- [x] 4.8 两个源模块 `sha256sum -c` **逐字节还原**（注入基线是 fmt-clean 的最终字节）
- [x] 4.9 全量门控 `cargo xtask check --profile full`

## 5. 交付

- [x] 5.1 报告 `docs/reports/2026-10-17-w135-pending-registration.md`
- [x] 5.2 回填 `docs/development-plan.md` / `docs/knowledge.md` / memory
- [x] 5.3 `openspec validate w135-pending-registration --strict`

归档后仍要做（**流程项，不是任务**）：`archive -y` → 两条 `openspec validate`（含 `--archived`）→
一次 commit + push → CI 观测（两平台结论 + 完整步骤序列）→ 结论回填报告。

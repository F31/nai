# Tasks: W131 — S10 子句一「`local_only` 不外发」

- [x] 1. 勘定：逐条现测 `S10` 三条子句的仓库落点（①零落点；②③已建且**真执行**）；
      确认 `documents` 表无 `local_only` 列、`RemoteGateway` 零生产调用点
- [x] 2. 先写会红的判据：判定面 `crates/nai-knowledge-core/tests/egress.rs`（4 条）+ 执行体
      `crates/nai-storage/src/tests/egress_targets.rs`（6 条）+ 迁移用例 1 条；
      首跑捕获 **`E0432` ×1**（`egress` 模块不存在）与 **`E0433` ×2 / `E0609` / `E0599`**（共 4 条）
      ⇒ 证明判据真的在测待建之物
- [x] 3. 承载物：`migrations/sqlite/0010-document-local-only.sql`
      （`documents.local_only INTEGER NOT NULL DEFAULT 0`）；版本号成套**六处**同步
      （`migration::VERSION` 9→10 / `SQL` 数组 / `backup.schema.json` const / Rust 契约样本断言 /
      `schema.test.mjs` 备份样本含「上一版 MUST 被拒」+1 / TS 生成物）；
      `ImportedDocument.local_only` **必填**且无默认
- [x] 4. 实现：判定面 `crates/nai-knowledge-core/src/egress.rs`
      （`EgressCandidate`/`EgressWithholding`/`EgressPlan`/`plan_egress`：两桶闭世界 + 具名扣留 +
      判定单元是 `(document_id, revision)` + 空集报 `0`）；执行体 `Store::remote_egress_targets`
      （域 = 活动快照 ∩ 未撤销 ∩ `kb_scope`；标记从存储读；非 0 的存疑取值按被标记读）
- [x] 5. 正向半与接线登记：被扣留的文档 MUST 仍被本地 FTS5 检索返回（夹具走 `split_document` +
      `tokenize` **真实路径**，MUST NOT 用占位 token 的手搓块）；确认新增的是 Rust 集成测试，
      由 `cargo test --workspace` 自动纳入，**不需**在 `engineering_checks` 登记
      （`test-reachability` 只管 `.test.mjs`）
- [x] 6. 判据绿 + fmt 基线：注入前先跑 `cargo fmt --all --check` ⇒ **发现 1 处红**（新加用例里的
      `execute(...)` 需折行）⇒ 就地修掉再取基线（W129 的坑：基线 MUST 是 fmt-clean 的最终字节）。
      `nai-knowledge-core` **55/55**、`nai-storage --lib` **81/81**、`schema.test.mjs` **30/30**
- [x] 7. 注入验证：**13 条**注入真跑（判定面 5 / 执行体 5 / 迁移 1 / 写入侧 1 / TS 生成物 1）；
      **12 条真红**，红的断言层或用例逐条可归因；**1 条本机不红**（TS 生成物内容漂移，
      本机判据只断言「文件存在与清单对齐」、不比内容 ⇒ 只能由 CI 的 `cargo xtask schema --check` 抓）；
      5 个被注入文件全部 `sha256sum -c` 逐字节还原
- [x] 8. 文档：`docs/adr/0109-local-only-egress.md` + 轮次报告 +
      `docs/development-plan.md` 小节 + `docs/knowledge.md` 轮次注记
- [x] 9. registry `S10`：notes 就地订正（现测：子句一此前零落点、子句二/三已建），
      `status` 判定**由证据决定**，MUST NOT 预先承诺 ⇒ 结论：**保持 `implemented` 不升**
      （四环：①②已建且有覆盖，③调用方未闭合——`sync` 不消费这份清单且零生产调用点）
- [x] 10. 全量门控 `cargo xtask check --profile full`（`portable` ⊂ `full`）⇒ **绿，7m51s**。
      Rust **1058 passed / 0 failed**（W130 为 1045 ⇒ **+13** == 本轮新增 4 判定面 + 8 执行体 +
      1 迁移，逐项吻合）；`^     Running ` **81**（80 目标 + 1 条 cargo 自回显的 xtask 包装行）、
      `   Doc-tests` **18** ⇒ `test result` **98**；Node **13 套件 324/0**（与 W130 持平）；
      错误类（`LNK1104`/`error[`/`out of memory`/`FAILED`/`panicked`）**0**。
      ⚠️ 前三次门控红，每次红在 `apps/engine` 内**不同**的链接目标
      （`pipeline` → `engine` → `nai_engine` bin）且**全在链接期**（`test result` 计数为 0，
      一条测试都没跑）⇒ 四条替代判据：错误类同族且全在 bring-up／失败集合**跨目标漂移**／
      `apps/` `git status` 为空／逐目标单独跑全绿（`pipeline` 13/13、`engine` 2/2）⇒ 宿主瞬态，
      **不是代码缺陷，未改任何代码**；另以「有界重试预热」6 次把 workspace 测试目标链接齐后，
      门控一次即绿（E 盘剩 48.3GB、目录可写 ⇒ 排除磁盘满）
- [x] 11. 闭环：`openspec archive --yes` + 两条 validate（`--all --strict` / `--archived`）+
      一次 commit + push + CI 观测（触发确认 1 次 + 完成态 ≥2 次，绝不轮询）+ 报告回填 +
      docs-only 回填提交 + 临时文件清理 + memory 三处

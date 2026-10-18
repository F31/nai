# Tasks: W144 — 作业面闭世界

## 1. 勘定

- [x] 用 `SwaggerParser.parse` 逐 `path × HTTP_METHODS` 量作业面：`control.yaml` = 15 条，`remote-knowledge.yaml` = 0 条
- [x] 现测 15 条全部 `$ref #/components/schemas/JobAccepted`、全部无 `x-implementation-status`、`jobOps ∩ implemented = []`
- [x] 现测 `JobView.kind.enum = ["upgrade"]`、`JobAccepted.required = [job_id, command_id, status]`
- [x] 现测生产侧零生产者：`grep -rn "JobAccepted" --include=*.rs crates/ apps/` 只命中生成物 `SCHEMAS_JSON`
- [x] 定位 15 条在路由表里的落点：零分支，全部落 `lib.rs:665` 的兜底 404 `CapabilityAbsent`
- [x] 定位 `blockedBy` 两类的源码锚点：`:665`（`control operation not implemented`）与 `:1121`（`no job id is minted`）
- [x] 定位 `kind` 的 journal / reader：`crates/nai-storage/src/upgrade.rs` 的 `UpgradeRecord`、`lib.rs:1351` 的 `"kind": "upgrade"`
- [x] 实测口径坑：`validate` 把 `$ref` 内联（量出 0），`parse` 保留字面（量出 15）
- [x] 实测 YAML 别名共享引用：`paths['/backups'].post.responses === paths['/providers/{id}'].put.responses` 为真，且 `structuredClone` 保留该共享

## 2. 判据

- [x] 新建 `scripts/contracts/job-surface.mjs`：`JOB_SURFACES` / `JOB_KINDS` / `JOB_BLOCKERS` 单一来源
- [x] `jobSurfaceErrors`（契约侧）：202 扫描双向、`$ref` 唯一、`kind`/`blockedBy` 互斥、闭集、`kind` ⇔ 已实现、`JobView.kind` 双向、文档键集双向、`status` 取值域、非空洞
- [x] `jobSourceErrors`（仓库侧）：锚点文件 MUST 被跟踪、MUST 可读、文本 MUST 还在；三种失败分开报
- [x] `tests/contracts/schema.test.mjs` 4 条用例：真实文档已闭 / 互斥与闭集 / 双向与非空洞 / 锚点
- [x] 合成样本 MUST NOT 从真实文档克隆（别名共享会让「改一处」变成「改 14 处」），用 `jobDocument()` 构造
- [x] 注入验证逐条真红：变更登记表删一条 / 把 `upgrades/execute` 标成 `kind` / 把 `JobView.kind.enum` 清空 —— 三条各产生**恰好一条**具名错误
- [x] 锚点判别性反例：裸词 `"upgrade"` 在读者文件里出现 >1 次、带判别键的形式恰好 1 次

## 3. 契约

- [x] `schemas/openapi/control.yaml`：`JobAccepted` 补 description（`accepted` 只表示收到，能否轮询由 journal 决定）
- [x] `cargo xtask schema` 再生；`cargo xtask schema --check` = no drift

## 4. 裁定与文档

- [x] `docs/adr/0118-job-surface-closed-world.md`：四个被否决方案、三条具名残余、计数订正
- [x] 本 proposal 的 Why 段 ≤ 1000 字符（样本，不是判据）
- [x] `docs/reports/2026-10-18-w144-job-surface-closed-world.md`
- [x] `docs/development-plan.md` 追加 W144 段

## 5. 闭环

- [x] `cargo fmt --all --check` / clippy `-D warnings` / `cargo xtask check --profile full`
- [x] 两条 openspec validate；归档
- [x] 一次 commit + push（CI 两平台观测与报告 §六 / §七 的回填见回填提交，`docs-only`）
- [x] 清理探针 `.w144-probe.mjs`

# 任务（w75 仓库受限的消费证据）

## 1. 实现

- [x] 1.1 新增 `scripts/contracts/git-index.mjs`：`parseIndex(buf)`（纯函数）与
      `readTrackedPaths(root)`；支持 DIRC v2/v3/v4，`.git` 为文件时读 `gitdir:` 指针；
      失败分类为 `missing`/`bad-magic`/`unsupported-version`/`truncated`/`corrupt`
- [x] 1.2 `tests/contracts/schema.test.mjs` 的 `codeFiles()` 路径改为 POSIX 相对路径（修 F6，
      初始目录 `'.'` 时不产生 `./` 前缀）
- [x] 1.3 `consumptionOf(files)` 改为返回 `{consumed, evidenceSources}`，并更新全部调用点
- [x] 1.4 新增 `repositoryErrors(evidenceSources, tracked)` 与断言
      「证据来源 MUST ⊆ 已跟踪路径集」＋非空洞断言（`evidenceSources` MUST 非空）
- [x] 1.5 新增解析器字节流单测：v2、v3（含 extended flags）、v4（公共前缀压缩）、
      坏 magic、截断、版本不支持、corrupt、空索引——并入既有 `schema.test.mjs`
      （`engineering_checks` 零改动）
- [x] 1.6 新增断言：扫描集里 MUST NOT 出现以 `./` 开头的路径（F6 的守卫）

## 2. 验证（非空洞注入，逐项还原）

- [x] 2.1 复现 W74 的洗绿攻击：未跟踪的伪造消费者 + 真孤儿 ⇒ **被堵**（V1a）；
      并测出「形状断言被放宽后 `repositoryErrors` 独立兜住」（V1c）、「两条都停用则 17/17 全绿」（V1b）
- [x] 2.2 注入：索引不可解析 ⇒ 校验失败且消息**可诊断**（V2a `missing`、V2b `bad-magic`）
- [x] 2.3 注入：让扫描集出现 `./` 前缀 ⇒ 路径格式断言变红（V3）
- [x] 2.4 每组注入都以编辑回退确认还原；`git status` 无残留探针

## 3. 文档

- [x] 3.1 `docs/adr/0053-repository-scoped-evidence.md`
- [x] 3.2 活文档就地订正：`AGENTS.md:1512`「`git ls-files` 取仓库文件集这条路不可行」＋追加 W75 轮次记录
- [x] 3.3 历史记录**保留原文 + 追加勘误段**：`docs/adr/0052` 两处（备选表行、§八遗留 1）
      与 W74 报告（§八遗留 1、§九教训 2）
- [x] 3.4 报告 `docs/reports/2026-10-12-w75-repository-scoped-evidence.md`
- [x] 3.5 `docs/development-plan.md` 追加 W75 轮次记录并在「下一主线」段登记

## 4. 闭环（执行记录，非勾选项）

4.1 `cargo xtask check --profile full` EXIT=0（目标 91 = 73 `Running` + 18 `Doc-tests`、858/0）
4.2 `openspec archive -y w75-repository-scoped-evidence`
4.3 两条 validate（`--all --strict` 与 `--all --strict --archived`）
4.4 清理 `dist/` 下的临时探针与门控日志
4.5 一次 commit + push
4.6 单次 CI 观测并回填报告——**特别核对**「CI 上 `.git/index` 可解析」这条假设

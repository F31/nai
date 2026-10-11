## 1. 规格与设计

- [ ] 1.1 建立 OpenSpec 变更 `w43-ci-failure-annotation`，新增 `engineering-foundation` 差量需求（3 条需求 / 3 个场景），`validate --strict` 通过。
- [ ] 1.2 落设计决策 D1–D7（用 annotation 而非 summary/artifact、统一 `shell: bash`、`set -o pipefail` 的必要性、诊断逻辑入仓库脚本、恒 0 退出 + `continue-on-error`、逐行与信号优先、临时分支端到端验证、不把 CI 语法写进 Rust 工具）。

## 2. 诊断脚本

- [ ] 2.1 新增 `scripts/ci-diagnose.mjs`：参数解析、ANSI 剥离、`%`/`\r`/`\n` 转义、失败信号优先 + 尾部兜底、逐行与长度截断、省略提示；对日志缺失/为空/读失败一律退出 0 且不输出 annotation。
- [ ] 2.2 本机功能验证：缺文件、空文件、参数错误（退出 2）、含 ANSI 与特殊字符（`%`、`::`、`,`）与超长行的合成样本、信号优先与省略提示（`--max-lines 5`）、**真实** xtask 具名拒绝日志。

## 3. workflow 接入

- [ ] 3.1 门控步改为 `shell: bash` + `set -o pipefail` + `tee "$RUNNER_TEMP/nai-gate.log"`。
- [ ] 3.2 新增诊断步骤：`if: failure()`、`shell: bash`、`continue-on-error: true`。
- [ ] 3.3 **`pipefail` 反向自检**：有/无 `set -o pipefail` 的同一条命令退出码对比（缺失时 `tee` 会把失败读成成功）。
- [ ] 3.4 用 Node 解析 workflow 做结构断言（临时脚本，跑完即删）。

## 4. 端到端验证

- [ ] 4.1 `main` 推送后观测正常路径：两个 job 全绿，且门控步与诊断步的结论符合预期。
- [ ] 4.2 推临时分支 `ci-annotation-probe`（只把门控命令换成 `check --profile bogus`），观测**两个平台**的
  check-run annotations 均能在**匿名**条件下读到具名拒绝信息。
- [ ] 4.3 删除 probe 分支。

## 5. 文档与闭环

- [ ] 5.1 新增 `docs/reports/2026-10-11-w43-ci-failure-annotation.md`（含 probe 的实测输出与「会留下一个失败 run 记录」的代价披露）。
- [ ] 5.2 更新 `AGENTS.md`、`docs/development-plan.md`、`tests/registry.yaml` 的 D01 notes（**不改任何 `status`**）。
- [ ] 5.3 勾选 tasks，`openspec archive w43-ci-failure-annotation -y`，`validate --all --strict --archived` 通过。
- [ ] 5.4 一次 commit + push；更新项目记忆。
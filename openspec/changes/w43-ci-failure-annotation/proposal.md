## Why

W42 的勘误段确立了一个仓库事实：**CI 的失败详情在无凭据时不可读**。三个实测结论：

- `GET /repos/F31/nai/actions/jobs/{id}/logs` 匿名返回 **403 `Must have admin rights to Repository.`**；
- `GET /repos/F31/nai/check-runs/{id}/annotations` 匿名**可读**，但目前只有 GitHub 自动生成的
  `Process completed with exit code 1.`——**没有任何失败原因**；
- 因此近 30 次运行里那 **13 次 ubuntu `Portable foundation` 间歇失败**（W42 已登记）只能定位到**步级**，
  定位不到**测试级**：本机非 Linux（`wsl.exe` 被安全策略禁止、无 Docker）、无 `gh` CLI、无仓库凭据。

也就是说：**CI 在跑，失败在发生，但没有人能在不登录的情况下知道为什么。** 这不是「本机无法观测」，
而是观测链路缺了一环——缺的那一环恰好是唯一**匿名可读**的那条（annotation）。

GitHub Actions 把 workflow command（`::error::…`）解析成 check-run 的 annotation，而 annotations API 匿名可读。
**把门控日志的失败摘要转成 annotation，就能用匿名可读的通道传递失败原因**——不需要新权限、不需要 runner、
不需要凭据。这条通道本身也需要被钉进规格：它的唯一价值就是「可观测」，若不写进规格，后人会把它当冗余删掉。

## What Changes

- 门控步骤改为 `shell: bash`，输出经 `tee` 落盘到 `$RUNNER_TEMP/nai-gate.log`，并**显式 `set -o pipefail`**。
- 新增 `scripts/ci-diagnose.mjs`：读该日志，按「失败信号行优先 + 尾部兜底」挑选，**逐行**转成 `::error::` annotation。
- 新增一个 `if: failure()` 的诊断步骤调用该脚本，并置 `continue-on-error: true`。
- `engineering-foundation` 新增三条需求：失败摘要必须以匿名可读的 annotation 发布；采集输出不得改变门控判定；
  无内容时不得制造 annotation。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `engineering-foundation`：新增三条需求——门控失败 MUST 以**无需凭据**即可读的 annotation 发布失败摘要；
  采集输出 MUST NOT 改变门控步骤的退出码、诊断步骤 MUST NOT 改变失败结论；日志缺失或为空时 MUST NOT 产生任何 annotation。

## Impact

- 新增：`scripts/ci-diagnose.mjs`；报告与 OpenSpec 变更文档。
- 修改：`.github/workflows/foundation.yml`（门控步 + 诊断步）、`openspec/specs/engineering-foundation/spec.md`（经归档吸收）、
  `AGENTS.md`、`docs/development-plan.md`、`tests/registry.yaml`（D01 notes）。
- **零新依赖**：复用 CI 已安装的 Node；不新增 workspace 包（仍 18），不触碰 Rust 源码。
- **零源码回归面**：`xtask`、各前端包与 `scripts/frontend-gate.mjs` 一行未改。
- **不改任何验收 `status`**。
- **验证代价（如实披露）**：要证明这条通道真的可读，必须让 CI **真的失败一次**。做法是在**临时分支**上把门控命令
  改成必然失败（`check --profile bogus`），观测两个平台的 annotations 后删除该分支。
  ⇒ 仓库会**永久留下一个失败 run 记录**（GitHub 的 run 历史无法删除）。这是本轮已知的、可接受的代价。
- **不做**：修那个 ubuntu 间歇失败本身（原因未知，需 admin 日志或等通道上线后再诊断）；不引入
  `act`/Docker 等本机复现手段；不把 GitHub 特有语法写进 Rust 工具（诊断逻辑只在 CI 脚本里）。
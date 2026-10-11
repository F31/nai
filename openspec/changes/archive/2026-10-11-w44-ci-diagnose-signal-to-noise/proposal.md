## Why

W43 建立了一条**匿名可读**的失败诊断通道：门控输出经 `tee` 落盘，失败时由 `scripts/ci-diagnose.mjs`
把摘要转成 check-run annotation。它的**第一次真实使用**就发生在 W43 自己的勘误提交上——

- run [`38110014189`](https://github.com/F31/nai/actions/runs/38110014189)（`81ce88b`）的
  `foundation (ubuntu-24.04)` 第 10 步失败、第 11 步诊断步成功执行；
- 匿名读到 `GET /check-runs/114383450000/annotations`，首次拿到**测试级**结论：
  `test halt_during_media_drill_prewarm_never_commits_a_frame ... FAILED`
  —— 这正是 W42 登记、历轮从未定位到测试级的 ubuntu 间歇失败。

**通道能用，但摘要在这次真实失败里质量很差**：11 条注解只有 1 条是信号，另外 10 条是噪音。
三类缺陷各有明确的、可复现的成因：

1. **裸 `error` 子串**：`STRONG` 正则用 `error` 做关键词，于是 `Checking thiserror v2.0.21` 与
   `Compiling thiserror-impl v2.0.21` 每次编译都被判成「强信号」（`thiserror` 含 `error`）。
2. **无关行填满配额**：信号稀少时，选行逻辑用日志里的普通行补足到上限，把唯一一条真信号淹没。
3. **通过测试被当成信号**：本仓库大量测试**名字里含信号词**
   （`…_cors_queries_unimplemented_routes_and_expired_access_are_explicit … ok`），关键词匹配把一个
   **通过**的测试判成强信号，并把它的后置上下文（就是那两行 `thiserror`）一起拖进摘要。

还有第四个、性质不同的观察：**这次采集到的日志只有 11 行**，而该 job 从 `started_at` 到 `completed_at`
跑了 **2 分 50 秒**。一份完整的 `check --profile full` 必然产出数千行；摘要里既没有 `test result:`
汇总行，也没有省略提示（省略提示只在 `omitted > 0` 时输出，而 `lines.length === 11` 时不会出现）。
**一份被截断的日志正在被当成全部证据**——必须让摘要自己点破这一点。

## What Changes

- `scripts/ci-diagnose.mjs`：
  - `STRONG` 里的 `error` 由裸子串改为 `\berror(\[|:)`（要求紧跟错误码方括号或冒号），`thiserror` 不再命中。
  - 新增噪音类：libtest 的**通过/忽略**逐测试行（`test <name> ... ok|ignored`）永不入选——
    判据从「这行含不含某个词」改为「这行报告了失败吗」。
  - 选行改为「强信号 → 其后置上下文与所属目标头 → 弱信号」；**一条信号都没有时**才退化到尾部窗口，
    **绝不**用无关行填满配额。
  - 新增目标头前查（`Running tests/xxx.rs …`）：测试名本身不足以说明失败落在哪个二进制。
  - 新增**截断自证**：出现逐测试行却没有 `test result:` 汇总行时，输出一条 `::warning::` 说明日志不完整并给出总行数。
- `engineering-foundation` 新增两条需求：摘要以信号为中心（不得用填充行淹没信号；通过测试的名字不得构成信号）；
  采集不完整时必须明示。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `engineering-foundation`：新增两条需求——**失败摘要 SHALL 以信号为中心而非填充**；**采集不完整 MUST 被明示**。

## Impact

- 修改：`scripts/ci-diagnose.mjs`、`openspec/specs/engineering-foundation/spec.md`（经归档吸收）、报告与 OpenSpec 变更文档。
- **零新依赖、零 Rust 源码改动、零 workspace 包增减**；`.github/workflows/foundation.yml` **不改**——
  本轮全部改动都在诊断脚本内部，接线沿用 W43。
- **不改任何验收 `status`**。
- **验证方式**：`scripts/*.mjs` 不在工程门控内，故以本机端到端样本验证并**如实记录**：
  截断样本（复刻本次真实内容）、完整失败样本、以及 W43 的 2307 行噪声样本回归。
- **不做**：不修 `halt_during_media_drill_prewarm_never_commits_a_frame` 本身（本轮只把结论推进到测试级，
  根因定位另开一轮）；不引入 `act`/Docker；不把 GitHub 特有语法写进 Rust 工具。
- **需登记的事实**：本次 ubuntu 失败把「间歇失败只有步级结论」推进到「有测试级结论」，
  但**采集日志为何只有 11 行仍未定**——这一点如实记入报告，并已由新的截断自证在下次失败时自动点破。

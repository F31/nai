# ADR 0121：手工预构建 MUST 与门控测试步同批单元 —— 并且它不叫「造一个 clippy 的替代物」

- 状态：已接受（W147，2026-10-18）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`
- 相关：W146 报告 §六第二段与 §八第 5 条（**本轮订正其归因**，勘误见该文 §九）、
  `docs/engineering.md`（活文档，处方正文）、`scripts/contracts/gate-coverage.mjs`、
  `tests/contracts/gate-coverage.test.mjs`、`xtask/src/lib.rs` 的 `engineering_checks`、
  `scripts/contracts/xtask-commands.mjs`（`superseded-by-workspace-run` 的逐字凭据）

## 背景

W146 的门控连红四次，全是链接期 `LNK1104`、且**失败目标每次都不同**；第 5 次同参数通过。
那一轮先做了 `cargo build --workspace --all-targets` 串行预构建（`EXIT=0`、`LNK1104` 0 次），
紧随其后的门控**仍红在链接**，于是记下一条成因：

> 门控第一步跑 `clippy`，clippy 与 rustc 的产物指纹不同 ⇒ `cargo test` 阶段重新链接，
> 预构建成果在门控内部被作废。

这条成因被写进了报告、开发计划与本轮记忆。它会被下一次会话当成指令执行——所以它必须是对的，
或者必须被改掉。W147 先做实测，结果是它**错了**。

## 证据（本机，`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1`，数 `Compiling` 行）

| 组 | 序列 | 结果 |
| --- | --- | --- |
| M | `build --workspace --all-targets` → `clippy --workspace --all-targets -- -D warnings` → `build` | 8 → **0** → **0** |
| A | 改一个 `.rs` 后 `build -p nai-xtask --lib` → `clippy -p nai-xtask --lib` → `build -p nai-xtask --lib` | 1 → 1 → **0** |
| B | `build --workspace --all-targets` → `test --workspace --exclude nai-xtask --locked --no-run` → `build --workspace --all-targets` | 1 → **7**（`LNK1104` 0）→ **7**（同一批 7 个包，不含 xtask） |

M 与 A 都指向同一件事：**clippy 不判脏 rustc 产物**。B 指向真正的原因：**单元集不对齐**。
`--all-targets` 展开为 `--lib --bins --tests --benches --examples`，其中 `--tests`
**只类型检查测试源码、不产 test harness**；`cargo test` 要编译并**链接**测试二进制，
而那四次 `LNK1104` 的目标（`pipeline-*`／`engine-*`／`nai_engine-*` 的 bin test）恰恰只在
`cargo test` 里链接。两套单元集**双向互不包含** ⇒ 用 `build --all-targets` 预构建，
门控该编的还是没编。

## 决定一：处方用两条 `--no-run`，并写进**活文档**、由判据对账

处方 = 门控两个测试步各加一个 `--no-run`：

```
cargo test --workspace --exclude nai-xtask --locked --no-run
cargo test --locked --package nai-xtask --lib --no-run
```

它落在 `docs/engineering.md` 的 `<!-- gate-prebuild:begin -->` / `<!-- gate-prebuild:end -->`
标记块里。选活文档而不是报告，是因为**报告是已发布记录、处方是会变的东西**：门控的测试步一旦
改动，处方必须跟着动，而「跟着动」只能发生在可编辑的载体上。

判据 `prebuildRecipeErrors()` 的形状是**双向一一对应**：文档每条命令 MUST 等于门控某条测试步
**+ 恰好一个 `--no-run`**，门控每条测试步 MUST 有且只有一条文档命令对应。旗标**按集合**比较
（`--locked` 写在 `--exclude` 前还是后是同一批单元），但**多少个旗标、哪几个旗标**必须一致——
这正是「换了一个旗标」与「换了个顺序」的分界。逐 token 取值的解析器（`testCommandsOf`）是从
既有的 `coveragesOf` 里**抽出来共用的**：`testCoverage` 会把命令降维成包集合，`--lib`/`--no-run`
被刻意丢弃，而这一层问的恰恰是被丢弃的那部分。

## 决定二：不改门控形状（撤回「让 clippy 与 test 共用产物」）

W146 §八第 5 条给了两条出路：让 clippy 与 test 共用同一份产物，或把 `-D warnings` 挪到独立步骤。
两者都会改门控形状，而实测表明它们修的是**不存在的成因**。额外的代价是具体的：门控两条测试步的
字面量是 `xtask-commands.mjs` 里 `superseded-by-workspace-run` 的凭据 marker，重排或移动它们会
打红一条既有覆盖；新增一条 `xtask` 子命令会打红 `xtask-commands.test.mjs` 里**逐字钉住的 24 条
签名清单**。用两处判据代价换零收益，不做。

## 后果

- 门控**一个字节没改**（`xtask/src/lib.rs` 本轮零改动）：这一条本身就是「归因错了」的旁证——
  正确的成因不需要改被测系统。
- 处方会漂：门控加/删测试步、换覆盖形态，`gate-coverage` 会红在具体的那一条上（注入实测：
  文档漏 `--no-run`、文档改 `--exclude` 的值、门控侧 `--workspace` 换 `-p`、删标记块、
  文档多一条命令，五条各自变红；且**前四条在既有的 24 条用例上零信号**）。
- 本机的 `LNK1104` 仍可能出现，但「预构建救不回来」不再是一个未解释的现象。它是宿主事实
  （候选面见下一条），不是仓库缺陷。

## 不做什么

- **不判定 `LNK1104` 的宿主原因**。本轮只把候选面收窄到「注册的杀软里只有 McAfee 在跑」
  （Defender 的 `RealTimeProtectionEnabled = False`、`Get-MpPreference` 报 `0x800106ba`），
  且 `tasklist` 无残留测试进程、E: NTFS Healthy。一条排查不能把一个在跑的扫描器和一次链接失败
  之间的因果钉死，所以它是**残余**，不是结论。
- **不拆 `CARGO_TARGET_DIR`**。CI 无 cargo 缓存（workflow 里 `cache`／`swatinem`／`rust-cache`
  零命中），所以拆目录对 CI 零成本；但本机要付一次冷编译（1m47s 量级），且 `build` 与 `clippy`
  共用目录会互相判脏。它治的是「链接时被占用」而不是「预构建编错了东西」，与本轮的问题不同。
- **不在判据里验处方是否真的跑过**。判据只钉「处方与门控同批单元」这一条对应关系；它跑没跑、
  跑的时候宿主是否抖，属于执行面事实，不在静态判定里假装知道。

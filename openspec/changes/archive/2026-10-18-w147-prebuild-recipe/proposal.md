# Proposal: W147 — 预构建处方必须与门控测试步同批单元

## Why

W146 门控连红四次（链接期 `LNK1104`，每次目标不同），第 5 次同参数通过。那一轮先做了
`cargo build --workspace --all-targets` 串行预构建（`EXIT=0`、`LNK1104` 0 次），门控**仍红在链接**，
于是记下成因：门控先跑 `clippy`，clippy 与 rustc 产物指纹不同 ⇒ 预构建在门控内部被作废。
这条成因写进了报告、开发计划与记忆，会被下一次会话当成指令执行。

本轮先实测再动手，结果**否证**它：clippy 不判脏 rustc 产物（`build` 8 → `clippy` 0 → `build` 0；
改一个文件后 1 → 1 → 0）。真因是**单元集不对齐**——`--all-targets` 里的 `--tests` 只类型检查、
不产 test harness，而 `cargo test` 要链接测试二进制（那四次 `LNK1104` 的目标正在这一批）。

故本轮不修宿主、不改门控：把正确的处方写进活文档、立一条双向对账判据、订正三处错误记载。

## What Changes

- `docs/engineering.md`：加 `<!-- gate-prebuild:begin/end -->` 标记块，正文为与门控两个测试步
  同批的两条 `--no-run`，并写清 `build --all-targets` 为什么不算预构建、clippy 为什么不是原因。
- `scripts/contracts/gate-coverage.mjs`：抽出 `testCommandsOf`（不降维的逐 token 取值，
  `coveragesOf` 改建在它上面——同一份解析，不写第二套）；新增 `PREBUILD_BLOCK`、
  `documentedPrebuildSteps`、`commandSignature`、`prebuildRecipeErrors`。
- `tests/contracts/gate-coverage.test.mjs`：+7 条用例（24 → 31），五条文件级注入逐条验证变红。
- `docs/adr/0121-prebuild-recipe-unit-set.md`、轮次报告、`docs/development-plan.md` W147 段；
  W146 报告追加 §九勘误（已发布记录不就地改），`development-plan` 与记忆就地订正。

## Impact

- **门控零改动**：`xtask/src/lib.rs` 一个字节不改，不新增 `xtask` 子命令 ⇒ 不撞
  `xtask-commands.test.mjs` 逐字钉住的 24 条签名清单，也不动 `superseded-by-workspace-run` 的凭据。
- **行为零变化**：只加判据与文档，不动任何被测实参、不动契约、不动生成物。
- 判据面：Node +7 条用例，不新增门控目标、不新增依赖、不需要 protoc。
- 本机 `LNK1104` 仍是残余（候选面收窄到「注册的杀软里只有 McAfee 在跑」），本轮**不判**它的宿主原因。

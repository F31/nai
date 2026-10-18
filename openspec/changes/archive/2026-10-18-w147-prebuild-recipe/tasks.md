# Tasks: W147 — 预构建处方必须与门控测试步同批单元

## 1. 勘定（先证成因，别先改东西）

- [x] 量 clippy 是否判脏 rustc 产物：`build --all-targets` → `clippy` → `build` 的 `Compiling` 数
      = **8 → 0 → 0**；改一个文件后 `1 → 1 → 0` ⇒ 否证 W146 的成因
- [x] 量 `build --all-targets` 与 `cargo test` 的单元集：`1 → 7 → 7`（同批 7 个包，不含 xtask）⇒ 真因
- [x] 量 `LNK1104` 的候选面：Defender `RealTimeProtectionEnabled=False` / `Get-MpPreference` 报
      `0x800106ba`；注册杀软 Defender + McAfee，`mc-fw-host`×2 / `mc-neo-host` / `McAfee WebAdvisor`
      在跑；`tasklist` 无残留测试进程；E: NTFS Healthy
- [x] 确认 CI 无 cargo 缓存（workflow 里 `cache` / `swatinem` / `rust-cache` 零命中）⇒ 拆产物目录对 CI
      零成本、对本地要付一次冷编译 ⇒ 降级为残余而非主修

## 2. 处方与判据

- [x] `docs/engineering.md` 加 `gate-prebuild` 标记块 + 两条 `--no-run`，并写清两个反直觉事实
- [x] `gate-coverage.mjs` 抽出 `testCommandsOf`（`coveragesOf` 改建在它上面，不写第二套解析）
- [x] 新增 `PREBUILD_BLOCK` / `documentedPrebuildSteps` / `commandSignature` / `prebuildRecipeErrors`
- [x] 双向一一对应：文档 == 门控测试步 + 恰好一个 `--no-run`；旗标按集合比、按集合验
- [x] 门控自己带 `--no-run` 具名失败（预构建是手工步骤，不是门控的一部分）

## 3. 用例与注入

- [x] 真实仓库用例：处方逐行对应、两侧非空洞、门控不含 `--no-run`
- [x] I1 文档第一条漏 `--no-run` ⇒ 25/27/31 红（sha256 还原）
- [x] I2 文档把 `--exclude nai-xtask` 改成 `--exclude nai-engine` ⇒ 25/31 红（sha256 还原）
- [x] I3 门控侧 `--workspace --locked --exclude HARNESS` → `-p nai-storage --locked` ⇒ 25/28/31 红，
      另有既有 1/9/11/15/23（包集合守卫本来就管这一段）（sha256 还原）
- [x] I4 标记块 `begin` 被改名 ⇒ 25/26/27/28/30/31 红（sha256 还原）
- [x] I5 文档多一条 `cargo test -p nai-storage --locked --no-run` ⇒ 25/27/28/31 红（sha256 还原）
- [x] I1/I2/I4/I5 在**既有 24 条用例上零信号**；29（门控自己预构建）不被 I1–I5 打红 ⇒ 判别性成立
- [x] 第 6 类：门控侧加 `--no-run` ⇒ 具名「belongs to the documented recipe」

## 4. 文档

- [x] `docs/adr/0121-prebuild-recipe-unit-set.md`
- [x] W146 报告追加 §九勘误（已发布记录不就地改）
- [x] `docs/development-plan.md` W146 段就地订正 + 追加 W147 段
- [x] `.workbuddy/memory/2026-10-18.md` 里那条错误记载就地订正
- [x] `docs/reports/2026-10-18-w147-prebuild-recipe.md`

## 5. 闭环

- [x] `cargo xtask check --profile full`（第 2 次 `EXIT=0`；首次红在 registry 的待登记检查——备份目录里
      有一份测试文件副本，与代码无关，删掉即绿）
- [x] `openspec validate --all --strict`（归档前 **44/0**、归档后 **43/0**）；`--archived` 在**提交时是
      `143 / 1`**（22/26 任务未勾），勾完闭环任务后 **144 / 0** ⇒ 顺序错了，见报告 §六末段
- [x] 一次 commit + push + CI 双平台观测（提交 `f1c5093` → run `39123291976`：两平台**同一处红**在
      第 16 步 `openspec validate --archived`，成因就是上一条的顺序错误，不是代码缺陷）
- [x] 回填报告 §六/§七 与 plan 两处占位

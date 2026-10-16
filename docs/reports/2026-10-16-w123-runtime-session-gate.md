# W123 报告：装配根接上真实会话，回答升级激活门（R10 接线）

日期：2026-10-16｜归属：按建议啃 R1「本仓可闭合」项，首选 **R10**｜**含生产代码改动**

## 一、裁定与缝（现测）
R10 的缺口 = 「真实会话注册表（装配根）未接线」。实测：`grep SessionGate crates/` 只有
`nai-upgrade-core/src/gates.rs`（trait + `SessionGate`）与 `rehearsal.rs`（`RehearsalSessions`，内存 `AtomicUsize`）；
`grep -rn "SessionGate\|UpgradeTransaction\|Multiplexer" crates/nai-runtime/src apps/engine/src` **无任何命中**
⇒ 装配根从未接过门。

## 二、语义分叉（用户裁定）
`active()` 取 **A：在播才活跃**（`Running` / `Draining`）。未取「一切未停止的会话」——本装配 `prepare` 后会话即
`Created`，那会让激活**永久 defer**，门恒挡＝死门而非谨慎。

## 三、改动（生产代码）
- `crates/nai-runtime/src/composition.rs`：`impl nai_upgrade_core::ActiveSessionGate for Composition`，读真实
  `Session` 的 lifecycle；**busy/poisoned ⇒ 返回 `Conflict`/`Internal`，绝不返回 0**（fail closed）。
- `crates/nai-runtime/Cargo.toml`：新增 `nai-upgrade-core`（`runtime → shared` 合法）。
- `crates/nai-runtime/tests/composition.rs`：新增 2 条判据。

## 四、判据（真实证据，12 → 14）
- `the_upgrade_activation_gate_reads_the_real_session_lifecycle`：Created ⇒ Allowed；推到 **Running** ⇒
  `Deferred{active:1}` 且 `require_idle` ⇒ `CapabilityNotReady`；**Draining** ⇒ 仍 Deferred；**Stopped** ⇒ Allowed。
- `the_upgrade_activation_gate_fails_closed_when_the_session_is_unobservable`：用既有 `hold_session` **握手**造出
  已被持有的会话锁 ⇒ `activation` 返回 `Conflict`（**不是** Allowed）。不靠 sleep 赌调度。
- 本机：`cargo test -p nai-runtime --test composition` ⇒ **14 passed; 0 failed**。

## 五、registry 判定（**维持 `implemented`**）
本轮接的是「装配根**能**答 gate」；运行时**仍无升级激活调用点**（无 `UpgradeTransaction` 使用处）⇒
「开播中只暂存不激活」尚未在真实链路上被执行。按 ADR 0100 同一条规矩：**守卫通过 ≠ 本项达标**，
故 R10 维持 `implemented`，notes 记本轮进展与剩余缺口。

## 六、门控（全量 `cargo xtask check --profile full`）
- `GATE_EXIT`：**0**（`PREBUILD_OK at attempt 1` + `GATE_OK at attempt 1`，首跑即绿，9m48s）；`rustc 1.96.0 (ac68faa20 2026-05-25)`。**LNK1104 = 0**（无链接瞬态）。
- **95** 个测试二进制全 `0 failed`，**真实测试失败 0**；clippy `-D warnings` 通过；`fmt --all --check` 通过；前端门控通过；registry 56/56。其中 `nai-runtime` 的 `composition` 目标 **14 passed; 0 failed**（含 W123 两条新增）。

## 七、提交 / 推送
- 提交 SHA：**`0ca3b5e9b7844c37f41315d759c6a999322d31b4`**（主提交）。｜push 区间：**`6871952..0ca3b5e`**，分支 `main`，SSH 443（共 12 文件）。
- 改动：`crates/nai-runtime/{Cargo.toml,src/composition.rs,tests/composition.rs}`、`tests/registry.yaml`(R10 notes)、
  `docs/adr/0101-*.md`、`docs/reports/2026-10-16-w123-*.md`、`docs/development-plan.md`、`docs/knowledge.md`、
  `openspec/specs/runtime-composition/spec.md`、`openspec/changes/archive/2026-10-16-w123-runtime-session-gate/`。
- openspec 三连：valid → `archive --yes`（`+2`）→ `--archived --strict` **121 / 0**。

## 八、CI 观测
- **主提交**（`0ca3b5e9b7844c37f41315d759c6a999322d31b4`）run id：**38853589407**。回读 `head_sha` **完整 40 位一致**；
  **双平台均 failure**，且**同一步**：step 9 `Check generated contracts without modifying consumers`
  （`cargo xtask schema --check`，exit 101）。
- **诊断（根因 = 本轮的暂存遗漏，非宿主瞬态）**：W123 给 `nai-runtime` 新增 `nai-upgrade-core` 依赖后，**本机门控据此改写了
  `Cargo.lock`**（+1 行依赖边），而主提交用「显式 `git add -- <path>`」逐文件暂存时**漏掉了 `Cargo.lock`** ⇒ CI 的
  `--locked` 要求锁文件已更新 ⇒ 失败。**判据**：两平台**同因**失败 ⇒ MUST NOT 当瞬态放过（否则真回归会被吃掉）；
  本机门控绿也**不能**替代（本机不带 `--locked` 的那条路径与 CI 的步骤集合不同）。
- **修复提交** `d9039f2da506a615efeb251022bcf4912a55b31d`（补 `Cargo.lock`，差异恰为那条 `nai-upgrade-core` 边）run id：
  **38854484587** ⇒ **双平台 `foundation` 均 `completed / success`**，`head_sha` 完整 40 位一致
  ⇒ **W123 的代码经 CI 双平台验证通过**。
- **教训**：显式暂存路径的盲区恰是「**工具链自己生成/改写的文件**」——本会话第 3 次踩暂存坑（W109/W110 漏
  `':!target-check'`，本次漏 `Cargo.lock`）。⇒ 改依赖后 MUST 显式 `git add Cargo.lock`，提交前逐项核对
  `git status --short` 的每个 `M`。
- **回填提交**（docs-only）自身 CI：按纪律观测 1 次，结论记入 memory，不在本报告占位。

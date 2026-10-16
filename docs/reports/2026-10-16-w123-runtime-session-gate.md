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
- 提交 SHA：TBD。｜push 区间：TBD，分支 `main`，SSH 443。
- 改动：`crates/nai-runtime/{Cargo.toml,src/composition.rs,tests/composition.rs}`、`tests/registry.yaml`(R10 notes)、
  `docs/adr/0101-*.md`、`docs/reports/2026-10-16-w123-*.md`、`docs/development-plan.md`、`docs/knowledge.md`、
  `openspec/specs/runtime-composition/spec.md`、`openspec/changes/archive/2026-10-16-w123-runtime-session-gate/`。
- openspec 三连：valid → `archive --yes`（`+2`）→ `--archived --strict` **121 / 0**。

## 八、CI 观测
- **主提交** run id：TBD；回读 `head_sha` 完整 40 位一致；双平台 `foundation` 均 success。
- **回填提交**（docs-only）自身 CI：按纪律观测 1 次，结论记入 memory，不在本报告占位。

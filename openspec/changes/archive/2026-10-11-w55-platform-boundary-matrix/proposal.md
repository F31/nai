# w55 平台边界与能力矩阵（macOS 前置）

## Why

用户拍板「macOS 后续应支持 Standard、Pro」，但核实后发现这一项与前三项**性质不同**：
基线 V3.7 全文对 `mac|darwin|apple` 的命中数为 **0**（只有 Windows 首发、Linux 轻云与 WSL、iOS/Android 实验），
registry 里也没有任何平台维度 ⇒ 这是一条**新增需求**，没有验收判据、没有目标三元组、没有签名与分发策略。

同时仓库里已经存在**没有被登记过的平台事实**：全仓平台分支仅 **11 处**，
`NativeVault` 在非 Windows 上**已经**返回 `CapabilityNotReady`（诚实的失败关闭），
`nai-storage` **已经**有 `#[cfg(unix)]` 的权限分支——但没有任何地方说明「哪个能力在哪个平台上是什么状态」。
于是「平台假设」只增不减，新增一处 `cfg(windows)` 不会引起任何人注意。

另有一处真实冲突：§14.1 把「抖音连接器与事件治理」列为 **Standard 的核心构件**，
而 §9.2/§687 说明抖音 SDK 与 Windows 音频是 **Windows 原生目标**；写实加速 §300 亦为「Windows NVIDIA CUDA 首发」。

故本轮按仓规「新需求先立验收」先做**可判定的一步**：把平台与能力状态登记成数据，并让源码与数据**互相约束**。

## What Changes

- 新增能力 `platform-matrix`：8 条需求 / 17 个场景，全部为可判定的形状。
- 新增 `schemas/platform-matrix/platforms.json` + `platforms.schema.json`：平台集合（windows 首发 /
  linux 轻云与 WSL / macos 后续 r2+r3 / ios + android 实验）与能力 × 平台状态矩阵。
- 新增 `scripts/platform/matrix.mjs` + `tests/platform/platform-matrix.test.mjs`：校验矩阵，并**反向**校验
  全仓平台条件编译文件是登记路径的子集；理由码**从 `connector.rs` 源码派生**，不另立常量表。
- 门控：`engineering_checks` 增加一行跑该测试（纯静态、不需 protoc，与 W52 的 `schema.test.mjs` 同理）。
- registry 新增 **A13**（矩阵与源码一致，`r1`/`implemented`）与 **A14**（macOS 覆盖 Standard 与 Pro，
  `r2`/`pending`，notes 记两处硬约束与缺失前置）。
- 新增 `docs/platform-matrix.md`；`AGENTS.md`、`docs/development-plan.md` 同步。

## Capabilities

### New Capabilities

- `platform-matrix`: 平台集合与目标三元组的显式声明；能力 × 平台状态矩阵的完备性；
  显式不可用必带封闭理由码；矩阵与源码平台分支互相约束；macOS 覆盖 Standard 与 Pro 的数据断言；
  Windows 绑定能力在 macOS 上如实不可得；数据根与签名按平台解析；登记不等于支持。

## Impact

- **零新增依赖**、**workspace 仍 19 包**、**内核未动一行**、**无 Rust 业务改动**
  （仅在 `xtask/src/lib.rs` 的 `engineering_checks` 增一行命令）。
- 新增一处门控步骤：全仓平台条件编译文件必须被矩阵登记在册。新增平台分支而不同步矩阵 **MUST** 失败。
- **不升级任何既有 `status`**；A13 记为 `implemented`（矩阵与校验已落地且可跑），
  A14 记为 `pending`（缺 macOS 实现、目标三元组、签名与公证链、Apple 开发者凭据）。
- **已知后果**：矩阵登记的是**本仓当前事实**，不是「macOS 能不能做」的裁决；
  矩阵不生成任何代码，故它不参与契约冻结流程，也不影响 frozen/draft 语义。

## Why

基线对供应链有三处明确要求，仓库里**一处都没有实现**：

- §11.6：「构建：Cargo.lock 与工具链锁定；CI 做依赖漏洞与许可审计；**每次发布附 SBOM**。」
- §29.5：「发布目录含 catalog、兼容矩阵、哈希、签名、**SBOM 与许可**、安装器……」
- §11.3 `T-08`（供应链：依赖、模型、升级包）：控制含「锁文件；依赖与许可审计；**SBOM**；哈希与签名；撤销检查；受控加载目录」。

`tests/registry.yaml` 的 S08 亦自记「SBOM 与真实签名链仍待建」。

问题不只是「缺一个文件」。「SBOM」在本仓库长期是一个**没有定义的空位**——没有规定它由什么生成、什么格式、
覆盖什么、不覆盖什么。「每次发布附 SBOM」因此在当前状态下**不可判真伪**：任何一份清单都能自称 SBOM。

本轮先消除这个空位：立规格定死「什么才算这份 SBOM」，再用**零新增依赖**的方式把它产出来，
并让产物**自带**其适用范围，免得它离开仓库后被当成漏洞审计或发布验收的证据。

## What Changes

- 新增 `cargo xtask sbom [--out <path>]`：从 `cargo metadata --locked`（`check-deps` 已经在用的同一份视图）
  与 `Cargo.lock` 的 SHA-256 生成 **CycloneDX 1.5 JSON**；不新增任何依赖、不访问网络。
- 新增实现 `xtask/src/sbom.rs`（含 11 项单测：许可分类、锁文件解析与两类具名拒绝、渲染确定性、范围自述）。
- 新增规格 `supply-chain-sbom`：规定覆盖范围、组件字段、**许可不得被静默改写**、确定性、
  以及产物必须自带范围与「不能用作什么」的声明。
- 新增 [ADR 0043](../../adr/0043-sbom-generation-and-scope.md)：裁决生成方式、格式与范围边界，
  并记录被否方案（`cargo-cyclonedx` / `cargo-sbom` / `syft` / 解析 `cargo tree` 文本）。

## Capabilities

### New Capabilities

- `supply-chain-sbom`：从锁定依赖闭包生成、可复现、且自带范围声明的软件物料清单。

### Modified Capabilities

（无。`engineering-foundation` 不动：本轮的生成命令**不进** portable 门控，理由见设计文档 §5。）

## Impact

- 修改：`xtask/src/sbom.rs`（新）、`xtask/src/lib.rs`（模块声明、命令分发、usage 串）、
  `openspec/specs/supply-chain-sbom/spec.md`（经归档吸收）、报告与变更文档。
- **零新增依赖**：`cargo_metadata` 与 `serde_json` 已是 `xtask` 的直接依赖；`Cargo.lock` 未变动。
- **不改任何验收 `status`**：S08 仍为 `implemented`——真实签名链仍缺，漏洞审计与许可**政策**（允许/拒绝清单）
  也不在本轮范围。本变更只消除「SBOM」这一个空位。
- **不改 portable 语义**：生成命令与 `check --secrets` 同类，是手动/发布期命令，不并入 `engineering_checks`；
  其单测仍在 `cargo test -p nai-xtask --lib` 内运行，故不是无人使用的幽灵工具。

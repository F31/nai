# 任务（w55 平台边界与能力矩阵）

## 0 立项

- [x] 0.1 建立 `openspec/changes/w55-platform-boundary-matrix/`（`.openspec.yaml` / `proposal.md` /
      `design.md` / `specs/platform-matrix/spec.md` / `tasks.md`）
- [x] 0.2 `openspec validate w55-platform-boundary-matrix --strict` 通过

## 1 矩阵数据与 Schema

- [x] 1.1 `schemas/platform-matrix/platforms.json`：平台集合（windows 首发 / linux 轻云与 WSL /
      macos 后续 r2+r3 / ios + android 实验），每个平台含 `target_triples` 与 `release_lines`
- [x] 1.2 同文件能力矩阵：系统密钥保护、数据根解析、路径包含、子进程监督、共享内存帧槽、
      2D 渲染演练后端、工具链目标假设、平台 SDK；每项能力的平台键集合**恰等于**平台集合
- [x] 1.3 `schemas/platform-matrix/platforms.schema.json`：三态封闭枚举、`explicit-unavailable` 必带
      `reason`、`supported` 不得带、`bound_to` 枚举、`additionalProperties: false`。
      **`reason` 刻意不写 enum** —— 合法值由校验从源码派生，避免第二处定义
- [x] 1.4 填入**今天的事实**（不臆造）：密钥保护 windows `supported` / 其余
      `explicit-unavailable{not_ported_to_target_os}`；数据根与路径包含与进程监督三平台均 `supported`
      （unix 家族含 macOS 已有落点）；帧槽 linux `supported` / 其余不可用（ADR 0017）；
      平台 SDK 三平台 `explicit-unavailable{no_official_interface}` 且 `bound_to: windows`

## 2 校验（`scripts/platform/matrix.mjs` + `tests/platform/platform-matrix.test.mjs`）

- [x] 2.1 平台 id 唯一且为小写标识；目标三元组形状 `<arch>-<vendor>-<os>[-<abi>]`；
      `release_lines` 取值限于仓库既有的五条
- [x] 2.2 每项能力的平台键集合与平台集合**逐一相等**（多一个、少一个都失败并给出差集）
- [x] 2.3 状态枚举合法；`explicit-unavailable` 必带 `reason`；`supported` 必不带
- [x] 2.4 `reason` **从 `crates/nai-plugin-sdk/src/connector.rs` 的 `UnavailableReason::label()`
      match 臂派生**（配平截取、不硬编码常量表）；派生前断言该 match 存在且解析非空，形状变化具名失败
- [x] 2.5 每个 `source_files` 路径**必须存在**（悬空引用失败）；`basis: source` 必须有证据、
      `basis: baseline` 不得有
- [x] 2.6 **核心门控**：全仓含平台条件编译（`cfg(windows)` / `cfg(unix)` / `cfg(target_os = ...)` /
      `cfg!` / Cargo 的 `[target.'cfg(...)'.x]`）的文件集合 ⊆ 登记路径并集
- [x] 2.7 `platforms[macos].release_lines` 同时含 `r2` 与 `r3`（用户拍板的数据断言）
- [x] 2.8 `bound_to: windows` 的能力在 macOS 上 MUST NOT 是 `supported`；平台 SDK 在 macOS 上
      **必须是** `explicit-unavailable{no_official_interface}`
- [x] 2.9 矩阵与文档 MUST NOT 出现 macOS 侧等价接入路径的描述（不臆造替换）—— `docs/platform-matrix.md`
      明文「不假定也不臆造 macOS 侧的等价接入路径」
- [x] 2.10 **非空洞守卫**：注入未声明平台键、缺平台、坏理由码、PascalCase 变体名、unavailable 无理由、
      `supported` 带理由、悬空 `source_files`、未登记的 cfg 新文件、macOS 丢 `r3`、Windows 绑定能力在
      macOS 标 `supported`、`source` 类无证据、`baseline` 类登记文件、重复平台 id、非法发布线、坏三元组，
      各检查**分别抓到**；**另有「扫描非空且命中已知分支」守卫**，防核心门控因扫描恒空而恒真

## 3 门控接入

- [x] 3.1 `xtask/src/lib.rs` 的 `engineering_checks` 增加一行跑
      `tests/platform/platform-matrix.test.mjs`（纯静态、不需 protoc，与 `schema.test.mjs` 同理）
- [x] 3.2 `scripts/contracts/validation.mjs` 注册 `platforms` 校验器；
      `tests/contracts/schema.test.mjs` 新增矩阵形态与三态守卫测试（11 → 12）
- [x] 3.3 零 Rust 业务改动（仅门控一行）；不改 kernel / contracts / 生成物；workspace 仍 19 包

## 4 registry 与文档

- [x] 4.1 `tests/registry.yaml` 新增 **A13「平台能力矩阵与源码一致」**（`release_line: r1`、
      `automation: unit`、`status: implemented`）
- [x] 4.2 新增 **A14「macOS 覆盖 Standard 与 Pro」**（`release_line: r2`、`automation: manual`、
      `status: pending`），notes 记两处硬约束与缺失前置
- [x] 4.3 新增 `docs/platform-matrix.md`
- [x] 4.4 `docs/platform.md` 加交叉引用（平台事件准入 ≠ 平台能力矩阵）
- [x] 4.5 `AGENTS.md`、`docs/development-plan.md` 同步；如实登记 `profiles/` 目录缺口
- [x] 4.6 `docs/reports/2026-10-11-w55-platform-boundary-matrix.md`

## 5 闭环

- [x] 5.1 预热后跑 `cargo xtask check --profile full`：**EXIT=0**、
      **86 目标（68 `Running` + 18 `Doc-tests`）、740 passed / 0 failed**、前端 6 步全过
      （本轮零 Rust 业务改动，数字与 W54 完全相同；`tests/platform` 22/22、`schema.test` 12/12、
      `test:ui` 52/52 均在日志中实测出现）
- [x] 5.2 `cargo xtask check-deps` = **19 workspace packages**（不变）
- [x] 5.3 全部 tasks 勾选后 `openspec archive -y w55-platform-boundary-matrix`
- [x] 5.4 `openspec validate --all --strict --archived` 记录条数
- [x] 5.5 一次 commit + push（提交信息用 Write 落 UTF-8 文件，避免 `\u` 字面转义）
- [x] 5.6 用**缓存破坏参数**匿名复核 CI：断言 `head_sha` 与推送一致、记录 conclusion 与失败步骤

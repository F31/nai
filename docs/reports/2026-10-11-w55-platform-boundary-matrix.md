# W55：平台边界与能力矩阵（macOS 前置）

- 日期：2026-10-11
- 变更：`openspec/changes/archive/2026-10-11-w55-platform-boundary-matrix/`
- 新能力：`platform-matrix`（8 条需求 / 17 个场景）
- 承接：用户五项决策的第 4 项「macOS 后续应支持 Standard、Pro」

## 一、缺口性质：这一项与前两项**不同**

前三轮（W52 开放契约册、W53 显式 Unavailable 连接器、W54 桌面壳边界）共有的形态是
「**基线有明文、仓库零落点**」。这一项不是：

| 项 | 基线依据 | 仓库落点 | 性质 |
| --- | --- | --- | --- |
| 第 3 项 平台接缝替身 | §17.2 / §9.4 **有明文** | 零 | 承诺了但没落点 |
| 第 1 项 桌面壳 Tauri | §1248 / §1260 **有明文** | 零 | 承诺了但没落点 |
| **第 4 项 macOS** | **零命中** | 零 | **全新需求** |

核实到的原始事实：

```
grep -c -iE "mac|darwin|apple" "docs/直播数字人 + 智能体系统技术方案 V3.7.md"   →  0
```

基线只有：Windows 桌面首发（§3、§44）、Linux 轻云与 WSL（§687，且明文「**不代替 Windows 安装与设备验收**」）、
iOS/Android 实验（§10.3、§665）。`tests/registry.yaml` 的 66 条验收按业务分系列，`release_line` 表达的是
**发布线**（Slice-0 / R1=Lite / R2=Standard / R3=Pro / 实验），**没有任何 OS 维度**。

⇒ macOS 缺**基线判据、验收标准、目标三元组、签名与分发策略**。按仓规「验收唯一来源为 `tests/registry.yaml`、
新需求先立验收」，本轮**不写 macOS 实现**（那会是凭想象造需求），只做**可判定的一步**。

## 二、侦察：三处已存在却从未被登记的事实

本轮最有价值的部分不是新增了什么，而是**发现了一批一直存在、却从未被写下来的事实**。

**① 全仓平台条件编译有 23 个文件、116 处。** 此前没有人统计过。窄口径只搜 `cfg(windows)` 会漏掉大半——
实际的主力是 `cfg(target_os = "linux")` 与 `cfg(unix)`。

| 落点 | 处数 | 性质 |
| --- | --- | --- |
| `crates/nai-media-core/src/frame_slots/{shared_ram,broker,consumer}.rs` 等 | 56 | 共享内存帧槽（最大面） |
| `apps/{engine,output-keeper,render-worker}` | 32 | 帧槽的装配与使用方 |
| `crates/nai-security/{src/lib.rs,Cargo.toml}` | 11 | 系统密钥保护 |
| `crates/nai-storage/src/paths.rs` | 6 | 数据根解析与权限 |
| `plugins/avatar-2d`、`crates/nai-plugin-hosts`、`crates/nai-runtime`、`xtask` | 11 | 渲染演练、路径包含、进程监督、工具链 |

**② 帧槽在 Windows 上不可用是「已裁决」，不是缺陷。** `shared_ram.rs` 的文件头与 **ADR 0017** 都写着：

> `//! Linux-only, explicitly selected tmpfs-file diagnostic backend (ADR 0017).`
>
> 「Windows/非Linux后端明确 CapabilityNotReady」「**不改变Windows正式共享内存目标**」

即：Linux 用 `/dev/shm` 私有 tmpfs 文件的**工程演练**后端（名为 `tmpfs-file-drill`，有复制，不称为 mmap/零拷贝）；
Windows 映射、受保护句柄 broker 与命名管道身份是**独立待做项**。**首发平台自己在这条能力上不可用**，
而这件事此前没有任何地方登记。

**③ macOS 并非一片空白——`cfg(unix)` 同时覆盖它。**

| 能力 | macOS 的现状 |
| --- | --- |
| 数据根解析与权限 | `paths.rs` 的 `#[cfg(unix)]` 分支设 0700，**已覆盖 macOS** |
| 路径包含与链接拒绝 | `boundary.rs` 的 unix 符号链接检测，**已覆盖 macOS** |
| 子进程监督与退出状态 | `supervisor.rs` 的 `ExitStatusExt::signal()`，**已覆盖 macOS** |
| 系统密钥保护 | `NativeVault` 在非 Windows 上**显式**返回 `CapabilityNotReady`（失败关闭） |

这些不是为 macOS 写的，但确实已是可复用落点。若不做这一步登记，它们会被当成空白，重复劳动。

**核心判断**：缺口是「**缺登记**」，不是「缺判定」——**这 116 处没有一处需要改代码**。它们已经是对的。

## 三、交付

| 位置 | 内容 |
| --- | --- |
| `schemas/platform-matrix/platforms.json` | 5 个平台（windows / linux / macos / ios / android）× 8 项能力的状态登记 |
| `schemas/platform-matrix/platforms.schema.json` | 形态约束（三态封闭、`explicit-unavailable` 必带 reason、`supported` 必不带） |
| `scripts/platform/matrix.mjs` | 校验逻辑（可 import 的纯函数） |
| `tests/platform/platform-matrix.test.mjs` | **22 项**（7 正向 + 15 非空洞守卫），经 `engineering_checks` 进入 `portable` 与 `full` |
| `tests/registry.yaml` | 新增 **A13**（`r1`/`unit`/`implemented`）与 **A14**（`r2`/`manual`/`pending`） |
| `docs/platform-matrix.md` | 平台指南 |
| `xtask/src/lib.rs` | `engineering_checks` 增一行（本轮唯一的 Rust 改动） |

### 三个设计要点

**① 理由码从源码派生，不另立常量表。** 矩阵的 `explicit-unavailable` 必带理由码，而合法集合由校验
从 `crates/nai-plugin-sdk/src/connector.rs` 的 `UnavailableReason::label()` 的 match 臂解析得到：

```
UnavailableReason::label()  →  {no_official_interface, platform_admission_pending,
                                not_ported_to_target_os, credential_absent, debug_build_only}
```

若把这份清单复制进校验脚本，改一侧忘另一侧时校验会变成**假的保证**。派生前还断言「该函数存在且解析非空」，
形状变化时**具名失败**，不静默通过。

这也是 W53 那次设计的意外回报：当时为「平台连接器」定义的枚举，在这里找到了第二个使用者，
且**一个变体都不用加**——`not_ported_to_target_os` 正是「密钥保护还没移植到该平台」的准确表述。

**② 核心门控是「文件子集」。** 全仓含平台条件编译的文件 MUST 是矩阵登记路径的**子集**。
正向（登记的路径必须存在）防悬空引用；反向（源码分支必须被登记）才是本步存在的理由——
**在此之前，新增一处 `cfg(target_os = ...)` 不会引起任何地方注意，平台假设只增不减。**

这与 W52 的「验收 `contract` 必须可解析」、A03/A04 的依赖方向检查是同一类断言：
**让没被写下来的事实变成构建失败。**

**③ `bound_to` 一般化判据。** 声明某原生平台依赖的能力，在其它平台上 MUST NOT 是 `supported`。
于是「抖音 SDK 是 Windows 原生目标」不必在校验里硬编码能力 id。

## 四、macOS：归属与两处硬约束

**归属**：用户拍板 macOS 覆盖 **R2（Standard）与 R3（Pro）**。该归属落为**数据断言**
（`platforms[macos].release_lines` 必须同时含 `r2` 与 `r3`），由校验钉住——**文档会漂移，数据会被校验**。

**两处硬约束（如实登记，不是缺陷）**：

1. §14.1 把「抖音连接器与事件治理」列为 **Standard 的核心构件**，而 §9.2 抖音与 §687「Windows 原生是 SDK、
   音频与写实的正式运行目标」说明抖音 SDK 与 Windows 音频是 **Windows 原生目标**。
   ⇒ 平台 SDK 在 macOS 上登记为 `explicit-unavailable(no_official_interface)`，
   **本仓库不假定也不臆造 macOS 侧的等价接入路径**（用户拍板「如实登记为不可得」）。
2. §300 的写实加速路径为「Windows NVIDIA CUDA 首发」，TensorRT 与 **DirectML（Windows 专属）** 单独认证。
   ⇒ macOS 侧需另定路径，**尚未裁决**。

⇒ **macOS 上的 Standard 与 Pro 内容 MUST 与 Windows 档位表（§14.1）分别定义，不得照搬。**

**A14 的缺失前置**：目标三元组进入构建与 CI；系统密钥库后端；帧槽的正式后端；macOS 侧加速路径的裁决与实现；
代码签名与**公证（notarization）**链；Apple 开发者凭据；安装器与分发；真实设备与媒体验收。
**A14 保持 `pending`**——登记归属不等于开始实现。

## 五、非空洞守卫

每一项检查都配一个注入样本，断言该检查**确实抓到**；否则「矩阵通过」与「检查恒真」无法区分。

| 注入样本 | 被哪条检查抓到 |
| --- | --- |
| 未声明的平台键 `plan9` | 平台键集合相等 |
| 少一个平台 | 同上（给出差集） |
| 理由码 `because_i_said_so` | 理由码 ∈ 源码派生集合 |
| 变体名 `NotPortedToTargetOs`（PascalCase） | 同上 |
| `explicit-unavailable` 无理由 | 必带理由 |
| `supported` 带理由码 | 必不带理由 |
| 悬空 `source_files` | 路径存在 |
| 未登记的平台分支文件 | **核心门控**（文件子集） |
| macOS 丢 `r3` | macOS 归属断言 |
| Windows 绑定能力在 macOS 标 `supported` | `bound_to` |
| `source` 类无证据 / `baseline` 类登记文件 | 依据一致性 |
| 重复平台 id / 非法发布线 / 坏三元组 | 平台声明 |

**另有一条「扫描本身非空」守卫**：若 `collectPlatformBranchFiles` 因 bug 恒返回空，
「文件子集」这条核心门控会**恒真**。故先断言扫描命中 ≥10 个文件且包含 4 个已知路径。这条守卫的价值与
W52 的「省略目录必须报错」同级。

## 六、验证

| 项 | 结果 |
| --- | --- |
| `cargo xtask check --profile full` | **EXIT=0** |
| 测试目标 | **86（68 `Running` + 18 `Doc-tests`）**，**740 passed / 0 failed** |
| 与 W54 对比 | **完全相同（86 / 740）**——本轮零 Rust 业务改动，数字不变是旁证 |
| 前端门控 | 6 步全过（两包 typecheck/build、桌面包单测、`test:ui`） |
| `tests/platform` | **22/22**（15 个 guard 用例） |
| `tests/contracts/schema.test.mjs` | **12/12**（原 11，新增 1：矩阵形态 + 三态守卫） |
| `tests/registry.test.mjs` | **2/2** |
| `tests/ui` | **52/52**（未改动，与 W54 持平） |
| `cargo xtask check-deps` | **19 包**（不变） |
| `openspec validate --all --strict` | **41 items**（40 specs + 1 活动变更） |
| 归档后 `validate --all --strict --archived` | **60/60**（41 specs + 19 已归档变更；W54 为 59） |
| `cargo fmt --all` | 只改本轮新文件（`git diff --name-only` 复核） |

**新增的门控步骤确实执行**（不能只看总绿）：日志中出现
`# Subtest: the committed matrix is internally consistent`、`ok 1`–`ok 22`、`1..22`、`# pass 22`。

## 七、未做与遗留

**未做**：

- **不写任何 macOS 实现**、不加目标三元组到构建、不加 profile、不改 Cargo target。
- **不创建**基线 §29.4 要求的 `profiles/{editions, deployments, resources}` 目录——**该目录至今不存在**，
  本轮如实登记这个缺口，但**不**创建它：它与本矩阵无重叠（本矩阵登记的是**操作系统平台**的能力，
  不是**档位**或**部署配置**），且 `editions` 的落点需要独立裁决。
- 矩阵**不生成任何代码**，故不参与契约冻结流程，不影响 frozen/draft 语义。
- **不升级任何既有 `status`**（A13 之外不新增 `implemented`）。

**遗留**：

- A14 的缺失前置清单（见 §四）是 macOS 立项时的输入。
- 矩阵登记的是**本仓当前事实**，不是「某平台能不能做」的裁决（ADR 0046 的 R1 同类后果）。
- 「文件子集」约束会随平台假设增加而变严——这是目的，不是副作用。

**如实披露的边界**：把一项能力在某平台标为 `explicit-unavailable` **不是缺陷**，是如实登记；
只有**不实声明**才是缺陷。本轮据此**未**升级 A03（`implemented`）、**未**升级 P01（`blocked`）。

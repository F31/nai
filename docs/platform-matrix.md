# 平台能力矩阵

本文登记**哪个能力在哪个平台上是什么状态**。数据在
[`schemas/platform-matrix/platforms.json`](../schemas/platform-matrix/platforms.json)（形态由
[`platforms.schema.json`](../schemas/platform-matrix/platforms.schema.json) 约束），校验在
`tests/platform/platform-matrix.test.mjs`，由 `cargo xtask check --profile portable|full` 执行。

> 它与 [平台事件准入](platform.md) 是不同的对象：那篇讲**平台事件**（抖音等平台的事件如何在可信宿主约束下
> 进入内核），本文讲**操作系统平台**的能力。两者都叫「平台」，但一个是业务概念，一个是运行环境。

**它登记的是本仓库当前事实，不是「某平台能不能做」的裁决。** 把一项能力在某平台标为
`explicit-unavailable` 是**如实披露**，不是缺陷；只有**不实声明**才是缺陷。

## 平台

| 平台 | 目标三元组 | 发布线 | 状态 | 依据 |
| --- | --- | --- | --- | --- |
| `windows` | `x86_64-pc-windows-msvc` | Slice-0 / R1 / R2 / R3 | `supported` | §44 首发；§687 Windows 原生是 SDK、音频与写实的正式运行目标 |
| `linux` | `x86_64-unknown-linux-gnu` | （无） | `partial` | §687 WSL 用于编辑、纯逻辑测试、离线模型制作与 Linux 轻云服务，**不代替 Windows 安装与设备验收**。工程门控在 ubuntu 上执行，但它不承载发布线 |
| `macos` | `aarch64-apple-darwin`、`x86_64-apple-darwin` | **R2 / R3** | `planned` | 新增需求，见下文 |
| `ios` | `aarch64-apple-ios` | experimental | `not_started` | §10.3 手机本地运行需原生壳，仅前台演练与录制；§665 X02 |
| `android` | `aarch64-linux-android` | experimental | `not_started` | §10.3；§665 X01/X02 中端 Android 前台 30 分钟、切后台降级或暂停 |

## 能力矩阵

| 能力 | windows | linux | macos |
| --- | --- | --- | --- |
| 进程间共享内存帧槽 | `explicit-unavailable`(not_ported_to_target_os) | `supported` | `explicit-unavailable`(not_ported_to_target_os) |
| 系统密钥保护（凭据封装） | `supported` | `explicit-unavailable`(not_ported_to_target_os) | `explicit-unavailable`(not_ported_to_target_os) |
| 数据根解析与访问控制 | `supported` | `supported` | `supported` |
| 路径包含与链接拒绝 | `supported` | `supported` | `supported` |
| 子进程监督与退出状态 | `supported` | `supported` | `supported` |
| 2D 渲染演练后端 | `explicit-unavailable`(not_ported_to_target_os) | `supported` | `explicit-unavailable`(not_ported_to_target_os) |
| 工具链的目标平台假设 | `supported` | `supported` | `explicit-unavailable`(not_ported_to_target_os) |
| 平台 SDK 接入（抖音） | `explicit-unavailable`(no_official_interface) | `explicit-unavailable`(no_official_interface) | `explicit-unavailable`(no_official_interface) |

`ios` 与 `android` 在全部能力上均为 `out-of-scope`（实验形态尚未立项）。

**三条容易误读的条目，如实说明**：

1. **共享内存帧槽在 windows 上不可用，不是缺陷**。[ADR 0017](adr/0017-frame-slot-ownership-and-linux-tmpfs-drill-boundary.md)
   已裁决：Linux 用 `/dev/shm` 私有 tmpfs 文件、0600、继承 FD 的**工程演练**后端（名为 `tmpfs-file-drill`，有复制，
   不称为 mmap/零拷贝），并明文「**Windows/非 Linux 后端明确 `CapabilityNotReady`**，不改变 Windows 正式共享内存目标」。
   Windows 映射、受保护句柄 broker 与命名管道身份是**独立待做项**。
   它的落点计数最多（`nai-media-core` 的帧槽模块与三个 app），正因如此更需要被登记。
2. **系统密钥保护在非 Windows 上是失败关闭**：`NativeVault` 的 `store`/`load`/`delete` 在非 Windows 目标上一律
   **显式**返回 `CapabilityNotReady`。它不是静默降级，也没有假装成功。
3. **macOS 已经在若干能力上"意外地"有实现**：`data-root` 的 0700 权限、`path-containment` 的链接拒绝、
   `process-supervision` 的退出状态都走 `#[cfg(unix)]` 分支——而 `unix` **同时覆盖 macOS**。
   这不是为 macOS 做的，但它确实已是可复用的落点，登记在此以免被当成空白。

## 状态语义

状态取自封闭集合，且**「不可用」必须带原因**：

| 状态 | 含义 |
| --- | --- |
| `supported` | 本仓库有实现落点，且该落点被登记的源码证据支撑。**不得**携带理由码。 |
| `explicit-unavailable` | 本仓库**明知**该能力在此平台不可用，并以显式失败关闭返回。**必带**理由码。 |
| `out-of-scope` | 该平台不在此项目当前范围。 |

理由码取自 `nai-plugin-sdk` 的 `UnavailableReason`：`no_official_interface`、`platform_admission_pending`、
`not_ported_to_target_os`、`credential_absent`、`debug_build_only`。**校验从 `connector.rs` 的
`UnavailableReason::label()` 源码派生该集合**，不在校验脚本里另立一份常量表——否则改一侧忘另一侧时，
校验会变成假的保证。Rust 侧新增一个变体不需要改脚本；用了不存在的码立即失败。

「这个平台没提」与「这个平台不支持」是两件不同的事：矩阵要求每项能力在**每个**平台上都有状态，
平台键集合与平台集合**逐一相等**，少一个都失败。

## 门控形状

矩阵与源码**互相**约束：

1. **正向**：矩阵登记的每个源码路径必须存在（悬空引用不通过）。
2. **反向（核心）**：全仓含平台条件编译（`#[cfg(windows)]`、`#[cfg(unix)]`、`#[cfg(target_os = ...)]`、
   `cfg!(...)`，以及清单里的 `[target.'cfg(...)'.x]` 段）的文件，MUST 是矩阵登记路径的**子集**。

```bash
node --test tests/platform/platform-matrix.test.mjs      # 或经 cargo xtask check --profile full
```

第二条是本步存在的理由：在此之前，新增一处 `cfg(target_os = ...)` **不会引起任何地方注意**，
平台假设只增不减。现在它会指向具体文件失败。

还有一条一般化判据：`bound_to` 声明某原生平台依赖的能力，在其它平台上 MUST NOT 是 `supported`。
这把「抖音 SDK 是 Windows 原生目标」一类事实变成可判定断言，而不必在校验里硬编码能力 id。

## macOS：覆盖 Standard 与 Pro

**这是一条新增需求**：基线 V3.7 全文对 `macos`/`darwin`/`apple` 的命中数为 **0**（只有 Windows 首发、
Linux 轻云与 WSL、iOS/Android 实验），registry 里也没有任何平台维度条目。用户拍板 macOS 覆盖
**R2（Standard）与 R3（Pro）**，该归属已落为**数据断言**（`platforms[macos].release_lines`），由校验钉住——
文档会漂移，数据不会。

**两处硬约束（如实登记）**：

1. §14.1 把「抖音连接器与事件治理」列为 **Standard 的核心构件**，而 §9.2 抖音与 §687「Windows 原生是
   SDK、音频与写实的正式运行目标」说明抖音 SDK 与 Windows 音频是 **Windows 原生目标**。故平台 SDK 在 macOS 上
   登记为 `explicit-unavailable(no_official_interface)`，**本仓库不假定也不臆造 macOS 侧的等价接入路径**。
2. §300 的写实加速路径为「Windows NVIDIA CUDA 首发」，TensorRT 与 **DirectML（Windows 专属）** 单独认证。
   macOS 侧需另定路径，**尚未裁决**。

⇒ macOS 上的 Standard 与 Pro 内容 MUST 与 Windows 档位表（§14.1）**分别定义**，不得照搬。

**缺失前置**：macOS 目标三元组进入构建与 CI；系统密钥保护的系统密钥库后端；共享内存帧槽的正式后端；
macOS 侧写实加速路径的裁决与实现；代码签名与**公证（notarization）**链；Apple 开发者凭据；安装器与分发；
真实设备与媒体验收。

详见验收 **A14**（`tests/registry.yaml`，`release_line: r2`，`status: pending`）。

## 未做边界

- 不写任何 macOS 实现、不加目标三元组到构建、不加 profile、不改 Cargo target。
- 不创建基线 §29.4 要求的 `profiles/{editions, deployments, resources}` 目录——那是另一件事，
  与本矩阵无重叠（本矩阵登记的是**操作系统平台**的能力，不是**档位**或**部署配置**）。
- 矩阵**不生成任何代码**，故它不参与契约冻结流程，也不影响 frozen/draft 语义。
- 登记一项能力在某平台不可得，**MUST NOT** 被读作某平台已构建、已打包或已可分发。

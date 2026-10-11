# 设计（w55 平台边界与能力矩阵）

## 1 为什么先做矩阵，而不是先写 macOS 代码

用户拍板「macOS 后续应支持 Standard、Pro」。直觉是开始写 macOS 分支。**否决**，理由是三项已核实的事实：

| 事实 | 出处 |
| --- | --- |
| 基线 V3.7 对 macOS 的命中数为 **0** | `grep -c -iE "mac\|darwin\|apple"` = 0 |
| registry 无任何平台维度条目 | 66 条验收全按业务分系列，`release_line` 只表达 Slice-0/R1/R2/R3/实验 |
| 「Standard」的核心构件之一是抖音连接器，而抖音 SDK 是 Windows 原生目标 | §14.1 档位表 + §9.2/§687 |

⇒ 没有基线判据、没有验收条目、没有目标三元组、也没有对「macOS 上的 Standard 是什么」的裁决。
直接写实现等于**凭想象造需求**，而仓规明文「验收唯一来源为 `tests/registry.yaml`，新需求先立验收」。

所以本轮交付的是**登记与约束**：把「哪个能力在哪个平台上是什么状态」变成可查的数据，
并让源码里的平台假设**不能再悄悄增加**。macOS 的实现与档位内容留给 A14，
那时它已经有了一份**需要被填满的形状**，而不是一张白纸。

## 2 三项已存在却从未被登记的仓库事实

侦察全仓，平台条件编译只有 **11 处**，分布在 5 个文件：

| 落点 | 现状 | 本轮如何登记 |
| --- | --- | --- |
| `nai-security/src/lib.rs`（6 处 `#[cfg(windows)]`） | `NativeVault` 在非 Windows 上**已经**返回 `CapabilityNotReady`——诚实的失败关闭 | 能力「系统密钥保护」：windows `supported`；其余 `explicit-unavailable{not_ported_to_target_os}` |
| `nai-security/Cargo.toml`（`[target.'cfg(windows)'.dependencies]`） | `keyring` 只在 Windows 目标下引入 | 同上 |
| `nai-storage/src/paths.rs`（2 处） | **已有** `#[cfg(unix)]` 设 0700 权限的分支；Windows 额外查 UNC 与重解析点 | 能力「数据根解析」：三平台均 `supported`（含各自的加固差异） |
| `xtask/src/lib.rs` / `schema.rs`（2 处） | `windows-local` profile 在非 Windows 报错；`.exe` 后缀 | 能力「工具链目标假设」，登记为工具性条目 |
| `xtask/tests/cli.rs`（1 处） | 非 Windows 上的条件测试 | 同上 |

要点：**这 11 处没有一处需要改代码**。它们已经是正确的；缺的只是「被写下来」。
这与 W53 的教训一致——缺口常常不是「缺判定」，而是「缺被判定的实物」。这里补的是登记。

## 3 状态三态与「不可用必须是显式的」

状态取自封闭集合：

- `supported`：**本仓库有实现落点**，且该落点被源码证据支撑。
- `explicit-unavailable`：本仓库**明知**该能力在此平台不可用，且以显式失败关闭返回（**不静默降级**）。
- `out-of-scope`：该平台不在此项目的当前范围（例如实验形态尚未立项）。

**关键纪律**：`explicit-unavailable` 必带理由码，`supported` 必不带。这条区分来自 W53 建立的
`UnavailableReason` 纪律——「不可用」是一个**要带上原因的状态**，不是一个可以省略的空白。
「没有提」与「不支持」是两件不同的事，S2 场景专门钉住这一点。

## 4 理由码从源码派生，不另立常量表

矩阵里的理由码是 snake_case 字符串。**不**在校验脚本里写一份变体清单的副本——那会造出第二处定义，
改一侧忘另一侧时校验反而成为假的保证。做法与 W54「从 `Cargo.toml` 读真实二进制包名」同一手法：

```
crates/nai-plugin-sdk/src/connector.rs  →  UnavailableReason::label() 的 match 臂  →  {no_official_interface, platform_admission_pending, not_ported_to_target_os, credential_absent, debug_build_only}
```

校验断言矩阵用到的每个理由码 ∈ 该集合。Rust 侧新增一个变体时，**不需要**改校验脚本；
新增一个**不存在的**理由码时，校验立刻失败。

这也是 W53 那次设计的意外回报：当时为「平台连接器」定义的枚举，在这里找到了第二个使用者，
并且**一个变体都不用加**——`not_ported_to_target_os` 正是「密钥保护还没移植到该平台」的准确表述。

## 5 「文件的子集」——核心门控的形状

矩阵登记每项能力的 `source_files`。校验有两个方向：

1. **正向**：登记的每个路径必须**存在**（悬空引用 MUST NOT 通过）。
2. **反向（核心）**：全仓含平台条件编译的文件集合 SHALL 是登记路径的**子集**。

反向约束是这一轮的真正价值：它把「平台假设」从一个**只增不减的隐式集合**，变成**必须显式登记**的集合。
明天有人新增一处 `#[cfg(target_os = "macos")]` 而不登记能力，门控立刻失败并指名文件。

这与 W52 的 `registry.mjs`（验收的 `contract` 必须可解析）和 A03/A04 的依赖方向检查是同一类断言：
**让「没被写下来的事实」变成构建失败**。

## 6 macOS 覆盖 r2 + r3，是一条可读的数据断言

`platforms[macos].release_lines` 必须同时含 `r2` 与 `r3`。这条**写进数据**而不是只写在文档里——
文档会漂移，数据会被校验。用户拍板的这条归属因此获得与其它门控同等的效力。

同时 `platforms[macos]` 的 `status` 明确为「尚未支持」，A14 保持 `pending`。

## 7 Windows 绑定的能力在 macOS 上如实不可得

**不做**「macOS 上也能接抖音」这类推断。理由：

- 抖音 SDK 与 Windows 音频是 §687 明文的 **Windows 原生目标**；
- 写实加速 §300 为「Windows NVIDIA CUDA 首发」，DirectML 亦为 Windows 专属；
- §17.2 的纪律「不放伪造 SDK」在这里的自然延伸是：**不臆造等价接入路径**。

因此「平台 SDK」能力在 macos 上登记为 `explicit-unavailable{no_official_interface}`，
且**不**在文档里写任何 macOS 侧候选路径。macOS 的 Standard 内容与 Windows 档位表**分别定义**，
这条差异如实登记在 A14 的 notes 里，供 macOS 立项时裁决。

## 8 门控位置

矩阵校验是**纯静态**的（读 JSON、扫源码、读 Rust 源码里的枚举标签），不需要 protoc，也不需要编译产物。
因此按 W52 的先例放进 `engineering_checks`，`portable` 与 `full` **都会**跑到：

```
deps::check → registry::check → schema::node(schema.test.mjs) → platform::node(platform-matrix.test.mjs) → fmt → clippy → tests
```

`platforms.schema.json` 的 Ajv 静态校验则并入既有的 `tests/contracts/schema.test.mjs`，不新开文件。

## 9 已知后果与风险

| # | 后果 | 处置 |
| --- | --- | --- |
| R1 | 矩阵登记的是**本仓当前事实**，不是「某平台能不能做」的裁决 | 如实登记；不声称任何平台「可以支持」 |
| R2 | 「文件子集」约束会随平台分支增加而变严，可能使无关改动失败 | 这正是目的：平台假设必须显式。失败信息点名文件 |
| R3 | 矩阵不生成任何代码，不参与契约冻结 | ADR 0046 与 catalog 的 stage 语义不受影响 |
| R4 | 基线 §29.4 要求的 `profiles/{editions,deployments,resources}` 目录**至今不存在** | 本轮**不**创建（属另一件事），如实登记在报告里 |
| R5 | A14 为 `pending`，易被读成「macOS 已排上日程」 | notas 区分「覆盖两档发布线的归属」与「缺失前置」 |
| R6 | 理由码派生依赖 `label()` 的 match 形状 | 派生前先断言该 match 存在且解析非空；形状变化时**具名失败**而非静默通过 |

## 10 不做

- 不写任何 macOS 实现、不加目标三元组到构建、不加 profile、不改 Cargo target。
- 不创建 `profiles/` 目录（R4）。
- 不改 kernel、不改 contracts、不改任何生成物。
- 不升级任何既有 `status`；A13 之外不新增 `implemented`。

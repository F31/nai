# ADR 0046：桌面壳形态与 Tauri 边界——壳做四件事，其余都不是壳的事

- 状态：接受
- 日期：2026-10-11
- 基线：V3.7 §1248（技术栈）、§1260（产物含 `desktop-shell`）、§1151（capabilities 与 CSP 最小权限）、
  §1259（sidecar 三元组与 UI 父子生命周期）、§900（数据根）、§1101（同机优先受保护 IPC）、
  §181（CI 依赖检查：kernel 不依赖 Tauri；UI 不访问数据库与凭据）、§84 INV-18、
  §10.1（桌面控制台四入口）、§10.2 路线 B / §10.3（移动端原生壳）
- 相关：[ADR 0041](0041-engineering-profile-frontend-boundary.md)（工程 profile 与前端门控边界）、
  [ADR 0014](0014-explicit-local-control-surface.md)（显式本机控制面）、
  [ADR 0019](0019-composition-root-boundary.md)（装配根边界）、
  [ADR 0035](0035-mobile-monitoring-surface-boundary.md)（手机监控面边界）、
  `apps/desktop/`、`xtask/src/deps.rs`、`tests/registry.yaml` A04/A12
- 变更：`openspec/changes/w54-desktop-shell-boundary/`

## 背景

基线把宿主壳写成了明文要求：技术栈是「**Tauri 2 + React + TypeScript + Vite**」（§1248），
产物清单里有「**desktop-shell**」（§1260），并分四处规定了壳的边界——capabilities 与 CSP 只开放必要命令与资源（§1151）、
sidecar 按目标三元组命名且 UI 父子生命周期显式处理、后台进程由 supervisor 或托盘管理而不依赖 WebView 保活（§1259）、
数据根由壳或操作系统路径 API 解析（§900）、同机优先受保护 IPC 而 HTTP 只用于测试适配（§1101）。

仓库的真实状态是**形状已定、边界与落点皆无**：

| 事实 | 出处 |
| --- | --- |
| `docs/adr/` 的 44 篇里**没有一篇**裁决桌面壳形态 | `ls docs/adr/`（0001–0045） |
| `apps/desktop` 是**纯 Web**：依赖只有 `react`/`react-dom`/`vite`，无 `src-tauri/` | `apps/desktop/package.json`、`find` |
| §1151/§1259/§900/§1101 四组条款在 `crates/`、`apps/`、`schemas/` **零命中** | 全仓检索 |
| `tests/registry.yaml` 的 **66 条里没有桌面壳条目**（A04 只管依赖方向） | `tests/registry.yaml` |
| INV-18「内核不依赖 Tauri」**已由门控强制**：`tauri` 前缀检查 + 单测夹具 | `xtask/src/deps.rs:108`、`:490` |

最后一行很关键：**本轮不重复实现它**。缺的不是「内核不依赖壳」这条禁令（它已就位），
而是「壳做什么、不做什么、授了什么、走哪条通道、后台归谁管」这一组**尚无落点的边界**。
在边界缺席的情况下，「壳做完了没」无法判真伪——任何一份配置文件都能自称壳。

## 决策

### D1 形态是 Tauri 2；壳是**呈现层与宿主接缝**，不是后台的监护者

技术栈照基线（§1248）。但要写明的是一件更基础的事：**壳是可随时关闭的呈现层**。
需要在 UI 关闭后继续运行的引擎与输出守护进程，其生命周期**归独立 supervisor 或托盘**，不归 WebView（§1259）。
这条决定了后面 D5/D6 的所有形状。

### D2 壳只做四件事

| 壳**做** | 依据 |
| --- | --- |
| 窗口与托盘及其生命周期 | §1259、§10.1 |
| 受保护 IPC 的**宿主侧** | §1101 |
| 数据根解析（交操作系统路径 API） | §900 |
| sidecar 的拉起与监督 | §1259 |

| 壳**不做** | 依据 |
| --- | --- |
| 会话、守门、审批、知识、媒体编排等业务逻辑 | §17.5、§181（UI 不访问数据库与凭据） |
| 任意 shell 执行、任意文件系统、SQL | §1151 |
| 后台进程的存活性 | §1259（归 supervisor / 托盘） |

「不做什么」这一半必须与「做什么」**同样可判**，否则边界会随时间被吸回去。故判定按**业务语义关键词**
（会话/守门/审批/知识/媒体编排）扫描壳的能力声明，命中即失败。

### D3 本轮不引入真实 Tauri 依赖——配置是**声明**，不是**构建**

形态既然定了，直觉是 `cargo add tauri`。**否决**：

- `tauri`/`tauri-build` 在 Windows 上需要 WebView2 的导入库，其构建链会访问网络获取平台组件。
  把它们放进 workspace ⇒ `cargo xtask check --profile portable|full` 的结论将取决于
  **本机是否装了 WebView2、网络是否可达**。那是**主机事实**，不是仓库事实。
- 这正是 ADR 0041 已裁过的禁忌：「让门控依赖『pnpm 在 PATH 且垫片与本机 ABI 匹配』，
  等于把**主机环境事实**混进门控结论」。同一条推理在这里指向同一个答案。
- 代价会外溢到口径：workspace 成员数、`check-deps` 包数、测试目标数与通过数全部需要重新解释，
  而收益（一座**还不可运行**的壳）为零。

因此本轮交付**真实格式的声明式配置**（Tauri 2 的 `tauri.conf.json` 与 `capabilities/*.json`）
与**自建的形状校验**，而不是一个构建产物。这不违反 §17.2 的「不放伪造 SDK」纪律——
那份纪律针对的是**冒充真实能力**；这里配置**如实就是配置**，且 ADR 明文：「配置落地 MUST NOT 被读作壳已实现」。

### D4 最小权限是**集合相等**，不是包含；禁止聚合权限集

§1151 的「**只**开放必要的命令与资源」是一个**上界**断言。而「包含」只能证明下界——
`[…必要权限…, "shell:allow-execute"]` 显然包含必要权限，却完全违反 §1151。所以判定取**集合相等**：
多授一项失败（超界），少授一项失败（声明与配置漂移）。

同时**禁止聚合权限集**（如 Tauri 2 的 `core:default`）：一条标识符展开成一组权限，
使「最小」无法逐条检查。故要求权限标识符形如 `core:<模块>:allow-<动作>`（判定 `^core:[a-z]+:allow-[a-z0-9-]+$`）。

**本轮声明集（5 条，钉住）**：`core:app:allow-version`、`core:event:allow-listen`、
`core:event:allow-unlisten`、`core:window:allow-start-dragging`、`core:window:allow-set-title`。
加权限**必须同时**改配置与规格声明，否则校验红——这是特性。

### D5 受保护 IPC 是生产路径，loopback HTTP 是测试适配

§1101 的原文如此规定。现有桌面面走 `ControlClient` 的 loopback HTTP（`apps/desktop/src/control.ts`，
已有测试钉住「只接受显式回环 URL 与内存凭据」）。**本轮不改这条通道**——它确实是测试适配。
本轮把它在**声明层面定性**：CSP 的 `connect-src` 只含受保护 IPC 的源（`ipc:` 与 `http://ipc.localhost`），
不含任何通配协议源；ADR 明确「HTTP 端口 MUST NOT 被表述为对外接口」，
使「HTTP 是正式通道」这一误读在声明层无法成立。

### D6 生命周期与 sidecar 名称都与「不依赖 WebView 保活」同源

§1259 一句话里有两个要求，本轮都落：

1. **UI 生命周期与后台解耦**：生命周期声明 MUST NOT 把「界面关闭」与「后台进程终止」绑定。
2. **sidecar 按目标三元组命名**：`bundle.externalBin` 声明**不带三元组**的逻辑名（如 `binaries/nai-engine`），
   实际文件为 `binaries/nai-engine-<target-triple>[.exe]`；判定要求每个声明项都能对应出带三元组的目标文件名。

两条同源：都来自 D1——**壳是可关闭的呈现层，不是后台的监护者**。

### D7 一次裁决三处：桌面、macOS、移动端路线 B 共用同一套边界

§10.2 路线 B（原生壳 + 局域网直连 + 证书固定）与 §10.3（Lite-Mobile 需原生壳）用的都是原生壳，
且 Tauri 2 本就有 macOS 与移动端目标。三处各写一套边界 ⇒ 同一套权限面被写三遍并必然漂移。故：

- **边界共用**（权限面、IPC 形态、生命周期纪律）；
- **差异只允许落在目标与分发上**（Windows 安装器 / macOS 签名与公证 / 移动端应用分发）；
- macOS 与移动端本轮**只登记边界与前置**，实现分别属第 4 项与第 5 项，MUST NOT 被读作已实现。

### D8 交付物与落点

- `docs/adr/0046-desktop-shell-form-and-tauri-boundary.md`（本文）
- `apps/desktop/src-tauri/tauri.conf.json`、`apps/desktop/src-tauri/capabilities/default.json`
- `tests/ui/desktop-shell.test.mjs`（经既有 `test:ui` 自动进入 `full` 门控，**不改门控代码**）
- `tests/registry.yaml` 新增 **A12「桌面壳宿主边界」**（`status: pending`）

## 否决方案

| 方案 | 否决理由 |
| --- | --- |
| 本轮 `cargo add tauri` 并建可构建壳体 | 把 WebView2 与网络拉进门控结论（D3）；ADR 0041 的同一条禁忌 |
| 把壳做成 workspace 成员 | 同上，并使 `check-deps` 口径与测试目标数无缘由漂移 |
| 用 `core:default` 一类权限集简化配置 | 聚合权限使「只开放必要」不可逐条检查（D4） |
| 只写 ADR 不落配置与校验 | 边界无法判定，「壳做完了没」仍不可判——回到本轮要解决的问题 |
| 为壳新建一套门控入口 | `scripts/frontend-gate.mjs` 按 `package.json` 的 `scripts` 驱动，新测试文件写进既有 `test:ui` 即可自动进门控（ADR 0041 D3），另立入口等于第二处定义 |
| 把 HTTP 端口表述为对外接口 | §1101：正式路径是受保护 IPC |
| macOS/移动端各写一套边界 | 同一套权限面被写三遍并漂移（D7） |
| 因配置落地而升级 A12 为 implemented | §1248/§1260 的真值是**可运行、可打包、可签名**的壳体；配置不是壳体 |

## 已知后果

| # | 后果 | 处置 |
| --- | --- | --- |
| R1 | 校验读的是配置**形状**，**不能**证明它被某个 Tauri 版本接受 | 如实登记；**不**声称「配置已验证可构建」 |
| R2 | Tauri 大版本改动配置字段时，本仓库检查**不会自动发现** | 引入真实依赖时应改为由**构建产出物**校验，而非形状 |
| R3 | `$schema` 指向官方 URL，**离线不可校验** | 校验**自建**，不依赖网络；schema 拉取不进任何门控 |
| R4 | 权限集被钉死为 5 条，加功能需同改两处 | 这是 §1151「只开放必要」的落地方式，不是缺陷 |
| R5 | A12 为 `pending`，易被读成「没做事」 | notes 区分**本轮落点**与**缺失前置**（真实依赖、可运行壳体、打包与签名链） |
| R6 | 前端页脚、`docs/control.md` 等处此前称「Tauri 尚未接线」 | 本轮就地订正为：**形态与边界已裁决，壳体尚未构建**——两者是不同的陈述 |

## 后果

- 桌面壳形态与边界**第一次有了可查的裁决**（此前 44 篇 ADR 中无一条）。
- `desktop-shell` 成为一条独立能力：8 条需求 / 14 个场景，全部为「可判定」的形状。
- macOS（第 4 项）与移动端路线 B（第 5 项）**已有共用边界可依**，各自实现时只需补目标与分发。
- **不变式**：workspace 仍 19 包、`check-deps` 仍 19、**零新增依赖**、**Rust 侧零改动**、内核未动一行。
- **未做**：壳体未构建（无 `tauri` crate、无 Rust 主体）、未打包、未签名；真实 WebView2 构建、
  安装器与 Windows 代码签名链属发布里程碑。

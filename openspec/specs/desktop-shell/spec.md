# desktop-shell Specification

## Purpose
把基线里关于**宿主壳**的四组条款落成**可判定**的边界与**可查**的落点：§1248（技术栈 Tauri 2 + React + TypeScript + Vite）、
§1260（产物含 `desktop-shell`）、§1151（capabilities 与 CSP 只开放必要命令与资源；前端没有任意 shell、文件系统、SQL 权限）、
§1259（sidecar 按目标三元组命名；UI 父子生命周期显式处理，引擎与 keeper 由独立 supervisor 或托盘管理，不依赖 WebView 保活，
不开放任意 shell）、§900（数据根由壳或操作系统路径 API 解析）、§1101（同机 Tauri 优先使用受保护的 IPC，HTTP 只用于已启用的
手机、轻云与测试适配）。

本能力判定**一份壳的形态声明与一份最小权限面**：壳只做什么、绝不做什么、授了哪些能力、走哪条通道、后台进程的生命周期归谁。
它**不构建壳、不引入 Tauri 依赖、不打包、不签名**——一处配置落地 MUST NOT 被读作「壳已实现」。
桌面、macOS 与移动端路线 B 的原生壳共用同一套边界，后两者本轮只写边界与前置，不实现。

## Requirements

### Requirement: The shell owns presentation, lifecycle and the host seam — never business logic

宿主壳 SHALL 只负责四件事：窗口与托盘及其生命周期、受保护 IPC 的**宿主侧**、数据根解析、sidecar 的拉起与监督。
会话、守门、审批、知识、媒体编排等**业务逻辑 MUST NOT 进壳**；内核 MUST NOT 依赖壳（INV-18「内核不依赖具体插件、平台 SDK、Tauri」）。
壳 MUST NOT 承担任何只有在设备与真实平台接入后才成立的工作。这条边界把「壳能做什么」从实现者习惯变成**可判定的声明**。

#### Scenario: The kernel reaches the shell

- **WHEN** 内核层依赖某个 Tauri 或宿主壳相关的依赖
- **THEN** 依赖方向门控失败并指名两侧

#### Scenario: The shell grows a business responsibility

- **WHEN** 一份壳的能力声明出现与业务语义（会话、守门、审批、知识、媒体编排）对应的条目
- **THEN** 校验以具名错误拒绝该条目

### Requirement: The granted capability surface is least-privilege and exactly declared

壳的 capabilities SHALL 只授予**逐条列举**的必要命令与资源，MUST NOT 出现任意 shell 执行、文件系统、SQL、
进程控制等能力（§1151）。MUST NOT 使用会把上述能力一并纳入的**聚合权限集**（例如一次性引入整组默认权限）。
声明的权限集合 SHALL 与规格约定的集合**逐一相等**：多授一项 MUST 失败，少授一项也 MUST 失败——
「最小」不是一个形容词，而是一次可比较的断言。

#### Scenario: An over-broad capability appears

- **WHEN** capabilities 里出现任意 shell、文件系统、SQL 或进程控制类权限，或引入一个聚合默认权限集
- **THEN** 校验失败并指名该权限

#### Scenario: The declared set drifts from the specification

- **WHEN** capabilities 的权限集合与约定的集合不一致（多一条或少一条）
- **THEN** 校验失败，且两侧差集被如实指出（加权限必须同时改声明，不得只改一侧）

### Requirement: Content security policy is explicit, and no unsafe evaluation is allowed

壳的 CSP SHALL 逐指令显式声明，MUST NOT 含 `unsafe-eval`（§1151「模型输出或知识片段不能当 HTML 插入执行」）。
连接源（`connect-src`）SHALL 只包含**受保护 IPC** 所需的源，MUST NOT 使用 `http:`、`https:` 等**通配源**，
MUST NOT 使用 `*`。

#### Scenario: The policy permits unsafe evaluation

- **WHEN** CSP 含 `unsafe-eval`
- **THEN** 校验失败

#### Scenario: A wildcard source is opened

- **WHEN** CSP 的任一指令使用 `*` 或通配协议源
- **THEN** 校验失败并指名该指令

### Requirement: Protected IPC is the production path; loopback HTTP is a test adaptation

同机 SHALL 优先使用**受保护 IPC** 作为壳与后台的正式通道（§1101）。loopback HTTP SHALL 只作为**测试适配**保留，
其存在 MUST NOT 被读作生产形态，其地址 MUST 限定为回环。壳的声明 MUST NOT 把 HTTP 端口表述为对外接口。

#### Scenario: HTTP is declared as the production interface

- **WHEN** 一份壳的声明把 loopback HTTP 端口表述为生产通道或对外接口
- **THEN** 校验失败——§1101 的正式路径是受保护 IPC

#### Scenario: The test adaptation stays loopback-only

- **WHEN** 测试适配使用的控制地址被检查
- **THEN** 它必须是回环地址，且其角色被表述为测试适配

### Requirement: UI lifetime is decoupled from background processes, and sidecars are named by target triple

关闭界面 MUST NOT 终止引擎与输出守护进程；需要继续运行的进程 SHALL 由独立 supervisor 或托盘管理，
MUST NOT 依赖 WebView 保活，MUST NOT 为此开放任意 shell（§1259）。随壳分发的 sidecar SHALL 按**目标三元组**命名，
使同一份声明可对应多个目标平台。

#### Scenario: Closing the UI ends the engine

- **WHEN** 壳的生命周期声明把界面关闭与后台进程终止绑定在一起
- **THEN** 校验失败——后台进程的生命周期归 supervisor 或托盘

#### Scenario: A sidecar file name carries no target triple

- **WHEN** 一个随壳分发的 sidecar 未按目标三元组命名
- **THEN** 校验失败并指名该 sidecar

### Requirement: Data root is resolved by the shell, never hardcoded

数据根 SHALL 由壳或操作系统路径 API 解析（§900），MUST NOT 在声明或源码中**硬编码**绝对路径
（例如 `C:\...`、`/home/...`、`/Users/...`）。安装目录只读、数据根可写这条区分 SHALL 在声明中保持。

#### Scenario: An absolute path is hardcoded

- **WHEN** 壳的声明或前端源码出现硬编码的绝对路径
- **THEN** 校验失败并指名该路径

### Requirement: One form decision covers desktop, macOS and the mobile native shell

桌面（Windows）、macOS 与移动端路线 B 的原生壳 SHALL 共用**同一套**边界（§10.2 路线 B、§10.3）。
macOS 与移动端壳 MUST NOT 另立一套权限面或另一套 IPC 形态；两者的差异 SHALL 只落在**目标与分发**上，
MUST NOT 落在边界上。后两者本轮只登记边界与前置，MUST NOT 被读作已实现。

#### Scenario: A second form factor invents its own boundary

- **WHEN** 一份 macOS 或移动端壳的声明引入与桌面壳不同的权限面或 IPC 形态
- **THEN** 校验失败——差异只允许落在目标与分发上

#### Scenario: An unbuilt form factor is read as delivered

- **WHEN** 一份壳声明被落地但对应形态尚未构建
- **THEN** 它 MUST NOT 使任何验收状态升级

### Requirement: A landing point is not an implementation claim

本能力交付**形态声明**、**最小权限面**与**校验**，MUST NOT 被读作壳已实现、可打包或可分发。
在引入真实依赖、产出可运行壳体与签名链之前，相关验收 MUST 保持未达标状态，并如实点名缺失前置。

#### Scenario: Configuration exists but no shell is built

- **WHEN** 声明与校验已落地，而壳体、打包与签名链尚未构建
- **THEN** 相关验收保持未达标并点名缺失前置（真实依赖、可运行壳体、打包与签名链）

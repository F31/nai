# desktop-shell Specification

## Purpose
把基线里关于**宿主壳**的四组条款落成**可判定**的边界与**可查**的落点：§1248（技术栈 Tauri 2 + React + TypeScript + Vite）、
§1260（产物含 `desktop-shell`）、§1151（capabilities 与 CSP 只开放必要命令与资源；前端没有任意 shell、文件系统、SQL 权限）、
§1259（sidecar 按目标三元组命名；UI 父子生命周期显式处理，引擎与 keeper 由独立 supervisor 或托盘管理，不依赖 WebView 保活，
不开放任意 shell）、§900（数据根由壳或操作系统路径 API 解析）、§1101（同机 Tauri 优先使用受保护的 IPC，HTTP 只用于已启用的
手机、轻云与测试适配）。

本能力覆盖**壳的形态声明、最小权限面与独立原生构建**：壳只做什么、授了哪些能力、走哪条通道、后台进程的生命周期归谁。
声明校验与真实构建是不同证据；独立壳的加载不能代替受保护引擎 IPC、打包、签名或业务验收。
桌面、macOS 与移动端路线 B 的原生壳共用同一套边界，各目标的构建和验收证据必须分别记录。

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

### Requirement: The native host embeds the console independently of the portable workspace

原生桌面壳 SHALL 从锁定依赖构建，内嵌现有前端生产产物，不依赖开发服务器。原生依赖 SHALL 与便携 workspace 隔离。缺前端产物或所声明的 sidecar 二进制 MUST 使构建显式失败，不得用占位文件替代。窗口加载的证据 MUST 与 Windows 安装、签名及业务交互验收分开。

#### Scenario: Native build consumes real artifacts

- **WHEN** 前端生产产物与当前目标的真实 sidecar 二进制齐备并执行原生构建
- **THEN** 生成原生壳可执行程序，WebView 可加载内嵌控制台

#### Scenario: A build prerequisite is absent

- **WHEN** 缺少前端产物或声明的 sidecar 二进制
- **THEN** 构建失败并指名缺失前置，不生成替身

### Requirement: The unconnected native console is explicit and cannot use the HTTP test adaptation

引擎受保护 IPC 尚未接线时，原生控制台 MUST 呈现具名不可用状态，MUST NOT 开放 HTTP 测试连接入口或伪造业务成功。浏览器控制台 SHALL 保留既有显式 loopback 测试适配。原生壳 MUST NOT 为绕过此边界放宽 CSP 或增加任意文件、进程及 shell 权限。

#### Scenario: Operator opens the native console before engine IPC is wired

- **WHEN** 操作者打开原生壳
- **THEN** 界面明确提示受保护引擎 IPC 未接线，HTTP 连接按钮不可用

#### Scenario: Operator opens the browser adaptation

- **WHEN** 操作者使用既有浏览器控制台
- **THEN** 原有显式 loopback 连接流程继续可用

### Requirement: The native host supervises an engine through private inherited control handles

原生壳 SHALL 只从固定侧车路径启动 engine，参数来自操作者显式声明。引导凭据 MUST 由系统随机源产生且仅通过私有继承管道传递，不进入 WebView、文件、环境或日志。管道 MUST 使用生成的 Protobuf、有界帧/队列、序号与 deadline；超时或协议错误 MUST 终止该连接且不得自动重试。engine 原生模式 MUST 不监听 HTTP。

#### Scenario: Native operator connects to the real engine

- **WHEN** 操作者提供合法的已配置工作区与场景并启动原生壳
- **THEN** 壳内状态/问题/审批通过私有管道到同一个 engine 业务处理器，WebView 不接触引导凭据

#### Scenario: A partial frame or unresponsive peer interrupts control

- **WHEN** 管道断开、帧超限或对端在期限内不完成响应
- **THEN** 通道失败关闭、不复发请求，engine 在失联期限内静音并撤权，壳不自动重启它

### Requirement: Window visibility is independent of engine lifetime

关窗 SHALL 隐藏控制台而非停止后台引擎，托盘 SHALL 提供恢复窗口与显式退出。显式退出 MUST 有界停止并回收本壳启动的 engine。引擎监督和心跳 MUST 在 Rust 宿主侧，不依赖 WebView 定时器。失联或重启 MUST NOT 恢复旧自动权限。

#### Scenario: Operator closes and restores the window

- **WHEN** 操作者关闭控制台窗口再从托盘恢复
- **THEN** 后台引擎仍由同一个原生宿主管理，没有重复进程或自动权限恢复

#### Scenario: Operator explicitly quits

- **WHEN** 操作者选择托盘退出
- **THEN** 壳有界停止自己拥有的 engine 并回收，之后退出

### Requirement: First-run setup is explicit and persists no secrets

原生壳 SHALL 从应用数据目录解析并只读展示数据根，用户 MUST NOT 通过 WebView 指定数据根。非机密设置 SHALL 原子写入 0600 文件；Provider 凭据 MUST NOT 写入该文件、日志、命令行参数或 WebView 持久状态。凭据 MUST 仅作为环境变量注入本壳派生的 engine，变量名取自场景的 `provider.credential_env`；场景未声明而输入凭据 MUST 具名拒绝。应用启动 MUST NOT 自动派生 engine。

#### Scenario: First run completes from the window

- **WHEN** 操作者首次打开原生壳，填写工作区、身份、profile、场景与 runtime profile 并选择初始化
- **THEN** 壳保存非机密设置、按期限运行 provision、派生 engine 并允许连接；配置中不含 API 密钥

#### Scenario: Credential cannot be silently discarded

- **WHEN** 场景没有 `provider.credential_env` 而操作者输入了 API 密钥
- **THEN** 启动被具名拒绝，界面说明原因，且密钥没有被写入任何文件或参数

### Requirement: Setup commands share the host boundary and fail closed

设置命令 MUST 校验标识符、绝对路径、文件存在与运行状态；未配置、初始化失败、重复启动与二进制缺失 MUST 返回具名错误。设置权限 MUST 只覆盖这些设置命令，不得授予任意路径执行或进程控制。

#### Scenario: Duplicate start is refused

- **WHEN** engine 已由本壳启动而再次请求启动
- **THEN** 返回 `HOST_ALREADY_RUNNING`，不产生第二个 engine 进程

### Requirement: Preflight can be run from the window without a command line

宿主 SHALL 使用已保存的非机密设置运行 `nai-engine --preflight`，并在显式请求时启用受限连通探针。结果 MUST 来自引擎报告（`all_passed` 与逐轴状态），宿主 MUST NOT 重新判定。进程失败、超时或报告缺失 MUST 返回具名故障，MUST NOT 与“预检拒绝”混为一谈。预检 MUST NOT 阻塞连接，也 MUST NOT 把 `not_ready` 呈现为通过。

#### Scenario: Operator runs preflight before connecting

- **WHEN** 操作者在设置面板运行预检
- **THEN** 界面逐轴展示 passed/failed/not_ready；报告写入数据根，连接后既有面板读取同一份

#### Scenario: Preflight process fails

- **WHEN** engine 无法启动、超时或未写出报告
- **THEN** 返回 `HOST_PREFLIGHT_*` 具名故障，界面不展示为预检结论

### Requirement: Window preflight only presents a complete report from its current run

宿主 SHALL 在开始预检前使旧报告失效，重叠的宿主预检 MUST 具名拒绝。无法启动、超时、报告缺失、超限或结构无效 MUST 返回 HOST_PREFLIGHT_* 故障且清理本次残留报告。宿主 MUST 有界读取并验证必要字段及封闭状态，不得填默认值掩盖缺失字段或把进程失败解释为成功；业务结论仍取自 engine，错误 MUST NOT 回显子进程原始输出。

#### Scenario: An earlier passing report exists but the new process writes nothing

- **WHEN** 旧报告存在而本次进程失败或未产生报告
- **THEN** 宿主返回具名故障，旧报告不会被展示为本次结果，连接后的读取也不能取得旧报告

#### Scenario: A new valid refusal report is produced

- **WHEN** 本次 engine 产生完整 failed/not_ready 报告并以拒绝退出
- **THEN** 宿主原样展示 all_passed=false 与逐轴原因，不把业务拒绝作为宿主故障

#### Scenario: A report is malformed or too large

- **WHEN** 报告超出 256KiB、缺必要字段、逐轴状态未知或退出结果与 all_passed 不符
- **THEN** 宿主返回 HOST_PREFLIGHT_REPORT 并清理该报告，不产生默认字段的结果

#### Scenario: A second host request arrives during preflight

- **WHEN** 本宿主已有预检尚未结束
- **THEN** 第二次请求立即返回 HOST_PREFLIGHT_BUSY，不启动第二个进程也不删除进行中报告

### Requirement: Installed Windows shells resolve packaged demo resources without the source tree

Windows 安装构建 SHALL 包含真实同目标 engine/keeper、合法演示包与原创图标。宿主 SHALL 从操作系统资源目录解析默认场景和 runtime profile，MUST NOT 依赖开发机源码路径。已保存设置 MUST 优先，安装资源 MUST 保持只读，数据仍写入独立应用数据根。缺失资源或侧车 MUST 具名失败。

#### Scenario: First launch uses the installed demo

- **WHEN** 没有已保存设置且壳在带完整演示资源的安装目录启动
- **THEN** 设置面板默认指向随包场景与 runtime profile，预检使用这份资源，不启动假 Provider 或设备

#### Scenario: Existing settings survive a new host launch

- **WHEN** 已保存了自选场景和 runtime profile
- **THEN** 宿主继续使用已保存配置，不以安装默认值覆盖

### Requirement: Windows installer builds distinguish engineering packages from signed releases

Windows 构建 SHALL 使用锁定 CLI 和 cargo lock，先准备前端与真实侧车，再生成当前用户 NSIS 安装器。构建 MUST 验证实际产物并输出哈希与签名状态；未签名包 MUST 需要显式工程选项且不得表述为发布通过。安装和资源读取证据 MUST 与升级、签名、设备及真实 API 认证分开。
首次安装候选 MUST 拒绝覆盖已有安装，活动进程或进程检查不可用时 MUST 拒绝安装，不得强杀现有会话。

#### Scenario: An unsigned engineering package is requested

- **WHEN** 构建者显式选择未签名工程构建且所需工具与产物齐备
- **THEN** 生成标注未签名的安装候选及摘要，A12/O01 不因此升级

#### Scenario: A required tool or sidecar is missing

- **WHEN** 构建工具版本不符或缺实际同目标侧车
- **THEN** 构建具名失败，不复制占位文件或输出成功摘要

#### Scenario: Candidate cannot bypass the upgrade path

- **WHEN** 已有安装，或 engine/keeper/壳仍活动，或无法核验进程状态
- **THEN** 安装候选具名拒绝，不进入旧安装卸载/覆盖或强杀路径

### Requirement: Rendered application startup acceptance
桌面启动冒烟 SHALL 仅在真实应用根挂载且关键界面可见后成功，导航完成不得替代React渲染成功。生产构建 SHALL 使用同一React实例提供组件与渲染器。
#### Scenario: JavaScript startup crashes
- **WHEN** 导航完成但React启动异常导致空root
- **THEN** 冒烟不得报成功，并在期限内具名失败
#### Scenario: Production application mounts
- **WHEN** 真实生产bundle成功渲染工作台并挂载
- **THEN** 冒烟才标记界面就绪，浏览器验证标题与控制入口实际存在

### Requirement: Windowless desktop host and engine startup
Windows桌面壳 SHALL 在Debug和Release都使用GUI子系统，宿主启动后台引擎 SHALL 不创建控制台窗口，且保留stdio控制及错误结果。
#### Scenario: Open engineering build
- **WHEN** 操作者启动Debug工程桌面壳
- **THEN** 显示应用界面，不附带控制台
#### Scenario: Execute background engine operations
- **WHEN** 启动引擎、初始化、预检或测试Provider
- **THEN** 子进程不创建控制台，私有控制/有界诊断保持可用

### Requirement: Observable native engine startup
桌面 SHALL 对启动操作即时展示阶段，并保留受控宿主错误码与可执行恢复建议；不得把原生字符串拒绝吞成无细节的失败，不得以加载页面代替实际连接成功。
#### Scenario: Native startup rejection
- **WHEN** 启动返回具名字符串错误
- **THEN** 工作台显示失败与对应恢复入口，解除忙碌且不自动重试、不调用付费API
#### Scenario: Real engine connection
- **WHEN** 引擎启动成功且控制状态查询成功
- **THEN** 才显示已连接并开放基于真实会话的操作

#### Scenario: Pending request crosses the native decoder
- **WHEN** WebView提交合法seq=0待发送请求
- **THEN** 宿主验证完整形状后交给唯一actor分配线上序号；前端指定线上序号或包含认证令牌时拒绝

### Requirement: Product knowledge is editable without changing provider credentials
桌面 SHALL 提供商品事实与讲品段落工作台，允许在引擎运行时保存草稿和显式热发布；文档外发许可 SHALL 由操作者逐条选择，默认仅本地。读写 SHALL 有大小/数量界限和并发版本检查；运行中发布 SHALL 经prepare/activate受控撤权切换，禁止直接覆盖运行绑定。
#### Scenario: Save a product draft
- **WHEN** 操作者在停止或运行状态保存有效商品草稿
- **THEN** 草稿在数据库重启后可读，已发布知识与供应商密文保持不变，不发起付费请求
#### Scenario: Publish product facts
- **WHEN** 操作者显式发布有效事实
- **THEN** 可信引擎创建并发布新的知识快照；停止时切换下次会话，运行时经撤权及持久化确认后切换当前会话，不改旧事实版本
#### Scenario: Concurrent or running edits
- **WHEN** 场景已被另一写入者修改，或操作者在引擎运行中修改商品
- **THEN** 并发冲突拒绝覆盖新配置；运行中草稿可保存，发布必须经过受控两阶段切换，准备后的失败保持暂停，不自动恢复旧授权

### Requirement: Live knowledge publication preserves engine and revokes old output
宿主 SHALL 允许运行中保存商品草稿并显式热发布。热发布 SHALL 在引擎内执行prepare/activate两阶段：先静音并持久化撤权，再准备快照；宿主持久保存后激活。失败 SHALL 保持暂停并显示原因，不自动恢复旧授权或回滚旧事实。
#### Scenario: Publish while engine runs
- **WHEN** 操作者显式发布有效商品知识
- **THEN** 同一引擎进程加载新快照，旧候选/审批与迟到任务不再外发，后续人工请求使用新知识
#### Scenario: Persistence or activation fails
- **WHEN** prepare之后宿主保存失败或激活失败
- **THEN** 不恢复旧播报；界面提示暂停和显式恢复办法，持久化的新配置不得静默回滚
#### Scenario: Forged staging reference
- **WHEN** 请求包含数据根外路径、符号链接或非受控暂存标识
- **THEN** 拒绝读取与发布，不授予新输出权限

### Requirement: Setup files are selected through a narrow native dialog
桌面 SHALL 为场景文件和运行配置文件提供中文路径字段与原生JSON文件选择按钮。宿主 MUST 只接受两种已声明选择目标，不提供任意读取/执行能力；取消 SHALL 保留原表单，失败 SHALL 释放忙碌并显示原因。选择文件 SHALL 不自动保存或启动。
#### Scenario: Select or cancel
- **WHEN** 操作者选择JSON文件或取消对话框
- **THEN** 仅选择成功的对应路径更新，取消不修改任何字段且不触发引擎/API。
#### Scenario: Invalid or concurrent request
- **WHEN** 目标不受支持或已有选择器打开
- **THEN** 宿主拒绝该请求，不打开额外对话框。

### Requirement: Provider models have presets and lossless custom editing
每个LLM/TTS主备配置 SHALL 提供模型下拉选择，预设自动填充现有协议/地址/模型字段；只有自定义模式 SHALL 显示地址和模型输入。未知已存配置 MUST 原样回显为自定义；切换不得自动保存、测试、携带密钥或变更其他行及主备顺序。旧共享临时Key SHALL 默认折叠并说明用途和留空条件。
#### Scenario: Choose a preset
- **WHEN** 操作者选择预设
- **THEN** 当前行得到对应公开参数，保持身份/路由/预算，地址或协议变更清除该行未保存Key，不发网络请求。
#### Scenario: Existing custom endpoint
- **WHEN** 读取未精确匹配预设的已存配置或切换自定义
- **THEN** 原地址和模型保持可编辑，不被默认预设覆盖。

### Requirement: Normal connection does not require technical file configuration
普通连接 SHALL 自动使用已有工作区与管理场景，缺省使用宿主随包配置。身份/路径/旧共享Key SHALL 默认折叠，不作为普通用户填写前置。首次显式连接且引擎库不存在时 SHALL 自动初始化；已有库或未知访问状态 MUST NOT 被当作空库覆盖。
#### Scenario: First connection without engine database
- **WHEN** 操作者完成服务配置并显式连接且引擎库不存在
- **THEN** 使用随包运行配置初始化后连接，不要求勾选技术初始化或选择文件，不调用模型。
#### Scenario: Existing database
- **WHEN** 库已存在或访问状态未知
- **THEN** 不自动重新初始化；保留已有配置，异常具名失败。

### Requirement: Models use automatically loaded deployment lists and focused editing
LLM/TTS SHALL 自动读取数据库并以主备列表/选中项编辑呈现，MUST NOT 出现常规读取数据库或跨能力批量覆盖按钮。模型/音色/Key优先，高级参数折叠；主备调整 SHALL 保留整项绑定。未知地址/模型/音色 MUST 原值保留；加载失败或旧请求迟到不得覆盖当前编辑。Key变更 SHALL 标记未保存，换地址/协议 SHALL 清除且仅清除本项未保存Key，旧绑定不得显示可用。
#### Scenario: Edit an existing deployment
- **WHEN** 进入页面或编辑/设主已有模型
- **THEN** 自动回填且无需重新输入已保存Key；其他模型/另一能力不被批量重置，不自动调用API。
#### Scenario: Unknown voice or loading failure
- **WHEN** 旧音色不在预设中或配置读取失败
- **THEN** 未知音色以自定义原样编辑；读取失败仅提供恢复入口，不允许空配置覆盖保存。

### Requirement: Model deletion is directly discoverable in the deployment list
模型列表每行 SHALL 提供删除按钮，最后一项 SHALL 禁用并说明至少保留一个模型。删除 SHALL 仅修改草稿及该项待保存Key，显式保存前不修改数据库；其他模型及其Key不得被清除。
#### Scenario: Delete a selected model
- **WHEN** 停止引擎后删除存在其他同类模型的选中记录
- **THEN** 列表移除此项，展示剩余首项并提示保存；最后一项不可删除。

## ADDED Requirements

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

## ADDED Requirements

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

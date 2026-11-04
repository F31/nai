## ADDED Requirements

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

# plugin-hosts-and-ipc Specification

## Purpose
为 nai 提供有界、可验证的插件传输与宿主边界：先限长再分配的二进制帧、先于能力授权的握手/ACL、可观测的宿主生命周期，以及仅从已验签根目录加载且拒绝路径逃逸的加载规则。

## Requirements

### Requirement: Bounded binary framing
IPC 帧 SHALL 使用 4 字节小端长度前缀 + Protobuf body，长度超过上限时在分配前拒绝；读取必须正确处理 partial read、多帧合并、EOF、超长帧、恶意长度与调用期限/取消。

#### Scenario: Oversized or malicious length
- **WHEN** 长度前缀超过上限、长度前缀不完整或在帧中间遇到 EOF
- **THEN** 在分配大缓冲前返回明确错误且不解析残留字节

#### Scenario: Coalesced and partial frames
- **WHEN** 一个写入包含多个完整帧，或对端逐字节发送一个帧
- **THEN** 读取方分别还原每个帧，且期限/取消检查随读取推进

### Requirement: Handshake and ACL before capability dispatch
宿主 SHALL 要求首个帧为握手，核对插件 ID/版本、协议范围、nonce 回显与身份材料，并协商双方版本交集；对每个调用 SHALL 在分派处理前按 peer/session/capability/operation 检查 ACL，拒绝时返回 PERMISSION_DENIED 且不调用插件实现。

#### Scenario: Invalid identity or denied peer
- **WHEN** 身份材料未通过宿主校验、nonce 未回显、版本无交集，或 peer 未获准当前会话/能力/操作
- **THEN** 拒绝调用/断开连接，不进入能力实现

#### Scenario: Successful handshake and grant
- **WHEN** 身份、nonce 与版本匹配且 ACL 已授予
- **THEN** 完成握手并仅按获准能力/操作分派，结果以 data/complete/error 明确结束

### Requirement: Observable host lifecycle
宿主 SHALL 支持 describe、validate_config、start、health、suspend、resume、stop；未 start 不得返回健康通过，suspend 期间调用被拒绝，停止后再启动必须重新验证。健康是有来源的观测，不是授权或设备/平台认证。

#### Scenario: Suspend and restart
- **WHEN** 宿主挂起或停止，随后恢复或重新启动
- **THEN** 挂起期间调用返回明确不可用；重新启动后不自动继承旧授权

### Requirement: Verified load and isolation boundary
宿主 SHALL 只从规范化后位于允许根目录的已验签路径加载文件/DLL，拒绝绝对逃逸、相对穿越和符号链接/重解析点逃逸；未获认证 OS 沙箱（受限身份/AppContainer/Job Object）时，不可信原生子进程 SHALL 返回显式 Unavailable/Blocked，不以同用户进程冒充沙箱。子进程环境 SHALL 清理密钥类变量，只保留受控引用。

#### Scenario: Path traversal or symlink escape
- **WHEN** 路径使用 `..`、绝对路径或指向根目录外目标的符号链接
- **THEN** 加载被拒绝，不产生根目录外文件访问

#### Scenario: Uncertified native subprocess
- **WHEN** 没有经认证的 OS 沙箱证据时请求原生子进程
- **THEN** 返回显式 Unavailable/Blocked，且不剥离密钥类环境变量之外的可信行为

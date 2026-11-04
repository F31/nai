## ADDED Requirements

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

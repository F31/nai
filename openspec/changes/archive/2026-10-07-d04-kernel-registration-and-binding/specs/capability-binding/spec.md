# Capability Binding

## Purpose

让 nai 的统一内核依据明确场景要求、版本与准入证据生成可追溯的能力视图和绑定快照，避免可选能力缺失拖垮所有模式、未知权限被误授予，或活动会话被静默换成不同版本与数据域的插件。

## ADDED Requirements

### Requirement: Independent bounded registration
登记 SHALL 校验正式能力、版本/宿主及独立 owner 准入证据，插件自报签名/可信不构成授权；未知 inproc 实现拒绝。注册/依赖数量有界且调用可取消/到期，循环依赖必须拒绝并不污染既有登记。

#### Scenario: Untrusted or cyclic installation
- **WHEN** 缺独立准入、声明不兼容、未知进程内实现或闭合循环
- **THEN** 登记失败，已有效登记保持不变

### Requirement: Deterministic closure and fallback
解析 SHALL 形成完整依赖闭包；多实现必须有显式主备链，备用必须获准且数据/上传策略作用域一致。未选择插件不启动，不以“某实现能响应”随机切换。

#### Scenario: Ambiguous or cross-domain replacement
- **WHEN** 多实现未配置顺序，或备用不属于允许作用域
- **THEN** 不选择替代实现，报告可解释缺失/拒绝原因

### Requirement: Scoped current health and permission
Ready 判断 SHALL 复检健康期限及当前会话权限；未知枚举、无来源、过期或撤销不当作 granted；A会话权限不得授予B会话。

#### Scenario: Expired or unknown capability
- **WHEN** 健康/权限到期，权限unknown/denied或来自另一会话
- **THEN** 相应能力不可用于该会话，冻结引用复检失败

### Requirement: Explicit degraded readiness and hard gates
必需缺失 SHALL NotReady，可选缺失或显式备用 SHALL ReadyDegraded 并列原因；外发必需 guard，avatar.render 必需 avatar.profile，自动模式必须明确平台权限及有效值守/预算前置材料。

#### Scenario: Optional feature absent but core complete
- **WHEN** 场景仅可选能力不可用且硬条件/必需依赖完整
- **THEN** ReadyDegraded，不把缺失一律等同无法准备

#### Scenario: Guard absent or automatic permission unknown
- **WHEN** 外发缺guard，形象缺profile，或自动权限/值守/预算未证明
- **THEN** NotReady，不静默降为未经场景授权的音频/字幕或自动模式

### Requirement: Frozen versioned session bindings
快照 SHALL 固定会话、版本、artifact/manifest、宿主及作用域，准备期间登记更新不修改旧快照；被引用版本不得热替换，撤销仍可立即使快照无效。Ready 只表示 D04 绑定预检，不冒充宿主/设备/最终播放验收。

#### Scenario: Hot replacement or revoked selection
- **WHEN** 已冻结实现被尝试换版，或健康/权限/准入被撤销
- **THEN** 换版拒绝或旧快照失效，不自动改成另一实现

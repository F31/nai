# live-readiness Specification

## Purpose
TBD - created by archiving change v48-live-readiness. Update Purpose after archive.

## Requirements

### Requirement: 直播准备聚合
系统 MUST 基于当前工作区的已发布快照、已启用模型凭据、内置形象和实际公共输出状态展示准备清单，并为未就绪项提供对应入口。

#### Scenario: 准备状态变化
- **WHEN** 发布内容、模型目录或公共输出状态变化
- **THEN** 首页重新计算清单，且不得把平台选择、页面打开或未知硬件状态表示为已开播

### Requirement: 执行守门保持独立
准备清单 MUST 仅用于导航和说明，不得签发、续期或替代公共播放批准。

#### Scenario: 清单全部可用
- **WHEN** 当前可观察项目均为可用
- **THEN** 系统仍提示在 OBS 和直播平台人工确认音画并执行平台开播

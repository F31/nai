# 基线与范围

## ADDED Requirements

### Requirement: V5.6.1 唯一执行基线

本变更 SHALL 作为 V5.6.1 的唯一执行基线；V4.8/V5.2/V5.5/V5.6 的 ACC、INV、任务编号、数据库、配置与协议 SHALL NOT 被继承为本变更的通过依据。

#### Scenario: 引用旧 ACC
- **WHEN** 实现/报告引用 V4.8 或 V5.6 的 ACC/INV 作为通过证据
- **THEN** 视为无效；必须使用 V5.6.1 的 ACC 与证据

#### Scenario: 旧注册表
- **WHEN** 本变更开始执行
- **THEN** `tests/registry.yaml` 标为 V4.8 只读归档；本变更新建 ACC 注册表与检查器

### Requirement: 无向前兼容

系统 SHALL NOT 保证旧数据库、旧配置、旧协议、旧 ABI、旧形象包、旧动作包或旧验收记录可用；用户资产 SHALL 只通过明确的手动导入进入新数据根。

#### Scenario: 旧数据库
- **WHEN** 用户用旧 V4.8/V5.6 数据根启动
- **THEN** 系统不自动迁移/覆盖；显示“不兼容”，提供手动导入资产入口

#### Scenario: 旧资产导入
- **WHEN** 用户导入旧肖像/参考音频/知识文档
- **THEN** 系统复制、哈希、校验并记录来源；不导入旧许可结论

### Requirement: 单操作员与单实例

系统 SHALL 面向单人、单机、单直播间、单当前形象；SHALL NOT 创建多工作区、多用户、角色、审批、预算账本、outbox 或审计哈希链。

#### Scenario: 首次使用
- **WHEN** 用户首次启动
- **THEN** 系统自动建立默认数据根，不要求选择工作区/角色/预算

#### Scenario: 并发直播
- **WHEN** 用户尝试同时开启第二直播会话
- **THEN** 系统拒绝并提示单实例限制，不启动第二 worker

### Requirement: 范围变更受控

任何新增能力（LiveAct、双引擎、Pro 默认、平台 connector、RTMP、虚拟摄像头、多用户等）SHALL 先修改本变更规格或新增 OpenSpec change/ADR；SHALL NOT 在实现中静默加入。

#### Scenario: 实现中想加 LiveAct
- **WHEN** 开发者希望恢复 LiveAct
- **THEN** 先新增实验规格/ADR 与资源基线；未通过前不得进入主线

#### Scenario: 旧设计冲突
- **WHEN** V5.5/V5.6 旧设计与本变更冲突
- **THEN** 以本变更为准；旧设计标为历史/输入

### Requirement: 代码复用与删除有据

实现 SHALL 记录每个复用/替换/归档模块的理由与消费者；SHALL NOT 同时保留旧 MuseTalk+LivePortrait/双引擎切换路径作为未声明的主线。

#### Scenario: 替换 avatar worker
- **WHEN** 接入 FlashHead worker
- **THEN** 旧双模型路径归档或作为明确降级，不参与默认会话

#### Scenario: 删除模块
- **WHEN** 删除旧路径
- **THEN** 记录删除范围、影响与回滚方式；不删除用户数据/许可记录

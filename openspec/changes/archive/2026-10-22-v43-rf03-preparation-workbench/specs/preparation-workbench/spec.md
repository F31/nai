## Purpose
通过新备播界面贯通工作区、产品内容、节目、发布复核和私有纯文字预演。状态来自唯一认证owner，私有呈现证据与阅读、媒体输出和平台直播验收保持分离，不借用旧安装的业务页面或数据权限。
## ADDED Requirements
### Requirement: Preparation UI uses one authenticated owner
桌面壳 SHALL 通过固定新引擎访问独立格式2数据根，主体由可信宿主持有，浏览器不得提供actor/token或直接读写SQLite。列表 SHALL 按角色过滤、分页且有界，空闲编辑不终止owner；请求超时不得盲目重发写入。
#### Scenario: Edit and restart
- **WHEN** 用户编辑保存后关闭并重新打开工作台
- **THEN** 草稿/节目/快照可重新读取，旧预演session和未确认写入不自动重放。
### Requirement: Private preview has its own exact gate and evidence
预演 SHALL 只使用精确快照/hash/cue、私有目标、当前主体/session/epoch和有效事实许可。卡片有期限，呈现回执 SHALL 绑定精确卡片hash和当前上下文，不得升级为阅读、硬件或OBS完成。
#### Scenario: Revoked or stopped preview
- **WHEN** 事实被撤销、卡片过期、session停止或owner失联
- **THEN** 服务端拒绝新卡片/回执；界面清空显示并拒绝迟到响应，已显示范围不被宣称已收回。
### Requirement: Users prepare content without technical files
界面 SHALL 提供产品事实/来源/确认、FAQ/话术、主持风格、节目选择/编辑、影响报告和显式发布；常用选项有合理默认，未保存修改不静默丢弃。没有Provider、TTS或媒体也可完成纯内容任务。
#### Scenario: First manual preparation
- **WHEN** 用户创建工作区、输入一个产品及来源事实、保存节目并确认影响报告
- **THEN** 可发布私有快照、点击预演并停止，重启后可读取内容且不自动恢复呈现。

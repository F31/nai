# workbench-navigation Specification

## Purpose
TBD - created by archiving change v48-workbench-navigation. Update Purpose after archive.

## Requirements

### Requirement: 工作阶段导航
系统 MUST 按准备、直播、复盘分组已有工作页面并保留未保存保护。
#### Scenario: 切换工作阶段
- **WHEN** 用户选择已有页面
- **THEN** 系统进入对应页面并保持现有切换约束

### Requirement: 真实开播准备入口
系统 MUST 显示形象和输出路线的真实能力，提供既有准备和公共输出入口，不将未实现的平台直连或自定义形象标记为可用。
#### Scenario: 用户准备直播
- **WHEN** 用户查看首页
- **THEN** 系统提供内容、编排、复核和公共输出入口，并说明OBS与直播伴侣衔接及平台开播由用户完成

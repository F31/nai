# adoption-republish-proof Specification

## Purpose
提供候选采用到场次保存、发布复核、新旧快照私有预演和重启读取的真实Windows引擎工程证据，区分本机虚构Provider与真实服务认证，禁止把连接成功或草稿保存误判为业务发布完成。

## Requirements

### Requirement: 采用到重新发布工程证据
系统 SHALL 提供隔离真实引擎测试，证明采用仅更新草稿，经显式场次保存与发布后新预演才使用新话术。
#### Scenario: 新旧发布隔离
- **WHEN** 候选采用、保存场次并重新发布
- **THEN** 新快照预演包含新话术，旧发布引用及旧预演内容保持原值，重启不得重发模型请求

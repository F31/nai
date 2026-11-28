# V5.6.1 单数据根与最小 Schema 实施记录

日期：2026-11-04。任务：1.1、1.2。状态：已实现并通过本地测试；目标机未执行。

## 范围

- 单数据根：`config.json`、`nai.db`、`assets/`、`logs/`、`recordings/`、`cache/`。
- 最小 SQLite schema：`settings`、`characters`、`voices`、`sessions`、`messages`、`actions`、`events`、`clips`、`metrics`。
- 明确不建：workspace、principal/role、approval、budget、ledger、outbox、audit 哈希链。
- 路径：拒绝绝对路径、`..`、前缀、符号链接逃逸。

## 实现

| 文件 | 内容 |
|---|---|
| `adapters/storage-sqlite/src/v561.rs` | `V561DataRoot`、`V561OpenOptions`、schema、路径安全、测试 |
| `adapters/storage-sqlite/src/lib.rs` | 导出 `pub mod v561` |
| `crates/nai-runtime-core/src/lib.rs` | 抽出 `validate_data_root_path`；`validate_data_root` 继续保留“空目录”检查 |

未新增 crate、进程或第三方依赖；复用现有 storage adapter、rusqlite、`nai-runtime-core`。

## 架构原则对齐

### 复用

- 放在现有 `nai-foundation-sqlite` 适配器中，不另起存储服务。
- 数据根路径校验复用 `nai-runtime-core::validate_data_root_path`，避免第二套绝对路径/父目录/符号链接规则。
- 连接/事务/错误映射沿用现有 rusqlite 适配层。

### 轻量敏捷

- 一个数据根、一个 SQLite 文件、10 张表；无 workspace/审批/预算/账本/outbox。
- 无迁移框架、无新常驻进程；首次打开 `CREATE TABLE IF NOT EXISTS` 幂等。
- `V561OpenOptions` 只注入配置，不引入额外生命周期。

### 开放解耦

- 存储细节不进入 engine/UI；后续 engine 只通过窄接口使用数据根/仓储。
- 配置以 `serde_json::Value` 注入和读取，不把具体部署默认值写死在调用方。
- 视觉/TTS/动作后续继续走独立适配器；存储不依赖模型或平台 SDK。

### 性能

- WAL、`synchronous=NORMAL`、`temp_store=MEMORY`、32MB cache、256MB mmap。
- 5s busy timeout；索引覆盖 session/message/action/event/metric 查询。
- 单写者、单连接入口，避免多进程争用；后续热路径使用 `prepare_cached`。

### 可配置

- `open_with(root, V561OpenOptions { config })` 可注入部署/测试配置。
- `load_config` / `save_config` 读写 `config.json`；已有配置不被 `open()` 覆盖。
- 默认配置包含 app/server/llm/tts/avatar/output/filters；不包含密钥明文。

### 可扩展

- 表已覆盖会话、消息、动作、事件、片段、指标；后续可按需增加字段/表。
- `settings.schema_version` 与 `PRAGMA user_version` 记录版本，为后续最小迁移留入口。
- 不预建未使用的工作区/审批/账本结构，避免把 V4.8 复杂度带回来。

## 测试

```bash
cargo test -p nai-foundation-sqlite v561 --locked
```

结果：11 passed（后续 1.5/1.6 增补）。

覆盖：

- 单数据根/默认配置创建；
- 仅 10 张表，且无 workspace/role/approval/budget/ledger/outbox/audit；
- 幂等重开；
- 绝对路径、`..`、符号链接逃逸拒绝；
- 注入配置、保存后重开保留；
- 相对根拒绝；
- WAL / user_version / synchronous。

## 已知边界

- 尚未接 engine/UI；尚无热路径仓储 API（settings/character/session 的读写）。
- 尚未实现最小迁移（当前只有 v1 schema）；后续新增字段时补 `schema_version` 迁移。
- 目标机路径（`E:\nai-data`）与 Windows reparse point 需真实运行验证。
- 未做多进程写竞争测试；V5.6.1 假设单写者。

## 下一步

- 1.4/1.5：依赖探测、配置读写/脱敏、系统凭据。
- 定义 engine 侧窄接口后再接 `V561DataRoot`，避免把 SQLite 类型泄漏到会话层。
- 热路径补充 `prepare_cached`/批量事务与性能基准。

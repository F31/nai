# V5.6.1 配置脱敏、系统凭据与资产导入实施记录

日期：2026-11-04。任务：1.5、1.6。状态：已实现并通过本地测试；UI/引擎接线与目标机 M0 未执行。

## 1.5 配置与系统凭据

实现：

- `V561DataRoot::redacted_config()`：递归掩码 `api_key/api_keys/secret/token/password/authorization/cookie/credential` 等值；保留 `api_key_ref` 可见。
- `V561DataRoot::reset_config()`：恢复 V5.6.1 默认配置。
- `save_config`/`open_with` 拒绝明文 secret，要求 `api_key_ref = keyring:<service>/<account>`。
- `adapters/llm-openai/src/credential.rs`：解析 keyring 引用；Windows 下 load/store/delete 走 OS credential store；非 Windows 显式 `CapabilityUnavailable`，不落明文文件。
- 不读取/打印 secret；不把 keyring 引用误当 secret。

测试：

- 配置注入明文 secret 被拒；
- save 明文 secret 被拒；
- redact 保留 `api_key_ref`、掩码 secret；
- reset 恢复 flashhead 默认；
- keyring 解析接受 `keyring:nai/llm`，拒绝空/多斜杠/空格；
- 非 Windows 存储/读取/删除显式不可用。

## 1.6 旧资产手动导入

实现：

- `V561AssetKind::{Portrait,Voice,Knowledge}`；
- `V561DataRoot::import_asset(source, options)`：
  - 只接受普通文件（拒绝 symlink/目录/空文件）；
  - 大小上限：肖像 20MiB、参考音频 50MiB、知识文档 10MiB；
  - 计算 SHA-256，复制到 `assets/characters|voices|knowledge/<kind>-<hash16>.<ext>`；
  - 写入 `characters`/`voices`/`knowledge_docs`；
  - 只记录本次明确传入的 `license_note`，不导入旧许可/旧数据库。
- 新增 `knowledge_docs` 表；`voices` 增加 `source_hash`；schema version 2（新基线，无旧迁移）。

测试：

- 肖像导入：文件存在、hash 64 位、DB name/hash/license 正确；
- 超限知识文件拒绝；
- symlink 肖像拒绝；
- 仅 10 张 V5.6.1 表，无 workspace/role/approval/budget/ledger/outbox/audit。

## 验证

```bash
cargo test -p nai-foundation-sqlite v561 --locked   # 11 passed
cargo test -p nai-llm-openai credential --locked     # 4 passed
cargo check --locked                                  # pass
cargo xtask check-deps                                # pass, 18 packages
cargo xtask check --profile workbench                 # pass
```

## 架构原则

- 复用现有 storage adapter / llm-openai adapter，不新增 crate。
- 配置只存引用，不存明文；非 Windows 明确不可用，不伪造成功。
- 资产导入只复制用户提供的文件并哈希，不做旧许可迁移。
- 表数从 9 增至 10；仍不引入 workspace/审批/预算/账本/outbox。
- 每个上限/路径/错误都有测试；未知/不可用保持显式。

## 未完成

- keyring 模块尚未接入 UI/引擎配置页（2.x/6.x 接线）。
- 1.3b 安装包/venv/模型下载未做（许可/权重来源未冻结）。
- 1.7 卸载/重装未做。
- 目标机 Windows M0 仍 Blocked。

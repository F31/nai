## Why

`tests/registry.yaml` 里升级器这条线的真实状态是：**R05**（升级原子与回滚）、**R09**（升级预检与下载失败）、**R10**（开播与回滚）三项全为 `pending`，notes 一律写着「upgrade-core 未实现（W15）」；**S08**（插件与供应链）虽为 `implemented`，notes 却明确「撤销列表与 SBOM 待建」；**O05**（升级成本显示）同样 `pending`。

规格把这条线写得很全，代码里一行都没有：

- V3.6 §14.3 给出**九步事务式流程**：解析依赖闭包 → 预检（系统、GPU、驱动、双份磁盘空间、凭据）→ 暂存下载并验签、验哈希、查撤销 → 等待无活动会话 → 一致性备份 → 迁移到新数据目录 → 隔离自检 → 写 journal（prepared、migrated、checked、committed、rolled_back）→ 原子替换当前版本清单 → 展示实际能力状态。
- §4.4：**升级包、插件、模型都要校验哈希与签名并检查撤销列表**。
- §11.6 / §14.3：撤销列表在**安装与每次启动时**检查，被撤销的插件或模型**立即禁用**。
- INV-16：升级只在**无活动会话**时激活，原子可恢复；回滚**不丢提交后的新写入**。

关键是**架构早已为它留好位置**，本轮不是重新设计：

- `xtask/src/deps.rs` 的 `expected_layer` 已把 `crates/nai-upgrade-core` 列入 **shared** 层（禁止依赖 plugin/runtime/app/tool）。
- `migrations/sqlite/0001-foundation.sql` 已有 `upgrade_journal(transaction_id, from_manifest, to_manifest, step, backup_ref, error_code)` 与 `plugin_installations(..., manifest_hash, certification_ref)`。

可复用的基座也已就位：

- `nai-storage::snapshot` 提供一致性 `backup`/`restore`（清单校验、`PRAGMA integrity_check`、工件 SHA256）；
- `nai-storage::migration` 提供版本化 `schema_migrations`（checksum + DDL 漂移检测）与**只向前**的 `migrate`；
- `nai-plugin-sdk::model::check_model_integrity` 已给出「JSON Schema 校验 + SHA256 + 有界 + 可取消」的完整性范式。

因此本轮的目标：**把「升级原子、可预检、可回滚、不伪报」从声明变成可执行判据**，同时不引入新依赖、不把机器/网络条件烘进 portable 检查。

## What Changes

- **新增 crate `crates/nai-upgrade-core`（shared 层）**：升级器业务内核。只依赖 `nai-contracts`（错误码）、`nai-storage`（journal/备份/版本清单）、`nai-security`（`Operation`/`Secret`）、`sha2`、`serde_json`、`semver`——**全部为已有依赖**。不依赖任何宿主、App、Tauri 或平台 SDK。

- **升级包清单（新增 `schemas/config/upgrade-package.schema.json`）**：一个升级包 = 一个不可变清单 + 一组按路径寻址的内容文件。清单声明包身份与版本、目标内核范围、依赖能力、支持的宿主/平台/架构、每个文件的 SHA256 与字节数、安装字节、签名与签名者引用、许可引用、迁移引用、回滚兼容范围、**成本构成**（下载/云资源/存储/预估运行增量）。

- **校验（`verify.rs`）**：按 `check_model_integrity` 同构的「先限长再分配、入口/分块/出口复检 deadline 与 cancellation」方式做：JSON Schema → 清单内文件声明唯一且路径规范 → 内容集合精确匹配（缺失/额外/重复一律拒绝）→ 逐文件 SHA256 → **签名验证（可注入 `SignatureVerifier`）** → **撤销列表检查**。任一步失败都**不产生暂存**。

- **撤销列表（`revocation.rs`，补 S08）**：`RevocationList` 是**带期限的可信缓存**——条目含被撤销对象身份与撤销时间，来源可注入（`RevocationSource`）；列表自身有 `issued_at`/`expires_at`，**过期的缓存不被当作「无撤销」**，而是显式不可用。安装与每次**启动**都必须复检（提供 `check_startup` 入口），被撤销的对象**立即禁用**、不进入 Ready。

- **预检（`preflight.rs`，R09 主体）**：解析依赖闭包（`semver` 范围对目标内核与依赖能力求交），检查平台/架构、**双份磁盘空间**（下载 + 暂存 + 迁移）、凭据可用性、GPU/运行时矩阵条目。**每一项资源都以可注入探针求值**（`SpaceProbe` 等），探针返回「未知」时**显式拒绝激活**并给出原因，**绝不伪报可用**。预检失败时**原版本继续运行**（不触碰当前版本清单）。

- **journal 与事务（`journal.rs` / `plan.rs`，R05）**：五状态 `Prepared → Migrated → Checked → Committed`（异常终点 `RolledBack`）逐状态持久化到既有 `upgrade_journal` 表（含 `backup_ref` 与 `error_code`）。编排顺序与 §14.3 一致；**崩溃恢复幂等**——重启后按 journal 的最后一步决定「继续」还是「回滚」，重复执行不产生叠加副作用；**原子替换当前版本清单**用写入新文件 + 原子改名（复用 `snapshot.rs` 的 `write_new`/`flush_durable` 约定）实现；回滚 = 恢复旧程序与备份，并**导出升级后的新写入**（事实与审计），不静默丢弃。

- **会话门控与降档（`gates.rs`，R10）**：`ActiveSessionGate` 使**开播中只暂存**——`stage` 允许、`activate` 被门控；激活要求无活动会话。降档**先排空多余会话**再停用依赖能力，**数据保留**。不兼容（回滚范围外或目标库版本更旧）时**阻止静默回滚**，返回显式错误。

- **成本显示（`cost.rs`，O05）**：`CostEstimate` 逐项列示**云资源、下载、存储增量、预估运行增量**，单位与口径显式；**无法估算的项标注为未知而非 0**——一个 0 会被读成「免费」，那是伪报。

- **接缝一律可注入、不绑定实现**：签名验证、撤销来源、磁盘探针、活动会话判定都由 trait 注入；默认实现只用 `std` 与既有依赖，**不引入 GPG/Ed25519/windows crate**。真实非对称签名与 Windows 文件锁由后续 update-helper 接入，本轮显式标注为未做。

## Capabilities
### New Capabilities
- `upgrade-transaction`：升级包校验、撤销列表、预检、事务与 journal、会话门控与降档、成本显示。覆盖 registry **R05 / R09 / R10 / O05** 与 **S08** 的「撤销列表」缺口。

### Modified Capabilities
- 无。既有 `local-persistence`（备份/迁移）与 `plugin-hosts-and-ipc`（验签加载）**不改**，本变更在其之上新增升级编排层。

## Impact

- 代码：新增 `crates/nai-upgrade-core/{Cargo.toml,src/*.rs,tests/*.rs}`；`Cargo.toml`（workspace members / default-members / workspace.dependencies 增加 `nai-upgrade-core`）；新增 `schemas/config/upgrade-package.schema.json`；`xtask/src/lib.rs` 增加 `test --suite upgrade`。
- 规格：新增 capability `upgrade-transaction`；新增 `docs/upgrade.md`、ADR 0026、报告 `docs/reports/<date>-w15-upgrade-core.md`。
- 依赖：**零新增**。`semver`、`sha2`、`serde_json`、`rusqlite` 均在 workspace 既有锁定版本内。
- registry：R05/R09/R10/O05 补证据并可依实现对 `pending → implemented`（**不标 `tested`**：无真实 Windows 安装、无 update-helper、无真实签名链）；S08 补「撤销列表已建」证据但**保持 `implemented`**；`A08`（档位升级不重装内核）本轮**不动**（属 R2/W15+W25 交集）。
- 明确不做：不接真实代码签名/GPG 私钥链；不做 Windows 应用停止后的独立 update-helper（文件锁）；不做真实下载器与断点续传的字节流实现（本轮定义校验与预检的**判据**，下载器作为可注入接缝）；不安装真实内核升级（缺 Windows 安装环境，`O04` 仍 `blocked`）。

## Non-Goals

- 不用演练结果替代外部认证：本轮证明的是**升级事务的架构判据**（原子、可恢复、可预检、不伪报），不是「Windows 上真的升级过」。
- 不把 GPU/驱动探测写成平台特定实现：探针返回未知即拒绝激活，**不猜**。
- 不改 `nai-storage` 的迁移语义（保持「只向前」），回滚靠备份恢复 + 新写入导出，不新增反向迁移。

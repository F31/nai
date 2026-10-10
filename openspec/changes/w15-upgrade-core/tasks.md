## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w15-upgrade-core`（proposal/design/tasks + `upgrade-transaction` 增量），`validate --strict` 通过。
- [x] 1.2 新增 `schemas/config/upgrade-package.schema.json`：包身份与版本、`kernel_range`、`requires`、`hosts`/`os`/`architectures`、运行时矩阵、文件声明（`path`+`sha256`+`bytes`）、`install_bytes`、`migrated_bytes`、签名者引用 `signer_ref`、`license_ref`、`migration_ref`、`rollback_range`、成本构成。签名本体作为 `verify_package` 的独立输入（清单不自引用）。
- [x] 1.3 新增 `crates/nai-upgrade-core`（`layer = "shared"`），接线 `Cargo.toml`（members / default-members / workspace.dependencies）；`cargo xtask check-deps` 通过（18 个包）。

## 2. 包校验与撤销列表（S08）

- [x] 2.1 `manifest.rs`：解析升级包清单为强类型 `UpgradePackage`；路径规范（无空段/点段/父目录段）；文件声明路径唯一；版本可被 `semver` 解析。
- [x] 2.2 `verify.rs`：`verify_package` 做 schema → 声明闭包 → 内容集合精确匹配（缺失/额外/重复拒绝）→ 逐文件字节数与 SHA256 → 签名（`SignatureVerifier`）→ 撤销（`RevocationList`）；**先限长再分配**、入口/分块/出口复检 deadline 与 cancellation；任一步失败**不产生暂存**。
- [x] 2.3 `revocation.rs`：`RevocationList{issued_at, expires_at, entries}` 三态判定——命中拒绝、未命中且未过期放行、**已过期拒绝**；`check_install` 与 `check_startup` 两个入口（安装与启动都要查）。
- [x] 2.4 `signature.rs`：`SignatureVerifier` 接缝 + `DeterministicVerifier`（演练替身，基于 `sha2`，文档标注**非真实签名链**）。
- [x] 2.5 单测与集成测试：篡改内容、额外/缺失文件、重复路径、超限、取消/期限、签名失败、撤销命中、**撤销缓存过期**、清单 schema 拒绝。

## 3. 升级预检（R09）

- [x] 3.1 `preflight.rs`：`preflight` 解析依赖闭包（`semver` 对 `kernel_range` 与 `requires` 求交）、校验 `hosts`/`os`/`architectures` 与运行时矩阵条目。
- [x] 3.2 资源三态：`SpaceProbe`（**双份空间** = `install_bytes` 暂存 + `migrated_bytes` 新目录）、凭据可用性、运行时；`Unknown` 与 `Insufficient` **同等拒绝**，且错误码可区分（`CAPABILITY_NOT_READY` / `RESOURCE_EXHAUSTED`）。
- [x] 3.3 预检只产出 `PreflightReport`（逐项结果 + `refusal_error`），**不触碰当前版本清单**；`NoSpaceProbe` 默认「未知」即拒绝。
- [x] 3.4 单测：版本不兼容、平台/架构不符、依赖闭包缺项、空间不足、探针未知、凭据未知、全部满足通过。

## 4. journal 与升级事务（R05）

- [x] 4.1 `nai-storage/src/upgrade.rs`：`UpgradeStep`（`Prepared/Migrated/Checked/Committed/RolledBack`）与 `UpgradeRecord`；`Store::record_upgrade` 以 `step` 单调推进的 `UPSERT` 写入既有 `upgrade_journal` 表，`backup_ref`/`error_code` 一经写入不被后续步骤抹除。
- [ ] 4.2 `plan.rs`：编排 `stage → preflight → backup → migrate → self-check → commit`，异常收敛到 `rolled_back`；**原子替换版本清单**（写新 `manifest.new` → `flush_durable` → 改名）。
- [ ] 4.3 崩溃恢复幂等：`recover` 按当前 step 决定继续或回滚；重复调用不叠加副作用；`committed` 重入返回成功。
- [ ] 4.4 回滚：恢复旧程序与备份，并**导出升级后新增事实与审计**（不静默丢弃）。
- [ ] 4.5 单测：正常全流程、各步失败回滚、断电（构造中间 step 后重入）、重复提交幂等、回滚保留新写入。

## 5. 会话门控、降档与成本（R10 / O05）

- [ ] 5.1 `gates.rs`：`ActiveSessionGate` 使开播中**只暂存**（`stage` 允许、`activate` 被门控）；激活要求无活动会话。
- [ ] 5.2 降档：先排空多余会话再停用依赖能力，**数据保留**；不兼容（目标更旧或超 `rollback_range`）**阻止静默回滚**并报错。
- [ ] 5.3 `cost.rs`：`CostEstimate` 逐项列示云资源/下载/存储增量/预估运行增量，分项为 `Known(amount, unit) | Unknown(reason)`，**未知不冒充 0**。
- [ ] 5.4 单测：有活动会话时激活被拒而暂存成功、降档排空后才停用、数据仍在、不兼容回滚被拒、成本未知项标注、已知项求和。

## 6. 规格、文档与提交

- [ ] 6.1 新增 `docs/upgrade.md`（接缝与三态语义、九步流程、journal 状态机与恢复、撤销期限规则、成本口径、未做边界）。
- [ ] 6.2 新增 ADR 0026，记录 crate 分层选择与三条否决方案（A 塞 storage / B 放 runtime / 以及「撤销列表做成集合」）。
- [ ] 6.3 报告 `docs/reports/<date>-w15-upgrade-core.md`：实测数字、逐项验收映射、未覆盖边界。
- [ ] 6.4 `xtask/src/lib.rs` 增加 `test --suite upgrade`；更新 help 文案。
- [ ] 6.5 `tests/registry.yaml`：R05/R09/R10/O05 补证据并可 `pending → implemented`（**不标 tested**）；S08 补撤销列表证据**保持 implemented**；A08 不动。`AGENTS.md` 与 `docs/development-plan.md` 记录本轮。
- [ ] 6.6 归档 `w15-upgrade-core`，跑 `validate --all --strict`（含 `--archived`）。
- [ ] 6.7 全量 `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4`）通过并留档；提交 + 推送。

## 里程碑切分

本变更按可独立验证的切片推进，每个切片都以一次提交收口：

- **Slice A（已完成）**：立项与骨架（§1）、包校验与撤销列表（§2，S08 撤销缺口）、升级预检（§3，R09 主体）。覆盖「任何东西在暂存前先被验签验哈希查撤销」与「不足或不可判定即拒绝」两条判据。
- **Slice B**：journal 与升级事务（§4，R05），含崩溃恢复幂等与回滚保留新写入。
- **Slice C**：会话门控/降档与成本显示（§5，R10 / O05）。
- **Slice D**：文档、ADR、registry、归档与全量门控（§6）。

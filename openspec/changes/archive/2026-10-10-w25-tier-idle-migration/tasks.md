## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w25-tier-idle-migration`（proposal/design/tasks + `deployment-lifecycle` 增量），`validate --strict` 通过。
- [x] 1.2 确认复用基座：`nai-upgrade-core` 的 `verify_package` / `RevocationList` / `Operation` 签名可用；离线安装不需要新增依赖——crate 无 HTTP 客户端即「不会偷偷下载」的**编译期**保证，而非一个恒为 `false` 的开关。

## 2. 档位驻留与空闲回收（A08 / A09）

- [x] 2.1 `residency.rs`：`ResourceKind`（封闭四族 `Process | Port | Vram | CloudRequest`）与 `ResourceClaim { kind, name, amount }`；声明校验（族内名字唯一、amount > 0、名字规范且有界）；能力数上限 1024。
- [x] 2.2 `ResidencyLedger`：`declared` 于 `install` 登记、`held` 只在 `enable` 物化——因此「未安装/未启用 ⇒ 零占用」是**结构性质**。`install` 拒绝重复安装与非规范足迹；`enable` 对**排他族**（`Process`/`Port`）做冲突检查，被别的启用能力持有即 `CONFLICT` 拒绝（**共享族** `Vram`/`CloudRequest` 不排他，两个能力可各自加载同一模型）；`disable`/`uninstall` 在仍有会话服务时拒绝。
- [x] 2.3 `IdlePolicy` 分族 TTL（worker 30s / 端口随进程 30s / 未用预览云请求 60s / 模型显存 300s，方向由**重载代价**决定）与 `Residency`（`Disabled | Serving{sessions} | Resident{since_ms}`）；`session_started` 清除空闲时钟，`session_ended` 在归零时起时钟；时间以参数进入，核心不取墙上时钟。
- [x] 2.4 `reclaim(now)`：只释放 **TTL 已到期**的声明，`ReclaimReport` 给出逐能力释放明细与 `held_back`（仍保留的声明数）；**正在服务会话的能力绝不被回收**；回收只动资源声明——不改启用状态、不删数据。
- [x] 2.5 审计做成对**公开快照值** `CapabilityResidency` 的**纯函数** `audit_residency(entries, policy, now)`：宿主可拿它审计自己的视图，且每条规则都能被手工构造的状态真实触发（只有误用才可达的检查既没法测也没法证明它活着）。六个稳定标记：`idle resource past its ttl`（可达的泄漏检测）、`disabled capability holds resources`、`enabled capability holds undeclared resource`、`exclusive resource held twice`、`idle capability has no idle clock`、`serving capability has an idle clock`；`held()`/`total(kind)` 给出当前占用与合计。
- [x] 2.6 `Tier` 与 `tier_plan(from, to)`：加装/移除集合由沿途增量并成一条图，`Lite → Pro` 不需要中间档且等于两段之并；集合为 `&'static str` 常量，**不可能含内核**；同档位双空。
- [x] 2.7 测试 `tests/lifecycle.rs`（17 项）：安装未启用零占用、启用物化足迹、排他冲突被拒且状态不变、共享族不排他、模型 TTL 未到不卸载（分级释放 + `held_back`）、TTL 到期释放四族、繁忙能力不被回收、回收忽略禁用能力、`disable` 先拒绝后释放且可再启用、足迹校验（重复/零量/缺能力）、`uninstall` 移除、会话转换守卫、审计六种标记逐条触发、`IdlePolicy` 拒绝负 TTL、档位差集与内核缺席、诊断是稳定标记而非 OS 字符串。

## 3. 离线安装与部署迁移（R11）

- [x] 3.1 `offline.rs`：`OfflineArtifact` + `install_offline`。前置检查「每个声明文件都在本地」，缺件以 `CAPABILITY_NOT_READY` 拒绝，另有 `missing_artifacts` 指出**是哪个文件**（与在线路径的「missing package content」可区分）；随后复用 `verify_package` 全链（Schema → 内容集合 → 逐文件 SHA256 → 签名 → 撤销）；拒绝时不留暂存。
- [x] 3.2 `migration.rs` 类型：`Deployment::{Local, Hybrid, Distributed}`、`PermissionGrant`、`PermissionMap`（按目标形态查表）、`DeploymentState { deployment, permissions, evidence_version, data_hash }`。
- [x] 3.3 `plan_migration`：起点等于目标即拒绝（`INVALID_ARGUMENT`，不是空操作成功）；有活动会话即拒绝（维护窗口，`CAPABILITY_NOT_READY`）；分布式目标 + SQLite 写者 > 1 即拒绝（`CONFLICT`）；权限无映射即拒绝（不静默丢弃）。计划带 `source`，使复验比较的是**实际导出**的那份状态。
- [x] 3.4 `verify_migration`：复验权限集合一致（映射后期望值，顺序无关）、证据版本一致（`VERSION_INCOMPATIBLE`）、数据哈希一致（`INVALID_ARGUMENT`）——三者错误码可区分。**否决**「只比哈希」：哈希一致证明不了权限被正确映射（`admin.team` → `viewer` 照样哈希一致）。
- [x] 3.5 `OutputSwitch::switch(key)`：同键第二次报告 `Duplicate`，即「输出切换不重复播报」；key 由调用方给（典型 `session/epoch`），因为「同一次切换」的身份只有调用方知道。
- [x] 3.6 演练宿主 `RehearsalMigration`：内存内忠实导出/导入（搬权限、保留证据版本与哈希），并用 `MigrationFault`（`DroppedPermission` / `EvidenceVersionDrift` / `DataHashDrift`）重现「一次错误的导入长什么样」，使三项复验可被证明真的会触发。
- [x] 3.7 测试 `tests/migration.rs`（11 项）：完整离线集经同一校验链安装、缺件被拒且被指出（与空集两种情况）、离线路径同样拒绝篡改/错签名/撤销命中/未声明多余文件、活动会话阻止迁移、多写者被拒（同一写者数在 Hybrid 下可通过）、同形态被拒、无映射权限被拒、迁移后权限/证据版本/哈希三项保持、三项复验各自失败路径与错误码区分（含重排不算不一致）、三种演练故障被捕获、输出切换幂等与非法 key。

## 4. 规格、文档与提交

- [x] 4.1 新增 `docs/deployment.md`（驻留与四族排他性、TTL 表与回收语义、审计标记与可达性、档位迁移图、离线拒绝与编译期保证、迁移三种拒绝与三项复验、未做边界）。
- [x] 4.2 新增 ADR 0027，记录放入 `nai-upgrade-core` 而非新 crate 的理由，并记录否决方案（独立 crate / 足迹写成文档 / 由宿主上报 / 立即释放 / 档位即内核配置 / `allow_fetch` 开关 / 只比哈希）。
- [x] 4.3 报告 `docs/reports/2026-10-10-w25-tier-idle-migration.md`：实测数字、逐项验收映射（A08/A09/R11）、未覆盖边界与环境边界。
- [x] 4.4 `tests/registry.yaml`：A08/A09/R11 补证据并按实现 `pending → implemented`（**不标 `tested`**——A08/A09 为 `manual` 且需真实设备与长播、R11 需真实轻云实例）；`AGENTS.md` 与 `docs/development-plan.md` 记录本轮。`check --registry` 通过。
- [x] 4.5 归档 `w25-tier-idle-migration`（归档名 `2026-10-10-w25-tier-idle-migration`，新 capability `deployment-lifecycle` 生效、7 条需求），跑 `validate --all --strict`（含 `--archived`）。
- [x] 4.6 全量 `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4`）通过并留档（EXIT=0，72 目标，396 passed / 基线 368）；提交 + 推送。

## 里程碑切分

本变更按可独立验证的切片推进：

- **Slice A**：立项（§1）与档位驻留/空闲回收（§2，A08/A09）。覆盖「未启用即零占用」「排他冲突拒绝」「按 TTL 而非立即释放」「繁忙不回收」「审计可检出」五条判据。
- **Slice B**：离线安装与部署迁移（§3，R11）。覆盖「缺件拒绝且不抓取」「维护窗口与单写者」「权限无映射即拒」「权限/证据版本/哈希三项一致」「输出切换不重复播报」五条判据。
- **Slice C**：文档、ADR、registry、归档与全量门控（§4）。

Slice A 与 Slice B 的代码在同一工作状态内完成并一起门控：两者共用 `nai-upgrade-core`，分开提交只会让第一次提交的测试目标不完整。Slice C 为纯文档/registry 变更，代码状态与门控时完全一致，故沿用同一次门控留档。

# W25 实测报告：档位驻留、空闲回收与部署迁移（A08 / A09 / R11）

- 日期：2026-10-10
- 变更：`openspec/changes/w25-tier-idle-migration/`（归档后见 `archive/2026-10-10-w25-tier-idle-migration/`）
- 决策：[ADR 0027](../adr/0027-deployment-lifecycle-boundary.md)；指南：[docs/deployment.md](../deployment.md)
- 判据：`tests/registry.yaml` A08、A09、R11

## 结论

W15 把「一个升级包能不能装」落成了可执行编排。本轮补上 §14 的另一半：**装完占多少**（A08）、
**闲置怎么放手**（A09）、**离线与换部署形态怎么办**（R11）。三项全部在 `nai-upgrade-core`（shared 层）内完成，
**零新增依赖**（依赖方向仍 18 个包），全部判据在 portable 下可测。

三项 registry 状态由 `pending` 升为 `implemented`，**均不标 `tested`**：A08/A09 的 `automation: manual` 需要真实设备与
长播，R11 需要真实轻云实例——本轮证明的是**架构判据**，不是「在真机上跑过」。

## 目标行为

| 判据 | 实现 | 关键设计 |
|---|---|---|
| 未启用即零占用 | `ResidencyLedger`（声明式足迹，只在 `enable` 物化） | 「禁用即零占用」是**结构性质**：只有 `enable` 写 `held` |
| 排他冲突拒绝 | `enable` 对 `Process`/`Port` 做冲突检查 | `Vram`/`CloudRequest` **不排他**：两个能力可各自持有同一模型/端点 |
| 按 TTL 而非立即释放 | `IdlePolicy` 分族 TTL + `reclaim(now)` | 只释放到期项并报告 `held_back`；繁忙能力绝不回收 |
| 回收不删数据不改状态 | `reclaim` 只动资源声明 | 回收**不是**降档；`disable` 在仍有会话服务时拒绝 |
| 审计可检出 | `audit_residency`（对公开快照值的纯函数） | 宿主可审计自己的视图；每条规则都能被手工构造的状态触发 |
| 档位不重装内核 | `tier_plan`（两段差集的并） | Lite→Pro 不需中间档；集合为常量 ⇒ 内核不可能出现 |
| 离线缺件拒绝且不抓取 | `install_offline` + `missing_artifacts` | 「不会偷偷下载」是**编译期**性质（crate 无 HTTP 客户端） |
| 迁移三种拒绝 | `plan_migration` | 有活动会话 / 分布式 + SQLite 多写者 / 权限无映射 |
| 迁移三项复验 | `verify_migration` | 权限集合、证据版本、数据哈希，**错误码可区分** |
| 输出切换不重复播报 | `OutputSwitch::switch(key)` | key 由调用方给（典型 `session/epoch`），同键第二次 `Duplicate` |

## 实测数字

```
cargo test -p nai-upgrade-core --offline
cargo xtask check-deps
cargo run -p nai-xtask -- check --registry
cargo xtask check --profile portable
```

- `nai-upgrade-core`：**3 lib + 17 lifecycle + 11 migration + 12 plan + 8 upgrade = 51 passed / 0 failed**
  （本轮新增 17 + 11 = 28 项）
- `cargo xtask check --profile portable`：**EXIT=0，72 个测试目标（55 二进制 + 17 doc-test），396 passed / 0 failed**
  （基线 368 / 70 目标，+28；新增的两个测试文件各是一个独立测试二进制，故二进制目标 +2）
- `cargo xtask check-deps`：**18 个 workspace 包通过**（无新增包、无依赖方向变化）
- `check --registry`：**pass**（`# fail 0`，V3.6 验收 + D01–D14 映射）
- clippy `--all-targets`：干净（`-D warnings`）；`cargo fmt --all --check`：干净

末行原文：`portable: engineering foundation checks passed; device/SDK/GPU/install certification not performed`

## 逐项验收映射

### A08 档位升级不重装内核（`automation: manual` → `implemented`）

| 规格句子 | 判据 |
|---|---|
| 「Lite → Standard → Pro 使用**同一迁移图**，不要求先装中间档」 | `tier_plan(Lite, Pro).install` == `STANDARD_ADDITIONS ∪ PRO_ADDITIONS`，且与两次单跳之并逐项相等（`tier_plan_uses_one_graph_without_the_intermediate_tier`） |
| 「不重装内核」 | 三个档位两两组合共 9 对，`install`/`remove` 均不含内核（`tier_plan_never_touches_the_kernel`）；因集合是常量，这是**数据的性质** |
| 「未启用的能力不占用进程、端口、显存或云请求」 | 安装后 `held()` 为空、`total(Vram)==0`、`residency_of==Disabled`、审计干净（`install_without_enable_holds_nothing`） |

### A09 空闲回收（`automation: manual` → `implemented`）

| 规格句子 | 判据 |
|---|---|
| 「未安装的模块没有后台进程、端口、显存与云请求」 | 足迹只在 `enable` 物化；`uninstall` 移除条目，`held()` 为空 |
| 「结束场次后按 TTL 卸载模型」 | 会话结束后 TTL 未到 ⇒ 模型仍驻留、`held_back==1`；超过 300 s ⇒ 四族全部释放 |
| 「结束临时 worker」「关闭未用预览」 | 同一 `reclaim` 在 60 s 处释放 `Process`/`Port`/`CloudRequest` 三族、保留 `Vram`——**分级释放** |
| 回收不得越界 | 繁忙能力在任意时刻 `reclaim` 都返回空且不释放；`reclaim` 不改 `residency_of`、不删数据 |

### R11 离线升级与部署迁移（`automation: fault` → `implemented`）

| 规格句子 | 判据 |
|---|---|
| 「离线包可安装」 | 完整本地集合经**同一条**校验链安装通过（`a_complete_local_set_installs_through_the_same_verification`） |
| 缺件必须拒绝 | `CAPABILITY_NOT_READY` + `reason == "offline install lacks a local artifact"`，且 `missing_artifacts` 指认 `data/model`（单缺与全缺两种情况） |
| 离线路径不放松校验 | 篡改内容、错签名、撤销命中、未声明多余文件四种情况全部拒绝 |
| 「权限与证据版本一致」 | 迁移后 `permissions == plan.expected`、`evidence_version` 与 `data_hash` 与源相同 |
| 「输出切换不重复播报」 | 同键第二次 `Duplicate`、`announced_count()` 不增；不同键仍 `Announced` |
| 「实时链路留在原部署，在维护窗口切换」 | `live_sessions > 0` ⇒ `CAPABILITY_NOT_READY` |
| 「SQLite 不挂多节点写」 | 分布式目标 + `sqlite_writers > 1` ⇒ `CONFLICT`（同一写者数在 Hybrid 下可通过） |
| 权限不得静默丢弃 | 无映射 ⇒ `INVALID_ARGUMENT`，不改写也不丢弃 |

## 未覆盖边界（必须如实记录）

- **`Vram` 是声明式索赔**，不是真实显存配额。真实 GPU 探测仍需宿主注入（与 `preflight` 的探针同构）。
- **R06（2h/8h/24h 长播无资源增长）与 B04（整链路延迟分位）未做**：需真实设备与长时运行，`automation: manual` 未变。
  本轮产出的是「账本不会给未启用能力留资源、TTL 到期一定释放」这条结构性判据，**不是**任何真机上的内存曲线。
- **R11 的真实迁移未执行**：无真实轻云实例与 TLS（§27.4 属独立任务）；`RehearsalMigration` 是内存宿主。
- **A10（云断连不伤本地链路）与 A11（Pro 多会话隔离）未做**，是独立主线。
- **A08 的真实档位加装未执行**：需要真实插件包与设备。
- 审计的六条规则中只有 `idle resource past its ttl` 在正确使用下可达；其余五条是**改错时的守卫**，
  靠 `audit_residency` 对快照的纯函数测试覆盖（`audit_cannot_hide_a_leak` 逐条构造），并不声称「误用时会自然发生」。
- 真实 Windows 安装器、update-helper、代码签名链（W15 遗留）仍不在范围内。

## 环境边界

- 宿主机为 Windows MSVC（Rust 1.96.0）；`CARGO_TARGET_DIR=E:/nai-build-target`。
- `CARGO_BUILD_JOBS=2`、`RUST_TEST_THREADS=4`、`CARGO_INCREMENTAL=0` 是 cargo/libtest 原生开关，用于规避本机
  批量链接争用与增量会话目录锁残留（宿主事实），**不写进** portable 检查——本机约束不是项目约束。
- 本轮门控**首跑即过、无 LNK1104**，与前两轮一样印证该现象与本机瞬时状态相关而非必然复现。
- `cargo test -p nai-upgrade-core` 不需要网络（离线安装测试的工件全部在内存里）。

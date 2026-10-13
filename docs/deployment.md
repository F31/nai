# 部署生命周期：驻留、回收、档位与迁移

W15 的升级器回答「一个包能不能装」。这份文档回答它之后的三个问题，仍在同一个 crate
（`nai-upgrade-core`，shared 层）：**装完占多少**（A08）、**闲置怎么放手**（A09）、**离线与换部署形态怎么办**（R11）。
四者同属 V3.11 §14「部署、档位与升级」，共享同一句不变量——§14.4 末句「未启用的能力不占用进程、端口、显存或云请求」。

```bash
cargo xtask test --suite upgrade      # nai-upgrade-core + nai-storage
cargo test -p nai-upgrade-core --offline
cargo xtask check --profile portable
```

## 为什么还在 `nai-upgrade-core`

判据同源。A08 的验收句与 A09 的前提句是同一条不变量；把驻留账本拆到另一个 crate，会让同一条不变量有两个
真相，并且 `offline.rs` 还要跨 crate 复用 `verify_package`。放进 `storage`/`runtime` 的候选也都否决了
（storage 与 SQLite 无关，runtime 在 portable 下不可单独测）。详见 [ADR 0027](adr/0027-deployment-lifecycle-boundary.md)。

**零新增依赖**：依赖方向仍是 18 个包。shared 层不许依赖 plugin/runtime/app/tool，也不许引平台 SDK——
这一条在离线安装里是**承重**的（见下）。

## 驻留账本：声明式足迹，只在启用时物化

`ResourceKind` 是**封闭四族**，正是 A08/A09 原文列的四样：

| 族 | 名字表示 | 数量表示 | 排他 |
|---|---|---|---|
| `Process` | 进程名（`render-worker`） | 1 | **是**（同名进程只有一个） |
| `Port` | `tcp/8765` | 1 | **是**（二次绑定必然失败） |
| `Vram` | 模型 / 形象 id | MiB | 否（两个能力可各自加载同一模型） |
| `CloudRequest` | 端点引用 | 请求/分钟 | 否（可各自轮询同一 API） |

`ResidencyLedger` 对每个能力保存两份足迹：`declared`（`install` 时登记）与 `held`（`enable` 时物化）。

```
install(cap, declared)  →  held 仍为空        （机器和装之前一样安静）
enable(cap, now)        →  held = declared    （排他冲突则拒绝）
session_started(cap)    →  sessions += 1，空闲时钟清零
session_ended(cap, now) →  sessions -= 1，归零时起空闲时钟（不释放）
disable(cap)            →  held 清空（仍有会话服务时拒绝）
uninstall(cap)          →  条目移除（仍有会话服务时拒绝）
```

于是「未安装 / 未启用 ⇒ 零占用」是**结构性质**而非承诺：只有 `enable` 会写 `held`。
`held()` / `total(kind)` 给出当前占用，`residency_of(cap)` 给出 `Disabled | Serving{n} | Resident{since}`。

## 空闲回收：按 TTL，分族

空闲**不是立刻释放**：场次一结束就卸载模型，下一句要付一次完整重载。TTL 是分族数据（`IdlePolicy`），
方向由**重载代价**决定：

| 族 | 默认 TTL | 理由 |
|---|---|---|
| `Process` | 30 s | 临时 worker 重启很便宜，先走 |
| `Port` | 30 s | 与其进程同 TTL——「进程还在、端口先放手」不是可达状态 |
| `CloudRequest` | 60 s | 未用预览停止轮询 |
| `Vram` | 300 s | 模型重载最贵，留到最后 |

`reclaim(op, now)` 只释放 **TTL 已到期**的声明，`ReclaimReport` 给出逐能力释放明细与 `held_back`
（仍保留的声明数）。**正在服务会话的能力绝不被回收**；回收只动资源声明——不改启用状态、不删数据。

时间以参数进入（`enable`/`session_ended`/`reclaim` 都收 `now_ms`），核心内部不取墙上时钟——这是这些
判据能在 portable 下确定性测出来的前提。

## 驻留审计

`audit(now)` 是 `audit_residency(&self.snapshot(), &policy, now)` 的薄封装。做成对**公开快照值**
（`CapabilityResidency`）的**纯函数**有两个好处：宿主可以拿它审计**自己**的视图；每条规则都能被手工构造的
状态真实触发——一个只有误用才可达的检查，既没法测，也没法证明它活着。

| 标记 | 含义 | 可达性 |
|---|---|---|
| `idle resource past its ttl` | 已过 TTL 仍持有——**漏跑了一次回收** | 可达（真正的泄漏检测） |
| `disabled capability holds resources` | 禁用了却还占着 | 正确使用下不可达（改错时的守卫） |
| `enabled capability holds undeclared resource` | 持有从未声明的东西 | 同上 |
| `exclusive resource held twice` | 排他资源被两个启用能力同时持有 | 同上 |
| `idle capability has no idle clock` / `serving capability has an idle clock` | 时钟与会话数自相矛盾 | 同上 |

## 档位：一张迁移图

`Tier` 为 `Lite | Standard | Pro`，增量是常量集合（`STANDARD_ADDITIONS`、`PRO_ADDITIONS`）：

```
tier_plan(Lite,     Standard) → install = STANDARD_ADDITIONS,  remove = []
tier_plan(Standard, Pro)      → install = PRO_ADDITIONS,       remove = []
tier_plan(Lite,     Pro)      → install = 两段之并（不要求先到 Standard）
tier_plan(Pro,      Lite)     → remove = 两段之并, install = []
tier_plan(x, x)               → 双空
```

「不重装内核」是**数据的性质**（集合里不可能出现内核），不是一条需要有人记住的规则。

## 离线安装：缺件拒绝，且结构上不会抓取

`install_offline` 唯一的前置是「每个声明文件都在本地」；缺件以 `CAPABILITY_NOT_READY` 拒绝，
`missing_artifacts` 另行给出**是哪个文件**。随后复用**同一条**校验链：Schema → 内容集合 → 逐文件 SHA256
→ 签名 → 撤销。拒绝时不留暂存。

为什么缺件要单独前置断言：`verify_package` 对缺件报的是「missing package content」——对在线路径准确
（字节还没到），对离线路径不是运维需要的那句话（「这个包不可能离线装，因为还差一个文件」）。

「不会偷偷下载」的保证是**编译期**的：本 crate 没有 HTTP 客户端，要抓取得先加依赖。

## 部署迁移：三种拒绝先于搬动，三项复验不合并

```
plan_migration(current, request, map)
  ├─ target == current                  → INVALID_ARGUMENT（不是空操作成功）
  ├─ live_sessions > 0                  → CAPABILITY_NOT_READY（维护窗口）
  ├─ Distributed 且 sqlite_writers > 1  → CONFLICT（SQLite 不挂多节点写）
  └─ 某权限在目标形态无映射              → INVALID_ARGUMENT（不静默丢弃）
verify_migration(plan, imported)
  ├─ 权限集合 == 映射后期望值（顺序无关） → 否则 INVALID_ARGUMENT
  ├─ 证据版本 == 源                      → 否则 VERSION_INCOMPATIBLE
  └─ 数据哈希 == 源                      → 否则 INVALID_ARGUMENT
```

**错误码刻意分开**：哈希一致能证明字节没被改，但证明不了**权限被正确映射**——把 `admin.team` 悄悄降成
`viewer` 之后哈希照样一致。

`OutputSwitch::switch(key)` 以调用方给的 key 幂等（典型是 `session/epoch`）：第一次 `Announced`，
重复 `Duplicate`。key 由调用方给，因为「同一次切换」的身份只有调用方知道；核心只保证同一个 key
不会被宣告两次——即 §14.3 的「输出切换不重复播报」。

`RehearsalMigration` 是内存宿主：忠实导出/导入，并可用 `MigrationFault`
（`DroppedPermission` / `EvidenceVersionDrift` / `DataHashDrift`）重现「一次错误的导入长什么样」，
使三项复验可被证明真的会触发。

## 未做边界

- `Vram` 是**声明式索赔**，不是真实显存配额；真实 GPU 探测仍需宿主注入（与 `preflight` 探针一致）。
- **R06**（2h/8h/24h 长播无资源增长）与 **B04**（整链路延迟分位）需真实设备与长时运行，本轮不做。
- **A10**（云断连不伤本地链路）与 **A11**（Pro 多会话隔离）是独立主线，本轮不做。
- 真实轻云实例与 TLS（§27.4）属独立任务；真实 Windows 安装器、update-helper 与代码签名链（W15 遗留）不在范围内。
- A08/A09 的 `automation: manual` 未变，registry 只补证据并**不标 `tested`**。

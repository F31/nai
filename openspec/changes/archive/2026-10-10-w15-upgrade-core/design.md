## Context

W15 要补的是「升级器基础：插件包安装、签名与撤销、备份、journal」（V3.6 §31.2 W15 行）。规格给出了完整的事务式流程（§14.3）与撤销规则（§4.4/§11.6），但代码零实现，而 `registry` 有 **5 个**验收项压在这里（R05/R09/R10/O05，外加 S08 的撤销缺口）。

第一个要回答的问题：**这段逻辑放哪一层？**

| 方案 | 做法 | 判断 |
|---|---|---|
| A. 塞进 `nai-storage` | 升级就是「备份 + 迁移 + 写 journal」，全在 storage 里 | **否决**：会逼 storage 认识 `semver` 依赖闭包、签名、成本口径。storage 的职责是「可靠地存」，不是「决策能不能升」。而且 install/撤销/成本 与 SQLite 无关，塞进去会让 storage 的依赖面被撑大。 |
| B. 放 `nai-runtime` | 装配根来编排升级 | **否决**：runtime 属 **runtime** 层，`check-deps` 规定 it 不能依赖……更重要的是升级器一旦在 runtime，`xtask` 就无法在 portable 下单独测它（runtime 会拉起宿主/监督）。而且 `deps.rs` 早已把 `nai-upgrade-core` 预留在 **shared** 层。 |
| C. 新增 `crates/nai-upgrade-core`（shared） | 只依赖 contracts/storage/security + 纯算法依赖 | **采用**：与 `deps.rs` 的既有预留一致；shared 层禁止依赖 plugin/runtime/app/tool，天然保住「升级器不认识宿主」的边界；可在 portable 下单测。 |

## Goals / Non-Goals

**Goals**
- 把九步事务式流程落实为**可执行、可测试**的编排，且每步失败都**不产生半成品**。
- 撤销列表**真被检查**（安装 + 启动），且**过期的缓存不被当作「无撤销」**。
- 预检**不足即拒绝**，**未知即拒绝**——不伪报可用（R09 的「不伪报可用」是这条线的核心语义）。
- 崩溃恢复**幂等**：重复执行同一 journal 不叠加副作用。
- 回滚**不丢**升级后新增的事实与审计（导出而非丢弃）。
- 成本**逐项可核对**，未知项标注未知而非 0。

**Non-Goals**
- 不引入真实签名库（GPG/Ed25519/windows crate）、不引入下载库。
- 不实现 Windows 文件锁的独立 update-helper。
- 不改 `nai-storage` 的「只向前」迁移语义。
- 不升级 A08（档位升级不重装内核，R2/W25）。

## Decisions

### D1 升级器是 shared 层的业务内核，接缝全部可注入

`nai-upgrade-core` 只持有**判定逻辑**，所有「世界状态」都从 trait 进来：

| 接缝 | 为什么必须可注入 |
|---|---|
| `SignatureVerifier` | 真实实现要 Ed25519/代码签名（新依赖）；测试要确定性、无密钥。注入让两者都不污染对方。 |
| `RevocationSource` | 撤销列表来自信任库/远端缓存，来源是运营配置，不是编译期常量。 |
| `SpaceProbe` | 磁盘空闲量的取得是平台细节（Windows `GetDiskFreeSpaceEx`、Unix `statvfs`）；把平台调用留在探针实现里，内核只消费「够 / 不够 / 未知」三态。 |
| `ActiveSessionGate` | 「有没有活动会话」的唯一真相在 kernel/装配根，不由升级器自己维护状态。 |

理由：若把其中任一条写死，升级器就会**绑定一个具体环境**——正是「不要硬编码和绑定太多依赖」要避免的。默认实现只用 `std`（`SpaceProbe::Unknown` 作为无平台探针时的默认，配合「未知即拒绝」），测试用确定性替身。

### D2 签名验证：接缝 + 确定性替身，并**显式标注不是真实签名链**

本轮**不引依赖**，因此不实现 Ed25519。取而代之：

- `trait SignatureVerifier { fn verify(&self, subject: &[u8], signature: &[u8], signer_ref: &str) -> Result<(), UpgradeError> }`
- 测试与 portable 用 `DeterministicVerifier`（基于 `sha2` 的**演练替身**，只证明「校验被调用、失败会拦住暂存」这一事实）。
- 真实非对称验证作为**注入点**留给后续 update-helper；本轮在文档/报告/registry 里**明确写「未接真实签名链」**，不把替身当认证。

这是刻意的取舍：**宁可显式承认未做，也不引一个半吊子签名实现冒充安全边界**。项目里 provider 的 rehearsal 桩、模型准入的「完整性不等于认证」都是同一原则。

### D3 撤销列表是**带期限的可信缓存**，不是 `HashSet`

若把撤销列表实现成一个集合，会漏掉规格里最要命的一句：「撤销列表与离线可信缓存**有期限**」（§11.6）。所以：

- `RevocationList { issued_at, expires_at, entries }`，条目为 `(subject_ref, revoked_at)`；
- `check(subject, now)` 的行为是**三态**：命中 → 拒绝；未命中且**未过期** → 放行；**已过期** → **拒绝**（`RevocationUnavailable`），不是放行。
- `check_startup` 是独立入口，与安装路径分开——规格要求「安装**与每次启动**」。

过期即拒绝的理由：一个过期的撤销缓存意味着「我们不知道现在有没有被撤销」。把它当「无撤销」放行，正好让**撤销机制在最需要它的时候静默失效**。

### D4 journal 用既有表，五状态 + 幂等恢复

`upgrade_journal` 表已在 `0001-foundation.sql` 里：`(transaction_id PK, from_manifest, to_manifest, step, backup_ref, error_code)`。事务**不是**把 9 步塞进一个 SQLite 事务——备份与迁移发生在数据库**之外**（文件系统），跨介质无法回滚。所以：

- `step` 逐状态 `UPSERT`（单条事务记录原地推进，而不是追加日志）：`prepared → migrated → checked → committed`，失败置 `rolled_back` 并记 `error_code`；
- 恢复时读**当前 step**决定动作：`prepared`（还没迁移）→ 删暂存、原样；`migrated`（迁移完未自检）→ 用 `backup_ref` 恢复；`checked`（自检通过未提交）→ 可继续提交或回滚；`committed` → 完成态，重入直接返回成功。
- 幂等靠「step 单调前进」：已完成的状态**不重复执行**。回滚把 `step` 置 `rolled_back` 并**保留**记录（审计需要知道曾发生过什么）。

### D5 预检：三态资源，未知即拒绝

每个资源检查返回 `Sufficient | Insufficient(reason) | Unknown(reason)`。`Unknown` 与 `Insufficient` **同等拒绝**，只是错误码与文案不同。这条直接来自 R09 的措辞「不伪报可用」——「不知道够不够」离「够」很远，把它渲染成「够」就是伪报。

「双份磁盘空间」按 §14.3 的字面意思：下载暂存 + 迁移后的新数据目录**同时存在**，所以需要 `install_bytes + migrated_bytes` 的余量，不是二选一。

### D6 成本：未知 ≠ 0

`CostEstimate` 的每个分项是 `Known(amount, unit) | Unknown(reason)`。用 0 表示未知会被读成「这块免费」，而「免费」是一个**可被核对的断言**——我们给不出。所以未知显式标注，UI 才有义务显示「待定」。O05 的「准确列示」在这一层的含义就是：**能算的逐项列出，不能算的不冒充能算**。

### D7 原子替换版本清单

「原子替换当前版本清单」落在文件系统上：写 `manifest.new` → `flush_durable`（复用 storage 里那条 Windows 下**必须可写句柄**的教训，见 ADR 0024）→ 改名覆盖 `manifest.current`。同目录改名在 Windows 与 Unix 上都是原子的，不需要 rename-into-place 之外的机制。

## Risks / Trade-offs

- **`DeterministicVerifier` 可能被误读为已签名**：文档、报告、registry 三处都写明「非真实签名链」；`SignatureVerifier` 的实现类型名里带 `Deterministic`，且 `lib.rs` 文档注释声明其仅用于可移植验证。
- **`SpaceProbe` 默认未知 ⇒ portable 下永不激活**：这是**特性不是缺陷**——portable 没有真实安装目标，本来就不该激活。「未知即拒绝」让默认行为天然安全。
- **journal 单记录原地推进**：崩溃在两个 step 之间会留下「落后一步」的状态，恢复逻辑按**当前 step**容错（保守回滚），不会出现「computed 但未记录」。代价是无法从 journal 复原完整历史链，需要历史时靠 `audit_records`。
- **`semver` 闭包解析只做交集判断**，不实现完整 SAT 求解：包清单声明的是单层 `requires`，交集足够；真正的多包冲突留给后续（本轮显式记为未覆盖）。

## Migration Plan

无数据迁移。新表不新增（复用 `upgrade_journal`）。新 capability 对既有代码零改动——`nai-upgrade-core` 是新 crate，未被任何 app 依赖，接入装配根是后续工作项。

## Validation

```
source /c/Users/32791/.nai-msvc-env.sh
export CARGO_TARGET_DIR=E:/nai-build-target
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4 cargo run -p nai-xtask --locked --offline -- check --profile portable
```

补充：`cargo test -p nai-upgrade-core` 全绿；`cargo xtask test --suite upgrade`。`cargo xtask check-deps` 必须仍通过（新 crate 的 layer 与依赖面）。

环境边界：本机 Windows MSVC 宿主（Rust 1.96.0）。`CARGO_BUILD_JOBS` / `RUST_TEST_THREADS` 是 cargo/libtest 原生开关，规避宿主提交限制，**不写进** portable 检查。本轮不需要网络。

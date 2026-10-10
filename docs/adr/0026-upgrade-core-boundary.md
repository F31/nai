# ADR 0026：升级器的分层与判定边界——shared 层业务内核、写前 journal、三态资源、带期限撤销

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.6 §14.3（升级与回滚流程）、§4.4（签名与撤销）、§11.6（离线可信缓存期限）与 §31 工作项 W15；承接 ADR 0015（基线与验收注册表）、ADR 0024（工程校验回路）
- 变更：`openspec/changes/w15-upgrade-core/`
- 判据：`tests/registry.yaml` R05「升级原子与回滚」、R09「升级预检与下载失败」、R10「开播与回滚」、O05「升级成本显示」，以及 S08「插件与供应链」的撤销列表缺口

## 背景

W15 是「升级器基础：插件包安装、签名与撤销、备份、journal」。规格给了完整的九步事务式流程（§14.3）、
撤销规则（§4.4、§11.6）与「只在无活动会话时激活、原子可恢复、回滚不丢新写入」（INV-16），但代码零实现，
而 `tests/registry.yaml` 有 **5 个**验收项压在这一条线上。

有四个问题必须先回答：这段逻辑**放哪一层**；升级靠什么**崩溃可恢复**；「资源不足」与「资源未知」
**是否同等对待**；撤销列表**是不是一个集合**。

## 决策

### D1 新增 `crates/nai-upgrade-core`，属 **shared** 层——三个候选都被否决

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 塞进 `nai-storage` | 升级就是「备份 + 迁移 + 写 journal」，全在 storage 里 | **否决**：会逼 storage 认识 `semver` 依赖闭包、签名验证与成本口径。storage 的职责是「可靠地存」，不是「决策能不能升」；install/撤销/成本 与 SQLite 无关，塞进去只会撑大它的依赖面。 |
| B. 放 `nai-runtime` | 装配根来编排升级 | **否决**：runtime 属 runtime 层，`check-deps` 规定其不能这样依赖；更关键是升级器一旦在 runtime，`xtask` 就无法在 portable 下单独测它（runtime 会拉起宿主与监督）。 |
| C. 新增 `crates/nai-upgrade-core`（shared） | 只依赖 contracts/storage/security + 纯算法依赖（`semver`/`sha2`/`serde_json`/`jsonschema`） | **采用**：与 `xtask/src/deps.rs` 的既有预留一致；shared 层禁止依赖 plugin/runtime/app/tool，天然保住「升级器不认识宿主」的边界；可在 portable 下单测。 |

`UpgradeHost`（世界效果）与 `UpgradeJournal`（推进记录）两个接缝把事务抬离文件系统、平台与会话注册表——
真实安装器是后续的**注入**，不是重写。

### D2 journal **写前记录**：先记 step，再执行对应效果

崩溃因此只留下「至多领先一步」的记录，恢复永远**向后收敛**。

**否决**的替代方案是「效果执行后再记录」：迁移中途崩溃会留下 `prepared` 记录压在半迁移的目录树上——
那是唯一无法安全恢复的状态（既不知道迁移到了哪，也没有可回滚的确认点）。

五状态 `prepared → migrated → checked → committed`，失败置 `rolled_back` 并保留记录（审计需要知道发生过什么），
单条记录**原地 `UPSERT`**、`step` 单调推进。**否决**「把 9 步塞进一个 SQLite 事务」：备份与迁移发生在数据库
**之外**（文件系统），跨介质无法回滚。

`checked` 恢复时比较 `current_manifest` 与记录的 `to_manifest`：已落地即确认 `committed`，未落地才回滚。
这样消除了「换清单成功但 journal 还没写」的歧义窗口，不需要靠猜。

### D3 资源是**三态**，`Unknown` 与 `Insufficient` **同等拒绝**

每个检查答 `Sufficient | Insufficient(reason) | Unknown(reason)`，只以错误码区分
（`CAPABILITY_NOT_READY` / `RESOURCE_EXHAUSTED`）。**否决**把未知当乐观：「不知道够不够」离「够」很远，
把它渲染成「够」就是 R09 明令禁止的「伪报可用」。

这条也决定了默认行为的安全方向：`NoSpaceProbe`（无平台探针时的默认）永远返回「未知」⇒ 拒绝，
所以 portable 环境下升级**永不激活**——这是特性，不是缺陷。

磁盘按 §14.3 字面要**双份**（`install_bytes + migrated_bytes`，暂存与新数据目录同时存在），不是二选一。

### D4 撤销列表是**带期限的可信缓存**，不是一个集合——「做成集合」被否决

`RevocationList { issued_at, expires_at, entries }`，判定为三态：命中拒绝；未命中且未过期放行；
**已过期拒绝**（不是放行）。

理由：把撤销列表实现成 `HashSet` 会漏掉 §11.6 最要紧的一句——离线可信缓存**有期限**。一个过期的缓存
意味着「不知道现在有没有被撤销」；把它当「无撤销」放行，正好让撤销机制**在最需要它的时候静默失效**。
安装（`check_install`）与启动（`check_startup`）是两个独立入口，因为规格要求「安装**与每次启动**」。

### D5 门控与激活**拆成两次调用**——会话事实归调用方

`prepare`（暂存）与 `activate`（换清单）分开。开播中可暂存不可激活；`SessionGate` 是**独立于** `activate`
的一次调用，升级核心不持有会话状态。**否决**把会话检查内建进 `activate`：那会让升级器维护第二份、
可能过期的会话真相，而「用不再为真的数字去放行一次激活」比不门控更糟。

`activation` 返回 `Allowed | Deferred { active }` 而非 `bool`——为什么延后是答案的一部分。被门控的包
**不失效**，会话结束即用同一次暂存激活。

降档**先排空、后停用**：`retire` 只在 `drain` 报告剩余为 0 后调用；排空不彻底即停止，不把会话搁浅在
已经不存在的东西上。`retire` 契约禁止触碰存储数据——降档退掉的是代码，不是事实。目标超出 `rollback_range`
时以 `VERSION_INCOMPATIBLE` 拒绝静默回滚：不兼容是拒绝，不是可以推翻的偏好。

### D6 成本**未知 ≠ 0**

`CostEstimate` 的分项是 `Known(amount) | Unknown(reason)`。**否决**用 0 表示未知：0 是「这块免费」这样一个
**可被核对的断言**，我们给不出。`total()` 因此是 `Complete(sum)` 或 `Partial { floor, unknown }`——
把部分和当作总额，就是分项层面已经避免的同一个「不伪报」违规上升了一层。

## 后果

**正面**
- 5 个验收项（R05/R09/R10/O05）与 S08 的撤销缺口都有**可执行判据**，且全部在 portable 下可测。
- 接缝全部可注入 ⇒ 真实签名链、Windows 安装器、真实下载器都是**注入**而非重写；升级器零宿主依赖。
- 「未知即拒绝」「过期即拒绝」「未知≠0」三条共享同一个「不伪报」原则，语义一致。

**代价 / 边界**
- **没有真实签名链**：`DeterministicVerifier` 是基于 `sha2` 的演练替身，`lib.rs` 文档、本 ADR、报告、
  registry 四处都写明「非真实签名链」，不把替身当认证。
- **没有真实安装器**：`RehearsalHost` 不持有字节、不落盘；真实 Windows 安装是后续注入。
- **journal 单记录原地推进**无法复原完整历史链，需要历史时靠 `audit_records`。
- **`semver` 只做交集判断**，不实现完整 SAT 求解：单层 `requires` 足够，多包冲突本轮显式记为未覆盖。
- `nai-upgrade-core` 尚未被任何 app 依赖，接入装配根是后续工作项。

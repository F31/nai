# ADR 0120：作业状态的第三端 —— 谁写这行、谁推进它、以及「能轮询」的代价

- 状态：已接受（W146，2026-10-18）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§30.1 / §31.2 作业面）
- 相关：ADR 0118（作业面闭世界）、ADR 0119（`blockedBy` 由路由器作证）、
  `scripts/contracts/job-surface.mjs`、`tests/job-surface.json`、
  `crates/nai-storage/src/upgrade.rs`、`crates/nai-upgrade-core/src/{plan,rehearsal}.rs`、
  `crates/nai-runtime/src/upgrade.rs`、`apps/engine/src/lib.rs`

## 背景

W144 收「每条 202 都在册、每册项恰有 `kind` 或具名 `blockedBy`」；W145 收「分类 ⇔ 路由器怎么处理」。
两轮都在 `202` 那一侧。`kind: upgrade` 断言的是另一件事：**这个 id 能被 `GET /jobs/{id}` 轮询**。
支撑它的判据只有两条锚点：`pub struct UpgradeRecord` 在、`"kind": "upgrade"` 在被打印。

本轮把这条链路逐跳量了一遍：

| 跳 | 落点 | 实测 |
| --- | --- | --- |
| 读取面 | `JobBackend::record` → `store.upgrade_record` | 生产装配根 `apps/engine/src/lib.rs:338` **真的** `with_jobs` |
| 表 | `upgrade_journal` | 迁移 `0001-foundation.sql:33` 建表 |
| 写入 | `record_upgrade` 的 `INSERT INTO upgrade_journal` | 在已跟踪 `.rs`+`.sql` 里**恰 1 处** |
| journal trait | `UpgradeJournal::record` | `plan.rs:200` |
| 执行体 | `UpgradeTransaction` | `plan.rs:226`，入参是 `&mut dyn UpgradeHost` |
| 装配入口 | `Composition::stage_upgrade` / `activate_staged` | `runtime/src/upgrade.rs:57` / `:75` |
| 调用方 | — | **全部在 `crates/nai-runtime/tests/upgrade.rs`，生产零调用点** |
| 宿主 | `impl UpgradeHost for` | **恰 1 处：`RehearsalHost`（演习替身）** |

结论：**读面接了，写面没有任何生产入口**。`/jobs/{id}` 的 404 分支可达、200 分支不可达。
这与「`JobAccepted` 零生产者」不是两处缺口，是同一个缺席。

> 勘定过程中有一次自我订正：首次只扫 `crates/` 得到「`JobBackend` 只在 tests 构造」，
> 扫 `apps/` 之后结论翻转——读面在 `apps/engine` 里是接上的。**扫描域就是结论的边界**，
> 这条订正已写进报告与记忆。

## 决定

### 决定 1：写入点用「逐字语句 + 计数」判，不用「代码里只有一处」这种信念

`JOB_KINDS[kind].writer` 记三样：文件、逐字锚点、**逐字写入语句**。判据在已跟踪的 `.rs`+`.sql`
里数这个语句：0 ⇒ 该 kind 永远没有行；≥2 ⇒ 有第二条写入路径。「恰一次且在被声明的文件里」
这句话可以被判，而「我知道只有一处」不可以。

选「语句」而不是「函数名」：语句是那次写入的**身份**，函数改名不影响它；而函数名（尤其只取前缀）
会被同名前缀的函数满足——本轮注入 I2 实测：`pub fn record_upgrade` 这个锚点在函数改名成
`record_upgrade_w146_probe` 后仍然命中。**锚点必须是判别性的**（W144 对 `"kind": "upgrade"`
已立过同一条），所以写入锚点最终取 `record_upgrade(&mut self`（带调用形态），并在用例里加了一条
结构断言把这次注入的结论留在仓库里。

### 决定 2：安装器实现者用**名册双向**，`production` 是布尔而不是名字推断

`JOB_HOSTS` 每行只答两个问题：它在不在、它是不是生产实现。判据扫 `impl UpgradeHost for X` 得到
集合，与名册键集**双向**比较。单向不够：只判「名册里的都有实现」会漏掉「仓库里多了实现而名册没写」，
而那正是本轮要防的那件事（安装器落地而登记表不动）。

`production` MUST 是布尔，MUST NOT 用「名字里没有 Rehearsal」推断：那种推断在第一个叫
`PlatformHost` 的演习替身出现时就错了，而且错得没有信号。

### 决定 3：成对规则把「下一跳要同时做到什么」写死

「声称 `kind` 的作业面 ⇔ 存在生产安装器」双向成立。今天两侧都是 0，规则**如实空转**——
它的价值不在今天，在于**下一个想实现作业的人**：他必须同时做到 (a) 某条在册项从 `blockedBy`
改成 `kind`、(b) 一个 `production: true` 的安装器落地。任一侧单独动，判据立刻红并说出缺的是哪一半。

这也正是 W145 §八第 1 条（「`kind` 的正面路径仍需一次真实作业」）在没有真实安装器时能得到的最好收口：
**不实现功能，但把实现它的全部前置条件变成判据。**

### 决定 4：判据函数的入参改成具名对象

`jobSourceErrors` 从四个位置参数变成六项具名对象。它不是风格问题：这一轮之后它同时喂
状态来源、拒绝理由、安装器名册、契约侧登记表四张表，位置参数会变成「第四个是什么来着」——
而**按错位参数判出来的绿比没有判据更坏**。六项全必需，缺一项即具名失败（`undefined` 与空集
MUST NOT 折叠）。

## 被否决的方案

1. **实现一个真实作业**（例如给 `post /backups` 接上真正的备份执行体）来让 `kind` 有正向样本。
   否决：那需要一个真实执行体与可轮询状态，而它们正是缺的东西；造一个「铸 id 而没人写行」的
   202 就是**伪造可观测面**（`control.yaml` 对 `upgrades/execute` 已经写明这句话）。
2. **把「执行入口零生产调用点」也做成判据**。否决：源码扫描在这里不可靠——注释、同文件调用、
   `tests/` 之外的测试 harness 都会命中，而排除规则一旦写错就把真实调用点排除掉。**登记为残余**，
   不造一条会被自己的文本搅乱的判据。
3. **只判单向名册**（名册 ⇒ 有实现）。否决：漏掉的正好是「仓库多了实现而名册没动」。
4. **用名字推断 `production`**。否决：见决定 2。

## 残余（具名，本轮不做）

1. **执行入口零生产调用点没有判据**：`stage_upgrade`／`activate_staged` 今天只被
   `nai-runtime/tests/upgrade.rs` 调用，这件事是**勘定结论**，不是判据结论。
2. **`job_view_json` 里的 `"kind": "upgrade"` 是硬编码字面量**（`lib.rs:1351`，`JobView` 结构体
   没有 `kind` 字段）。第二个 kind 落地时它必须变成字段，今天只由「契约枚举 ⇔ `JOB_KINDS` 双向」
   间接守，那一行本身没有判据。
3. **成对规则在真实仓库两侧空转**（0 条 `kind`、0 个生产安装器）：判别力由合成世界的两个反例证明，
   MUST NOT 记作「已由真实仓库验证」。
4. **扫描域只看 `.rs` 与 `.sql`**：另一个语言写的安装器实现不会被看见。扩展名表是显式的
   （`WRITE_SCAN_EXTENSIONS`），所以这是一条看得见的边界，不是盲区。

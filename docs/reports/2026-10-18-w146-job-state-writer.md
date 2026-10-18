# W146 报告：作业状态的第三端 —— 谁写这行、谁推进它

- 日期：2026-10-18
- 变更：`openspec/changes/w146-job-state-writer/`（归档后 `openspec/changes/archive/2026-10-18-w146-job-state-writer/`）
- ADR：[0120](../adr/0120-job-state-writer.md)
- 起点：W145 报告 §八第 1 条（「`kind` 的正面路径仍需一次真实作业」）

## 一、勘定与现测

W144 收的是「每条 202 都在册、在册项恰有 `kind` 或具名 `blockedBy`」；W145 收的是「分类 ⇔ 路由器
怎么处理」。两轮都在 **202 那一侧**。`kind: upgrade` 断言的是另一件事——**这个 id 能被
`GET /jobs/{id}` 轮询**——而支撑它的判据只有两条锚点：`pub struct UpgradeRecord` 在、
`"kind": "upgrade"` 在被打印。**没有一个判据问过谁写那一行。**

于是本轮先把整条链逐跳量了一遍：

| 跳 | 落点 | 实测 |
| --- | --- | --- |
| 读取面 | `JobBackend::record` → `store.upgrade_record` | 生产装配根 `apps/engine/src/lib.rs:338` **真的** `JobBackend::new` + `.with_jobs` |
| 表 | `upgrade_journal` | 建表在 `migrations/sqlite/0001-foundation.sql:33` |
| 写入 | `record_upgrade` 的 `INSERT INTO upgrade_journal` | 在已跟踪 `.rs`+`.sql` 里**恰 1 处**（`crates/nai-storage/src/upgrade.rs:134`） |
| journal trait | `UpgradeJournal::record` | `crates/nai-upgrade-core/src/plan.rs:200` |
| 执行体 | `UpgradeTransaction` | `plan.rs:226`，入参 `&mut dyn UpgradeHost` |
| 装配入口 | `Composition::stage_upgrade` / `activate_staged` | `crates/nai-runtime/src/upgrade.rs:57` / `:75` |
| 调用方 | — | **全部在 `crates/nai-runtime/tests/upgrade.rs`（8 处），生产零调用点** |
| 宿主 | `impl UpgradeHost for` | **恰 1 处：`RehearsalHost`**（`rehearsal.rs:153`，演习替身） |

三个数字：**1** 处写入、**1** 个实现者（且是替身）、**0** 个生产调用点。

**一次自我订正**：首次只扫 `crates/` 时得到「`JobBackend` 只在 tests 里构造（`tests.rs:1574`）」，
据此差点把「读面也没接」写进报告。再扫 `apps/` 之后结论翻转——`apps/engine/src/lib.rs:338`
就是生产装配根。**扫描域就是结论的边界**，这条订正留在 §五第 1 条与记忆里。

## 二、缺的是哪一端：读面接了，写面没有

| | 状态 | 证据 |
| --- | --- | --- |
| 读面（`GET /jobs/{id}` → `JobView`） | **已接** | `apps/engine` 注入 `JobBackend`；未注入时 503、无行时 404、有行时 200（W138 三条用例） |
| 写面（`upgrade_journal` 的行） | **没有生产入口** | 执行入口零生产调用点 + 宿主零生产实现 |

所以 `/jobs/{id}` 的 **404 分支可达、200 分支不可达**——而且不是「忘了接线」，是**没有东西可接**。
这与 W144／W145 反复写到的「`JobAccepted` 零生产者」不是两处缺口，是**同一个缺席**：
平台安装器（`UpgradeHost` 的生产实现）。

本轮**不实现安装器**（真值缺口不在仓库内），只把这条链上三件「此前只是散文」的事变成可判的。

## 三、三条判据，各自为什么这样写

### 1. 写入点：逐字语句 + 计数（`JOB_KINDS[kind].writer`）

`writer` 记三样：文件、逐字锚点、**逐字写入语句**。判据在已跟踪 `.rs`+`.sql` 里数那个语句：

- **0** ⇒ 该 kind 的 journal 永远收不到行（它的每个 `/jobs/{id}` 都是构造性的 404）；
- **≥2** ⇒ 有第二条写入路径绕过 journal，「谁写了这一行」不再只有一个答案；
- 唯一那一处 **不在声明的文件里** ⇒ 声明指向一个不写的文件。

选「语句」不选「函数名」：语句是那次写入的身份，改名不影响它。而**锚点必须是判别性的**——
见 §四 I2。

### 2. 安装器名册：双向 + `production` 是布尔（`JOB_HOSTS`）

扫 `impl UpgradeHost for X` 得到集合，与名册键集**双向**比较。单向不够：只判「名册里的都有实现」
会漏掉「仓库里多了实现而名册没写」，而那正是本轮要防的那件事。`production` MUST 是布尔，
MUST NOT 用「名字里没有 Rehearsal」推断——那种推断在第一个叫 `PlatformHost` 的替身出现时就错了。

### 3. 成对：能轮询的作业面 ⇔ 生产安装器

登记表任一作业面声称 `kind` ⇒ 名册里至少一个 `production: true`；反之亦然。今天两侧都是 0，
规则**如实空转**——它的价值在下一跳：想实现作业的人必须同时做到（a）某条在册项从 `blockedBy`
改成 `kind`、（b）一个生产安装器落地。任一侧单独动，立刻红并说出缺的是哪一半。

## 四、判据与注入

五条注入全部真改文件、真跑、`sha256` 逐字节还原（4 个目标文件全部 `OK`）：

| 注入 | 改动 | 结果 |
| --- | --- | --- |
| I1 | 在 `crates/nai-storage/src/tests.rs` 放第二处 `INSERT INTO upgrade_journal` | 红：`appears 2 times (…tests.rs, …upgrade.rs)`（3 条用例红——它们共用同一条判据） |
| I2 | `record_upgrade` 改名成 `record_upgrade_w146_probe` | **首轮没红** → 修锚点 → 复测红：`JOB_KINDS[upgrade].writer anchors on "record_upgrade(&mut self" … no longer there` |
| I3 | 名册键 `RehearsalHost` → `RehearsalHostX` | 红，**双向各一条**：`no tracked file contains "impl UpgradeHost for RehearsalHostX"` + `RehearsalHost implements UpgradeHost … but JOB_HOSTS does not declare it` |
| I4 | `RehearsalHost.production` 改 `true` | 红：`JOB_HOSTS declares production host(s) RehearsalHost but no job surface claims a kind` |
| I5 | 登记表把 `post /knowledge/import` 改成 `{"kind": "upgrade"}` | 红：`1 job surface(s) claim a kind (…post /knowledge/import) but JOB_HOSTS declares no production host` |

**I2 是本轮最要紧的一条——它抓到了判据自己的缺陷**：`writer.text` 原先是 `pub fn record_upgrade`，
而那是**前缀形态**：函数改名成 `record_upgrade_v2` 之后这句话仍然是子串，锚点照旧命中，判据继续绿。
这正是 W144 对 `"kind": "upgrade"` 立过的同一条纪律（锚点必须判别性），本轮换成写入函数时又犯了一次。
处置：锚点改成 `record_upgrade(&mut self`（带调用形态），并在真实仓库用例里加一条结构断言
（`writer.text` MUST 含 `(`），把这次注入的结论留在仓库里——否则下次有人「简化」回前缀不会有信号。

## 五、诚实边界

1. **扫描域就是结论的边界。** 首次只扫 `crates/` 得出「读面也没接」，扫 `apps/` 之后翻转。
   这条订正不是笔误，是方法：任何一次「某处没有调用方」的结论都必须先声明扫描域。
2. **执行入口零生产调用点仍然不是判据。** 它是勘定结论（附实测命令与结果），不是判据结论：
   源码扫描在这里会被注释、同文件调用、测试 harness 搅乱，**造一条会被自己文本搅乱的判据比不造更糟**。
   ADR 0120 残余第 1 条。
3. **成对规则今天两侧都空转。** 判别力由合成世界的两个反例（I4／I5）证明，MUST NOT 记作
   「已由真实仓库验证」。
4. **`job_view_json` 里 `"kind": "upgrade"` 是硬编码**（`lib.rs:1351`；`JobView` 结构体没有 `kind`
   字段）。第二个 kind 落地时它必须变成字段——那一行本身今天没有判据，只由「契约枚举 ⇔ `JOB_KINDS`
   双向」间接守。ADR 0120 残余第 2 条。
5. **`JobAccepted` 仍然零生产者。** 本轮一个 202 都没接上真实实现；这条事实与 W144／W145 一致。

## 六、门控

`cargo xtask check --profile full` → **GATE_EXIT=0**（第 **5** 次尝试；`CARGO_INCREMENTAL=0
CARGO_BUILD_JOBS=1`，`PROTOC` 指向 3.21.12）。

| 项 | 实测 |
| --- | --- |
| `cargo fmt --all --check` | 通过（门控内执行；本轮 Rust 零改动） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过，无告警 |
| `cargo test --workspace --locked --exclude nai-xtask` | `test result:` 行 **102** 条，合计 **1147 passed / 0 failed**（与 W145 持平——本轮没有新增 Rust 用例） |
| Node 侧 15 组 | 合计 **374 passed / 0 failed**（W145 的 372 + 本轮 2 条用例）；其中 `tests/contracts/schema.test.mjs` **37 / 37** |
| `openspec validate --all --strict` | **43 / 0**（归档后；归档前 44 / 0，含 `change/w146-job-state-writer`） |
| `openspec validate --all --strict --archived` | **143 / 0**（含 `2026-10-18-w146-job-state-writer`） |

**前四次都是 EXIT=1，且都不是判据在响**：全是链接期 `LNK1104: 无法打开文件`，而且**目标每次不同**：

| 尝试 | 失败目标 | 处置 |
| --- | --- | --- |
| 1 | `deps\pipeline-2f68e59025fded98.exe`（`nai-engine` test） | 直跑 → 目标文件不存在、`tasklist` 无残留进程 ⇒ 重跑 |
| 2 | `deps\engine-c10401799740653b.exe` | 目标漂移 ⇒ 按纪律串行预构建 |
| 3 | `deps\nai_engine-561969003593c8e0.exe`（bin 的 test） | 预构建后**仍红** ⇒ 见下 |
| 4 | `deps\nai_engine-40fe7a3b8d5f1866.exe` | 有界重试 |
| 5 | — | **EXIT=0** |

预构建本身是绿的（`cargo build --workspace --all-targets --locked` → `EXIT=0`、`LNK1104` **0** 次），
却没能救下一轮门控。原因不在「没预构建」：门控自己第一步跑 `clippy`，**clippy 与 rustc 的产物指纹
不同**，于是 `cargo test` 阶段这些目标全部重新链接——预构建的产物在门控内部就被作废了。
这条让「预构建再跑」这个既有招式在本仓库**只在不带 clippy 的回路里有效**，已登记（§八 第 5 条）。

判它环境瞬态而不是代码缺陷，四条：①错误类同族（链接器打不开一个新产物）；②**失败目标每次都不同**
（不是某个目标稳定坏掉）；③本轮 Rust 零改动；④同参数重跑即绿——不是「跑一次就绿」，是连续四次
红之后第五次绿，且中间**没改一个字节**。处置：未改代码、未放宽判据。

## 七、CI 观测

_（回填）_

## 八、具名登记（本轮发现但不做）

1. **执行入口零生产调用点没有可靠判据形态**（ADR 0120 残余 1）：`stage_upgrade`／`activate_staged`
   今天只被 `nai-runtime/tests/upgrade.rs` 调用。要判它，先解决「扫描域与注释/同文件调用的区分」。
2. **`job_view_json` 的 `kind` 字面量**（残余 2）：第二个 kind 落地时它必须变成字段。
3. **扫描域只看 `.rs` 与 `.sql`**（残余 4）：另一个语言写的安装器实现不会被看见。扩展名表是显式的
   （`WRITE_SCAN_EXTENSIONS`），所以这是看得见的边界。
4. **W145 §八其余 4 条仍未接手**：`tests/job-surface.json` 没有 schema、类别表与执行侧期望表靠名字
   耦合、W143 §八 4 项、`BRINGUP_OPERATION` 在 CI windows 上偶发不足（一次样本）。
5. **「预构建再跑门控」在本仓库只在不带 clippy 的回路里有效。** 门控第一步跑 `clippy`，其产物指纹与
   rustc 不同 ⇒ `cargo test` 阶段重新链接，预构建成果被作废。本轮实测：预构建 `EXIT=0`、`LNK1104`
   0 次，紧随其后的门控仍红在链接。要真正消掉它得让 clippy 与 test 共用同一份产物（例如
   `cargo clippy` 后不重建），或把 `-D warnings` 移到检出的独立步骤——两者都会改门控形状，
   属于下一轮的范围，**本轮只登记**。

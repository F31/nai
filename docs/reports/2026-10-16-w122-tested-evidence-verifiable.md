# W122 报告：`tested` 的证据可核验（registry 状态回填第一批）

日期：2026-10-16｜归属：首轮「从改 registry 状态入手」的验收推进｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W121 之前 9 轮（W113–W121）产出集中在判定面守卫，`git log W113..HEAD -- tests/registry.yaml` **为空**
⇒ 判据密度上升但**验收状态零推进**。转入「改 registry 状态」后第一步要回答：升级凭**什么**。

**立项判据（实测）**：`admission-registry` 声明「`tested` 仅可用于已有归档实测证据的项目，且证据 MUST 指向真实报告
与测试计数」，但 `tests/registry.test.mjs` 中 `grep evidence` **只命中三处 `landing` 内容锚点（W93）**——
`acceptance[].evidence` **零判据**。⇒ `evidence` 可写空 / 写不存在路径 / 指向**工作区存在但未跟踪**的文件，
**门控全绿**，`tested` 退化为无法复核的声明。

## 二、判据落点（`tests/registry.test.mjs`，52 → 56）

遍历 registry **自己的键**、覆盖**全部在册 `tested` 项**（不扫手写清单——清单的盲区恰是它的补集）：

- **新增 1**：`every tested acceptance carries a report that exists, is tracked, and lives under reports/`
  —— 三方向一起判：非空 → 落在 `reports/`（相对 `docs/`）→ 是仓库成员 → 磁盘存在。**判定顺序**
  刻意从「形状」到「仓库成员」再到「磁盘」，且**先判 tracked 再判磁盘**（反序则断言随本机磁盘状态变化，违反铁律二）。
- **新增 2**：`the evidence-carrier scan is closed over the whole tested set` —— 闭世界：扫描数 == `tested` 总数
  （不静默跳过）+ id 唯一 + 非空洞下限 `W122_TESTED_FLOOR = 11`（A13 升级后实测）。
- **新增 3**：`guard: a tested acceptance with an empty evidence field is rejected` —— 注入空 `evidence` ⇒ `PROBE-EMPTY:<empty>`。
- **新增 4**：`guard: evidence outside reports/, or untracked, is rejected` —— 注入 `docs/knowledge.md` ⇒
  `not-under-reports`；注入 `reports/2026-10-16-w122-not-a-real-report.md` ⇒ `not-tracked`。

**实测教训（设计推测之外）**：首跑 **11/11 报 `not-tracked` 假阳性** —— registry 的 `evidence` 是**相对 `docs/`**
的引用（与 `development-plan.md` 的相对报告链接同约定），而 `.git/index` 派生集是**仓库根**相对路径。
判据先落 `EVIDENCE_BASE = 'docs/'` 再判归属后即 **56/56**。这条基准若靠推测写，会得到一个恒假阳性或恒真的守卫。

## 三、R1 15 条 `implemented` 的逐条判定（本轮产出，实测）

| 项 | 标题 | 缺口类别 | 本轮处置 |
|---|---|---|---|
| **A13** | 平台能力矩阵与源码一致 | — | ✅ **升 `tested`**（静态结构事实；22 项断言在 portable+full 门控实跑 `1..22 / # pass 22`） |
| A03 | 新增连接器不改内核 | **外部真值**（真实官方接口 / §9.3 平台准入） | ❌ 维持 `implemented`（结构守卫**确已**实跑通过，但剩余缺口本仓无从提供证据） |
| S06 | 注入与越权红队集 | **本仓可闭合**（建对抗语料集） | 保持 `implemented` |
| R08 | 存储维护有界 | **本仓可闭合**（WAL/配额覆盖补全） | 保持 `implemented` |
| R10 | 开播与回滚不丢写入 | **本仓可闭合**（装配根接线） | 保持 `implemented` |
| S07 | 无明文密钥 | 待建功能（日志/诊断包/崩溃转储扫描） | 保持 `implemented` |
| S08 | 插件与供应链 | 待建功能 + 外部数据源（真实签名链、advisory 审计） | 保持 `implemented` |
| S10 | 数据外发与画像 | 待建功能（长期画像） | 保持 `implemented` |
| R09 | 升级预检与下载失败 | 待建功能（真实下载器） | 保持 `implemented` |
| B03 | 成本对账 | 待建功能（账单异步对账） | 保持 `implemented` |
| B06 | 场次复盘报告 | 待建功能 + 外部数据（真实场次、报告 UI） | 保持 `implemented` |
| R05 | 升级原子与回滚 | **外部条件**（真实 Windows 安装器/断电） | 保持 `implemented` |
| A02 | 仅改配置换 Provider | **外部条件**（付费账号无余额、真实 TTS 设备） | 保持 `implemented` |
| B05 | 真人接管音轨与回声 | **外部条件**（真实音轨/回声设备；automation=manual） | 保持 `implemented` |
| A06 | 多宿主与信任边界 | **待裁定**（wasm/remote「待用」是否属 R1 要求） | 保持 `implemented` |

**小计**：可闭合且已升 **1**（A13）｜本仓可闭合待做 **3**（S06/R08/R10）｜待建功能 **6**｜外部条件/真值 **4**｜待裁定 **1** = 15。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- 本轮**只升 1 条**，且升级依据是「命题为静态结构事实 + 断言在门控内实跑」——**不是**「补了个守卫」。
  A03 即反例：结构方向守卫确实通过，**但不得升级**（剩余缺口是外部真值）。
- 其余 13 条的缺口在**功能面/外部面**，加判据不能替代实现或外部证据；不得为凑数升级。
- 语义不变量：`MET_STATUSES = ['implemented','tested']` ⇒ 本次升级**不改变** met/unmet 归类，
  因而对待裁定项的 precondition 判定等对账守卫零影响。
- `tested` 的「测试计数」未写入 registry（易漂移的数字不进活文档），其载体是证据报告本身。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`admission-registry`）

新增 3 条 Requirement（各 1 Scenario）：
- **A tested acceptance names a non-empty archived evidence report**（An empty evidence field on a tested item is rejected）；
- **A tested acceptance's evidence report is a repository member**（An untracked report is rejected even if it exists on disk）；
- **The tested-evidence scan is closed over the whole tested set**（Every tested item is scanned and the set is non-vacuous）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w122.sh`，`PREBUILD_OK at attempt 1` + `GATE_OK at attempt 1`，首跑即绿，耗时 7m26s）。`rustc 1.96.0 (ac68faa20 2026-05-25)`。
- **registry 自身门控**：`node --test tests/registry.test.mjs` **56 / 0**；`cargo xtask check --registry` → `admission registry check passed`。
- **LNK1104 瞬态计数**：**0**（grep `LNK1104` = 0；`PREBUILD_OK attempt 1`，无 Defender 持锁的链接失败）。
- 全 workspace `cargo xtask check --profile full`：**95** 个测试二进制全 `test result: ok. ... 0 failed`，**真实测试失败 0**；clippy `--workspace --all-targets --locked -- -D warnings` 通过；`cargo fmt --all --check` 通过；前端门控（`[frontend] 全部步骤通过`）通过。
- **W122 新守卫确在门控内执行**（不是只在本机跑）：门控日志含 `ok 56 - guard: evidence outside reports/, or untracked, is rejected` 等四条 subtest，registry 计数 `1..56 / # tests 56 / # pass 56`。

## 七、提交 / 推送（一次 commit + push，SSH 443）

- 提交 SHA：**`e6d4f08469c5a8ae00f735f356f252bf7eda8623`**（主提交）。
- push 区间：**`772f8cb..e6d4f08`**，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。共 11 文件、+378/-2。
- 改动文件：`tests/registry.yaml`（A13 升 tested + A03 留痕）、`tests/registry.test.mjs`（+4 守卫）、
  `docs/adr/0100-tested-evidence-verifiable.md`（新建）、`docs/reports/2026-10-16-w122-tested-evidence-verifiable.md`（新建）、
  `docs/development-plan.md`（W122 段）、`docs/knowledge.md`（W122 注记）、
  `openspec/specs/admission-registry/spec.md`（合入 3 条 Requirement）、`openspec/changes/archive/2026-10-16-w122-tested-evidence-verifiable/`。
- openspec 三连：`validate --strict` valid → `archive --yes`（`+3`）→ `validate --archived --strict` **120 项 / 0 failed**。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：**38837204111**（workflow `Engineering foundation`，event `push`）。回读 `head_sha` = `e6d4f08469c5a8ae00f735f356f252bf7eda8623`，与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`：
  - `foundation (windows-latest)` — completed / **success**
  - `foundation (ubuntu-24.04)` — completed / **success**
  - 本机门控绿（`GATE_EXIT=0`）与 CI 双平台绿一致。
- **回填提交**（docs-only）自身 CI：按纪律做 **1 次**观测，结论记入 memory，**不在本报告占位**（W119 起的写法）。

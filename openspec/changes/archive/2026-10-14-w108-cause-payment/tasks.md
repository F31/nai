# w108 任务

## 1 建立 OpenSpec 变更

- [x] 写 `proposal.md` / `design.md` / tasks / 1 条 ADDED + 1 条 MODIFIED
- [x] `openspec validate 2026-10-14-w108-cause-payment --strict` 报 `is valid`

## 2 先写会红的判据（**先测后设计**）

- [x] `tests/contracts/xtask-commands.test.mjs` 加用例：凭据 `marker` 不存在 ⇒ 红、`file` 未被跟踪 ⇒ 红、
      `file` 落在 `scripts/contracts/` ⇒ 红、某格零实例 ⇒ 红、兑付表双向不等 ⇒ 两个方向各红、
      `why` 为空 ⇒ 红、缺输入 ⇒ 红
- [x] `tests/contracts/workflow.test.mjs` 加同族用例
- [x] **在现有数据上跑，确认红**：实测 `pass 15 / fail 2`，两条红的**原因唯一**——
      `the cause "host-subprocess" is in the closed set but no payment says what it stands on`
      （判据**自己**指出了那一格无人兑付，逐出的决定由此推出）。step 层同样：`pass 10 / fail 3`，
      原因亦唯一
- [x] `load()` 扩出 `reasonInput = {sources, tracked}`，并把**全部**既有调用点补上新参数
      （命令层 15 处、step 层 15 处，另加 3 个正交性测试文件）
- [x] 既有用例逐项结论**与扩张前相同**（扩面 MUST 带回归断言）：扩张后首次跑，除「该红的那条」外
      **逐项全绿**（命令层 16/17、step 层 12/13）

## 3 实现：两处闭集收缩 + 兑付表 + 核验

- [x] 新建 `scripts/contracts/cause-payment.mjs`：`reasonPaymentErrors`（**两处成因闭集不同、
      核验同一套** ⇒ 只写一次，避免分家）
- [x] `scripts/contracts/xtask-commands.mjs`：`STANDALONE_REASONS` 4 → 3（逐出 `host-subprocess`）
- [x] 新增 `STANDALONE_REASON_PAYMENTS`（3 格，逐格凭据见 design §5.2）
- [x] `commandErrors` 加第 4 个**必填**参数 `reasonInput`，接入 6 条核验
- [x] `schema --check` 的 `reason`：`host-subprocess` ⇒ `requires-compiled-artifact`
- [x] `scripts/contracts/workflow-steps.mjs`：`CI_ONLY_REASONS` 4 → 3（逐出 `host-subprocess`）
- [x] 新增 `CI_ONLY_REASON_PAYMENTS`（3 格）
- [x] `workflowStepErrors` 加第 5 个**必填**参数 `reasonInput`，接入同一套核验
- [x] step「Check generated contracts without modifying consumers」的 `reason`：
      `host-subprocess` ⇒ `compiled-artifact`
- [x] 两处常量与登记表上方**注释**同步订正（四条取值 → 三条；`host-subprocess` 的说明移除）
- [x] `step-config.mjs` **一行不改**（核对立证：`git diff --stat scripts/contracts/step-config.mjs` 空输出。
      它按闭集 `includes()` 分派，收缩自动正确）
- [x] 兑付表开一个**可选**注入位 `payments`（缺省用模块常量）——没有它，「少一格／多一格／
      凭据指错」这几种缺陷**无法被测试构造**（守卫都要注入验证）

## 4 测试与非空洞守卫

- [x] `tests/contracts/xtask-commands.test.mjs`：`STANDALONE_REASONS` 逐字钉改为三值
- [x] 「闭集非空」弱断言升级为**逐格 ≥1 实例**（命令层与 step 层各一处）
- [x] `tests/contracts/workflow.test.mjs`：`CI_ONLY_REASONS` 同族一处
- [x] 新增**在册数据守卫**：出货的兑付表 MUST 干净（`commandErrors(...) === []`／
      `workflowStepErrors(...) === []`），再逐条注入八种变形
- [x] 反向用例：一条**合法**的凭据替换（换成同一个文件里另一个真的存在的串）MUST NOT 红

## 5 非空洞注入验证（真改文件 + 真跑 + 逐字节还原）

基准：先跑 `cargo fmt --all`（零改动 ⇒ 基准 == 当前工作区状态），`sha256sum` 备份三个生产文件
（两个 `.mjs` + `xtask/src/lib.rs`）。每组的红数以 `node --test` 的 `pass/fail` 现测为准，
命中数 = 该组专有错误消息在输出里的出现次数（非零即判据在响）。

- [x] I1 某格凭据 `marker` 换成不存在的串 ⇒ **`15/3`**，`receipt is a claim, not a fact` × 6
- [x] I2 凭据 `file` 换成未被跟踪的路径 ⇒ **`15/3`**，`the repository does not track` × 6、
      `was not provided for judgement` × **0**（**顺序正确**：先报「未跟踪」，不误报「未提供」）
- [x] I3 凭据 `file` 换成 `scripts/contracts/workflow-steps.mjs`（自引）⇒ **`15/3`**，`true by construction` × 6
- [x] I4 某格**零实例**（把该格全部条目改归别因）⇒ **`14/4`**，`vacuously true` × 6
- [x] I5 兑付表**多**一个闭集外的键 ⇒ **`15/3`**，`declares a payment but is not in the cause table` × 6
- [x] I6 兑付表**少**一个键（闭集有、表没有）⇒ **`15/3`**，`no payment says what it stands on` × 6
- [x] I7 删掉凭据的 `why` ⇒ **`15/3`**，`does not say what it answers` × 6
- [x] I8 **删掉门控里那条 `--workspace` 调用**（`superseded-by-workspace-run` 的**兑付物**）
      ⇒ **`15/3`**，`receipt is a claim, not a fact` × 6 —— 这条是本设计**最强**的兑付
- [x] I9 step 层：`platform-toolchain` 凭据 `marker` 换成不存在的串 ⇒ **`11/3`**，同族消息 × 6
- [x] I10 step 层：某格零实例（`not-in-profile` 全改归 `platform-toolchain`）⇒ **`10/4`**，`vacuously true` × 6
- [x] 注入前**先确认探针值不存在**（W93 教训：选了个真实存在的串 ⇒ 假阴性）——
      `grep -rlF` 四个探针串，**全部零命中**
- [x] `pass=0 fail=1` 是「注入自身坏了」的特征信号 ⇒ 本轮**未出现**该形态（每组均有多条命中）
- [x] **I2 暴露了一处测试缺陷并已加固**：首次跑 I2 得 `pass 3 / fail 15` 而 W108 消息**零命中**
      ——输入构造在读不到凭据文件时抛 `ENOENT`，把「判据该报的缺陷」伪装成「测试自己坏了」。
      修法：输入构造**不抛**（该文件不进 `sources`），让**判据**去具名报告；五个测试文件的输入构造
      同步加固。修完 I2 重跑得 `15/3` 且命中 6 ⇒ **判据在响**
- [x] 逐字节还原：每组跑完 `cp` 备份拷回，末次 `sha256sum -c` **三行全 OK**
- [x] 注入临时目录已逐个删除（`rmdir .w108-inject`；**MUST NOT** 被 commit）——
      `git status --porcelain` 只剩预期改动，无 `?? .w108-inject/`

## 6 文档与登记

- [x] 新增 `docs/adr/0086-cause-payment.md`（决策 1–6 / 如实披露 5 条 / 遗留 4 条 / 替代方案 8 条 /
      关联）；并订正其中对 KNOWLEDGE 小节号的引用（§6.33）
- [x] ADR 0073 / 0078 **追加勘误段**（**原文保留不改写**）：0073 的遗留 1 已关闭（判据已配会过期的断言）；
      0078 决策 5 的成因枚举收缩（决策 5 的规则本身**不变**，并立证 `step-config.mjs` 一行不改）
- [x] `AGENTS.md` / `docs/development-plan.md`：**判载体后就地订正**被本轮改写的**活陈述**——
      `AGENTS.md:852-858`（宿主瞬态归因）、`AGENTS.md` W95／W100 段两处成因枚举、
      `AGENTS.md` 的「与平台无关的成因」举例（`host-subprocess` → `deliberate-on-demand`）、
      `docs/development-plan.md` W100 段两处成因枚举；并**追加 W108 轮次段/条目**。
      ⚠️ **同形不同义、不订正**：本文件与历史报告里**各轮段**中关于宿主瞬态的叙述是**那一轮的记录**
- [x] 新增 `docs/reports/2026-10-14-w108-cause-payment.md`（含侦察实测、逐格凭据、注入红数、
      双宿主坑、如实披露、遗留与边界）
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.33 条目，并更新 §10 计数索引
      （Rust **95/914**、Node **13/319/0**、`--archived` **106/0**）
- [x] `tests/registry.yaml`：**不改**（W95/W96/W100/W101 同族均未登记；本目录不新增验收 id）

## 7 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。

实测记录（本轮）：

- `cargo xtask check --profile full`：绿（`EXIT=0`），计数现测 Rust **95**（78 `Running` 含 1 行伪目标
  + 18 `Doc-tests`）/ **914 passed / 0 failed**，Node **13 / 319 / 0**。**共跑 5 次**：
  ① `rustc` 挂死（33 分钟零输出、CPU 2.4s）、② ③ ④ 宿主 `LNK1104`（每次点名不同目标）、⑤ 绿。
- 归档前 validate：`--specs --strict` **43/0**、`--all --strict` **44/0**、`--changes` **1/0**。

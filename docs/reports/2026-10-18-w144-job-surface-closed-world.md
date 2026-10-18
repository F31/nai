# W144 报告：作业面闭世界 —— 15 条「铸了 `job_id` 却没有状态来源」的声明

- 日期：2026-10-18
- 起点：W138 报告 §五第 1 条「12 条路由铸 `job_id` 而无状态来源」，经 W143 报告 §八第 5 条转交。
- 交付：`scripts/contracts/job-surface.mjs`（新，两条判据 + 三张单一来源表）、
  `schema.test.mjs` +4 条用例、`JobAccepted` 补 description（生成物再生）、ADR 0118、
  OpenSpec `w144-job-surface-closed-world`。

## 一、勘定与现测

五组数字全部本轮现测，不是从上轮推断。

| 项 | 现测值 | 怎么量的 |
| --- | --- | --- |
| `control.yaml` 的 `paths` | 40 | `SwaggerParser.parse` 后取 `Object.keys(paths)` |
| 带 `202` 的操作 | **15** | 逐 `path × HTTP_METHODS` 取 `responses['202']` |
| 其中 body 是 `$ref JobAccepted` | 15 | 逐条读 `content['application/json'].schema.$ref` |
| 其中带 `x-implementation-status` | **0** | 逐条读该键 |
| `remote-knowledge.yaml` 带 `202` 的操作 | 0 | 同一口径 |
| `JobView.kind.enum` | `["upgrade"]` | `components.schemas.JobView.properties.kind.enum` |
| `JobAccepted.required` | `[job_id, command_id, status]` | 同上 |
| `JobAccepted` 在 Rust 侧的生产者 | **0** | `grep -rn "JobAccepted" --include=*.rs crates/ apps/` 只命中生成物 `SCHEMAS_JSON` |
| 15 条在路由表里的分支 | **0** | 逐条搜路由表，全部落 `lib.rs:665` 兜底 404 `CapabilityAbsent` |

15 条清单（逐条 `impl=-`）：
`post /knowledge/import`、`post /knowledge/snapshots/{id}/publish`、`post /knowledge/evaluate`、
`delete /knowledge/documents/{id}`、`put /providers/{id}`、`post /providers/{id}/test`、
`post /avatars/{id}/validate`、`put /scenes/{id}`、`post /scenes/{id}/validate`、
`post /devices/pair`、`post /devices/{id}/revoke`、`post /upgrades/execute`、
`post /upgrades/{id}/rollback`、`post /backups`、`post /restore/preflight`。

### 计数订正：W138 的「12」从来没有对过

W138 §五第 1 条标题写「12 条同族路由」，清单里却列了 13 个端点（其中 `providers post`、
`avatar post`、`scene post`、`scene delete` 四种写法在契约里根本不存在），而同一份报告
自己记的 `grep -c "jobResponses"` 是 15（含锚点定义行，即 1 个锚点 + 14 个别名）。
W143 §八 又沿用了「12 条」这个数字。

本轮用解析器口径定案为 **15**，差额来自：清单漏了 `providers/{id}/test`、
`devices/{id}/revoke`、`restore/preflight`，且没有把 `upgrades/execute` 与
`upgrades/{id}/rollback` 计入同一族（它们确实也是 `202 + JobAccepted`，只是拒绝的方式不同）。
「12」是一次靠记忆汇总的产物，`grep -c` 反过来是对的。

### 口径坑：`validate` 会把 `$ref` 内联

第一次测量用 `SwaggerParser.validate`（`validation.mjs` 的 `apiDocuments()` 走的就是它），
量出 `202_with_JobAccepted = 0`——15 条的 body 全部被展开成内联对象，
`$ref` 的字面已经不存在了。改用 `parse` 才得到 `=15`。

**凡要判「是不是同一个 schema」，MUST 用 `parse`。** 这条写进 ADR 0118 与
`job-surface.mjs` 的注释，因为照着 `apiDocuments()` 抄一遍就会掉进同一个坑。

## 二、收口方式：不实现，只让沉默失效

给 14 条路由各造状态表是 14 次迁移 + 14 套状态机，而**每条的状态形状至今没有人设计过**
（备份的终态是什么？测评的终态是什么？）。把没人设计过的东西赶进一轮实现，
产出的只会是 14 个各自编造的进度词汇。

本轮改的是**声明面**，判据形状由契约自己给出：`JobView.kind` 早就写着
「Discriminator for the shared job id space; only upgrade jobs have a journal today」。
于是：

1. `202 + JobAccepted` 的操作集与 `JOB_SURFACES` 登记集**双向相等**；
2. 在册项**恰有** `kind`（能被 `/jobs/{id}` 轮询）或 `blockedBy`（今天不铸 id 的理由）；
3. `kind` ⇔ 已实现；
4. `blockedBy` 落闭集 `no_handler` / `refuses_without_installer`，每个取值带一个**源码锚点**；
5. `JOB_KINDS` 与 `JobView.kind` 双向相等，每个 kind 的 journal 与 reader MUST 被跟踪且锚点文本还在；
6. `JobAccepted.status` 的取值域恰好 `accepted`（`accepted` ≠ 可轮询）。

第 2 条是全部要点：它把「我也不知道该怎么办」从一个**沉默**变成**必须写的值**。

## 三、两类 `blockedBy` 必须分开

| 取值 | 今天是哪几条 | 源码事实 |
| --- | --- | --- |
| `no_handler` | 13 条 | 路由表零分支 ⇒ 兜底 `:665` 404 `CapabilityAbsent "control operation not implemented"` |
| `refuses_without_installer` | `upgrades/execute`、`upgrades/{id}/rollback` | 有专用分支 `:628/630` → `upgrade_refusal`，`:1121` 逐字写着 `no job id is minted` |

混同会让「做不到」与「没有这条」在客户端不可区分——ADR 0113 已为读取面裁定过同一件事。
两个锚点都是「这句话一改，这个声明就不再成立」的那一句。

`kind` 的锚点更讲究：`JOB_KINDS.upgrade.reader.text` 取 `"kind": "upgrade"` 而不是裸的
`"upgrade"`。实测同一个文件里 `upgrade_refusal` 的**类别名**也是 `"upgrade"`（`:1118`），
只锚那个词的话两处同时命中，「reader 不再报这个 kind」永远不会变红。用例里有一条断言
钉住这个选择（裸词出现 >1 次、带判别键的形式恰好 1 次），谁简化回去谁当场看到红。

## 四、判据与注入

`jobSurfaceErrors`（契约侧）与 `jobSourceErrors`（仓库侧）分开，输入域不同：前者吃解析后的
文档对象，后者吃 `.git/index` 路径集 + 源码文本。`readSource` 由调用方**注入**，
模块只判定不做 I/O——判定本身才能被合成输入驱动。

合成样本 MUST NOT 从真实文档克隆：实测 `control.yaml` 的 14 条共用 `&jobResponses` 别名，
YAML 展开后它们是**同一个对象**（`a === b` 为真），而 `structuredClone` 按结构化克隆算法
**保留共享引用** ⇒ 改一处等于改 14 处，反向用例会为了错误的原因变绿。
因此用 `jobDocument()` 构造最小文档。

载重验证（三条变异各自产生**恰好一条**具名错误）：

| 变异 | 判据输出 |
| --- | --- |
| 登记表删掉 `post /backups` | `… answers 202 with a job id, but JOB_SURFACES does not declare it …` |
| 把 `upgrades/execute` 标成 `kind: upgrade` | `… claims kind "upgrade" but is not declared implemented …` |
| `JobView.kind.enum` 清空 | `… JOB_KINDS declares kind "upgrade" … but JobView.kind does not list it …` |

用例总数 34（本文件），新增 4 条：真实文档已闭 + 互斥与闭集（10 个注入）+ 双向与非空洞
（7 个注入）+ 锚点（6 个注入 + 1 条判别性反例）。

## 五、诚实边界

1. **`blockedBy` 的取值本身仍是声明。** 锚点只证明「那句源码还在」，不证明「这条路由真的
   没有分支」。要变成可判定需要一张执行面名册（谁在路由表里有分支），属 W143 §八 已登记的
   「产物与扫描域的界线」那一类，本轮不顺手做。
2. **`JobAccepted` 仍然零生产者。** 本轮一个 202 都没有接上真实实现——这个事实**没有变化**，
   它现在是被**登记**着，不是被消除。如实披露不是缺陷，不实声明才是。
3. **没有一条在册项带 `kind`。** 今天 15 条全部 `blockedBy`；`kind` 那一半的判据只在合成
   样本上被执行过（真实文档上没有一条 `kind` 项可以走通）。这不是缺陷，但它意味着
   **「`kind` ⇔ 已实现」的正面路径尚无真实样本**——第一次真正接上作业状态时，它会第一次
   在真实文档上生效。
4. **`remote-knowledge.yaml` 的作业面是 `{}` 而不是省略。** 「我们看过，没有」与「我们忘了」
   是两件事；后者会让「给它加一条 202」变成零信号的编辑。

## 六、门控

- `cargo fmt --all --check`：EXIT=0
- `cargo xtask check --profile full`：**`GATE_EXIT=0`**
  - Rust：102 条 `test result` / **1146 通过 / 0 失败**（含 18 Doc-tests）。与 W143 同为 1146——
    本轮只加 JS 用例，没有加 Rust 用例，`JobAccepted` 的 description 也不进任何断言。
  - Node：15 次运行 / **371 通过 / 0 失败**（W143 是 367，+4 即本轮新增四条）。
  - 前端：7 步全过（`[frontend] 全部步骤通过（工程基础，非渲染/人工 UI 验收）`）。
- `cargo xtask schema` 再生后 `cargo xtask schema --check` = **no drift**（`PROTOC` 指向 21.12）。
  改 `control.yaml` 的两个生成物 `control_http.rs` / `http.ts` 各改一处。
- `openspec validate --all --strict` = **44 passed / 0 failed**（W143 是 43，+1 即本轮变更）。
- 首跑红**两次**，都不是代码缺陷，如实记下来：

  1. `consumptionOf` 报 `ENOENT: .w144-why.mjs`。成因是我自己的探针文件：那条用例先 `readdir`
     扫工作区、我随后把探针删了、再读时已不在（TOCTOU）。**这不是判据灵敏，也不是缺陷**——
     探针清干净后复跑即绿。
  2. `LNK1104` 打不开 `nai_avatar_2d-*.exe`。本机既有形状。`CARGO_INCREMENTAL=0` +
     `CARGO_BUILD_JOBS=1` 已在跑，仍红一次；按既有处置先
     `cargo build --workspace --all-targets --locked` 串行预构建（`PREBUILD_EXIT=0`），
     复跑即绿。**未改任何代码、未放宽任何判据。**

## 七、CI 观测

推送 `a01962f`（`a01962f27f215155321a55466e13f68c21731eea`），运行 **`39109328945`**。
判结论三件齐：唯一查询参数 `cb=`（纳秒）、请求头 `Cache-Control: no-cache`、
回读 `head_sha` 与推送 SHA **完整 40 位**一致，且已看到 `conclusion`。

| 平台 | 状态 | 步骤（API 口径） | `skipped` |
| --- | --- | --- | --- |
| `foundation (windows-latest)` | `completed / success` | 20 | 第 8 / 11 / 14 步 |
| `foundation (ubuntu-24.04)` | `completed / success` | 20 | 第 11 步 |

两平台的 `skipped` 分布与 W130 起的形状逐条一致。步骤条数是 **API 口径**
（含 `Set up job` / `Post Run …` / `Complete job`），与 workflow 的 `- name:` 步数
**不能相减**，此处不作条数断言。

## 八、具名登记（本轮发现但不做）

1. **W138 §五第 1 条的计数错误已订正，但 W138 报告本身不就地改**（已发布的历史记录）。
   订正落在本报告 §一 与 ADR 0118 的背景段。
2. **`blockedBy` 取值缺执行面判据**：见 §五第 1 条。
3. **W143 §八 其余 4 项仍未接手**：2 份归档 proposal 结构性缺段、`13`/`15` 幽灵值、
   产物与扫描域的界线、真实补集「非空」从此不可断言。

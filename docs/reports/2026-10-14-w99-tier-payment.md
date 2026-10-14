# W99：站点图层声明（`tier`）必须由宿主解析出的区域兑付

- 变更：`openspec/changes/archive/2026-10-14-w99-tier-payment`（`archive` 后真路径）
- ADR：[0077](../adr/0077-tier-payment.md)
- 前置：W84（站点可达性闭世界）、ADR 0073／0074／0075／0076

## 一、目标

`scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES` 每条站点声明
`{id, host, marker, base, tier}`。W84 把 `host`／`marker` 那一面管住了；本轮修的是另一面：
**`tier`（∈ `portable`／`full`／`ci`，意思是「跑它的最低一层门控」）只有一个形状检查**——

```
siteShapeErrors()  →  TEST_TIERS.includes(site.tier)      ← 仅此而已
```

⇒ 一个**未付代价的声明**：把 `portable` 改成 `full`（或反过来），覆盖集不变、闭世界不变、门控全绿。

## 二、缝在哪

### 2.1 探针 P-A（改真文件 → 真跑 7 个 portable 套件 → 备份拷回 → `sha256` 逐字还原）

| 编号 | 改动 | 结果 |
| --- | --- | --- |
| **P-A** | 把 `tests/contracts/npm-scripts.test.mjs` 的注册从 `engineering_checks` **移进** `["check","--profile","full"]` arm，站点仍声明 `tier: 'portable'` | **192 passed / 0 failed —— 零信号** |
| 还原 | 备份拷回（**不用 `git checkout --`**，W97 教训） | `sha256 = f61b2c73…728c` **逐字一致** |

P-A 证明两件事：① `tier` 声明不付代价；② **注册点的位置本身也没有守卫**——把一行 `--test`
从 portable 面搬进 full 面，portable 面少跑一个套件，无人发现。T2 的「marker MUST 在
**剥离注释后**的 `engineering_checks` 区域里」正是这两条的守卫。

### 2.2 注释陷阱（本层再实测一次）

`xtask/src/lib.rs` 第 46 行的**注释**里写着 `tests/contracts/descriptor.test.mjs`：

```
// （`tests/contracts/descriptor.test.mjs`）不在此列，仍留在 contracts suite。
```

实测 `gateRegions(libText)` 的 `engineering_checks` 区域：**原文含**该串、**剥离注释后不含**。
⇒ 不剥注释，两个 `ci` 站点会被这条注释**自我满足**，T5 的反向断言同时失效。
本报告 §五 的 P5 就是这个形态的注入。

### 2.3 「出现过」≠「被调用」

`scripts/frontend-gate.mjs` 的 `STEPS` 里，`apps/desktop` 有 3 项（`typecheck`／`build`／`test`），
只有 `test` 的**脚本值**含 `tests/*.test.mjs`（`tests/contracts-ts` 有 2 项、`.` 有 1 项）。
⇒ 用「该目录出现过 STEP」当 `full` 的兑付，会让那条路径**恒真**。

## 三、判据设计

新 `scripts/contracts/tier-payment.mjs`，8 条：

| 编号 | 判据 | 判定域（由**解析**得出） |
| --- | --- | --- |
| T1a | `Object.keys(TIER_PAYMENTS)` MUST == `TEST_TIERS` | 两张常量表 |
| T1b | 每个 tier 取值 MUST ≥ 1 站点 | 站点表 |
| T2 | `portable` 站点的兑付串 MUST 在 `engineering_checks` 区域（**剥注释后**）里 | `gateRegions` + `stripRustComments` |
| T3 | `full` 站点的包目录 MUST 是某 STEP 的 `dir`，且**该 STEP 的 `script` 脚本体** MUST 含 marker | `driverSteps` + `packageScripts` |
| T4 | `ci` 站点的 marker MUST 在某个 step 的 `run` 文本（剥整行注释）里 | `workflowStepsOf` + `stripShellComments` |
| T5 | **反向**：`full`／`ci` 的 marker MUST NOT 落在 portable 区域里 | 同 T2 |
| T6 | 缺输入按其需要具名（存在该 tier 的站点时那份输入必需） | — |
| T7 | 区域不可解析／无兑付规则 ⇒ 具名失败，MUST NOT 静默降级 | — |

### 3.1 三个设计要点

1. **`tier` 的语义先定死 =「跑它的最低一层」**。语义不定，T5 无从写起：只有在「最低一层」的
   语义下，「也在 portable 面里」才等于「它就该声明 `portable`」（本仓库既有约定：凡
   `engineering_checks` 里跑的测试，站点都声明 `portable`，即使 `full` 也跑它们——portable ⊂ full）。
2. **兑付形状由 host 派生，不留可选的 `via` 字段**：

   ```
   host === 'xtask/src/lib.rs'  → 兑付串 = marker            （内联注册：`--test <marker>` 就在区域里）
   host !== 'xtask/src/lib.rs'  → 兑付串 = /<module>::\w+\(root/   （宿主模块在区域里被调用）
   ```

   `xtask-registry` 是第二种：区域里是 `registry::check(root)?;`，而 `tests/registry.test.mjs`
   的字面量在 `xtask/src/registry.rs` 里。「可选」就是判据的缝（W93）——一个可省的字段会让某个
   站点悄悄走默认分支。
3. **T5 反向不可省**：只做正向会漏掉「声明**少报**了本机门控真正跑到的东西」。把 marker 混进
   portable 面、站点仍声明 `ci`，就是一条**合法的逃离路径**（P4 实测）。

## 四、实现

- 新 `scripts/contracts/tier-payment.mjs`（判据；导出 `HARNESS_LIB`／`DRIVER`／`CI_WORKFLOW`／
  `PORTABLE_REGION`／`TIER_PAYMENTS`／`stripShellComments`／`portableSurface`／`tierErrors`）。
- 新 `tests/contracts/tier-payment.test.mjs`：**22 个用例**（合成世界 + 注释陷阱 + 真实仓库 + 非空洞 + 反向不红 + 三条边界）。
- `xtask/src/lib.rs`：`engineering_checks` 里 **+1 行登记 + 一段注释**（Rust **零行为改动**）。
- `scripts/contracts/test-reachability.mjs`：`TEST_REGISTRATION_SITES` **11 → 12**（新增
  `xtask-engineering-tier-payment`，`tier: 'portable'`）。既有 11 条**一个字符未动**。
- `tests/registry.test.mjs`：逐字站点清单同步（并注明 W99 新增）。

### 4.1 一处顺序前提（铁律一的适用）

`the real repository reaches every tracked test file`（W84）与
`dropping a site reds the reachability judge but not the tier judge`（W99）都断言
「本轮新测试文件**在册**」。而提交前 `tests/contracts/tier-payment.test.mjs` **还没进 `.git/index`**
（实测：先跑套件时该用例红，报 `site xtask-engineering-tier-payment matches no tracked file`）。
⇒ 修法是**由构造使前提成立**（用例内用「`.git/index` 的集合 ∪ 新测试文件」），
MUST NOT 用短路关掉判据；真跑门控时则**先 `git add` 再跑**（`.git/index` 就是判定域）。

## 五、非空洞验证

### 5.1 注入（真改文件 → 真跑 8 个 portable 套件 → 备份拷回 → `sha256` 逐字还原）

| 编号 | 改动 | W99 具名错误（逐字） |
| --- | --- | --- |
| **P1** | `npm-scripts` 注册从 `engineering_checks` 移进 full arm | `site xtask-engineering-npm-scripts declares tier portable yet "tests/contracts/npm-scripts.test.mjs" is not in the engineering_checks region of xtask/src/lib.rs — a tier declaration with no region paying for it is a lie` |
| **P2** | 删掉 `tier-payment` 的注册行 | `site xtask-engineering-tier-payment declares tier portable yet "tests/contracts/tier-payment.test.mjs" is not in the engineering_checks region …` |
| **P3** | `registry::check(root)?;` 搬出 `engineering_checks` | `site xtask-registry declares tier portable yet "registry::<fn>(root" is not in the engineering_checks region …` |
| **P4** | 把 `tests/ui/*.test.mjs` 加进 `engineering_checks` | `site root-test-ui declares tier full yet its marker "tests/ui/*.test.mjs" is inside engineering_checks of xtask/src/lib.rs once comments are stripped — the declaration understates what the local gate already runs` |
| **P5** | CI step 的 `run` 里，`descriptor.test.mjs` 只活在 `#` 注释里 | `site xtask-contracts-suite declares tier ci yet "tests/contracts/descriptor.test.mjs" is not named by any step's run in .github/workflows/foundation.yml — a CI-only test that no named step runs is silently unreachable`（两个 `ci` 站点各一条） |
| **P6** | 往 CI 加一个具名 step（边界用） | **W99 零错误**（`22 passed`）——W95 红 2 条 |

`sha256` 逐字还原：`xtask/src/lib.rs` `4ac2bbc1…9e05`（P1–P4 前/后一致）、
`.github/workflows/foundation.yml` `2f46d5f5…8c1c`（P5／P6 前/后一致）。

**⚠️ 本轮踩到的流程缺陷（当场修掉）**：一次 `SIGTERM` 把 P5 的改动留在 `foundation.yml`，
而下一轮**在残留改动上取备份** ⇒ 那个「RESTORE OK」实际是把**坏状态**备份拷回。
修法 = **每轮先按索引复原（`git checkout -- <file>`）并断言 `git diff --quiet` 干净，再取备份**。
⇒「逐字还原」这条纪律**只有在「起点干净」被断言之后才有意义**。

### 5.2 边界（互不顶替）

六个探针 × 六个判据套件的 fail 数（每个套件单跑）：

| 探针 | W84 `registry` | W95 `workflow` | W96 `xtask-commands` | W97 `gate-coverage` | W98 `npm-scripts` | **W99 `tier-payment`** |
| --- | --- | --- | --- | --- | --- | --- |
| P1 `npm-scripts` 注册搬家 | 0 | 0 | 0 | 0 | 0 | **3** |
| P2 注册行删除 | **1** | 0 | 0 | 0 | 0 | **3** |
| P3 `registry::check` 搬家 | 0 | 0 | **2** | 0 | 0 | **3** |
| P4 full 的 marker 进 portable 面 | 0 | 0 | 0 | 0 | 0 | **3** |
| P5 CI marker 只在注释里 | 0 | 0 | 0 | 0 | 0 | **3** |
| P6 新增具名 CI step | 0 | **2** | 0 | 0 | 0 | **0** |

读法：**P1／P4／P5 是 W99 独有**（三个不同机制：注册搬家、反向、注释）；
**P2 与 W84 共红**（两个判据、两种理由：W84 = 覆盖集少一个文件，W99 = 层级声明没兑付）；
**P3 与 W96 共红**（同一行 `registry::check(root)?;` 既是 `check --registry` 的门控落点，又是
`xtask-registry` 的 portable 兑付串——**同一处文本、两种语义**，W98 的 M1 同形）；
**P6 是干净的边界**：W95 红、W99 绿（本轮不读 step 的名字与 `kind`）。
P5 同时是注释陷阱的实证：**W84 绿**（marker 仍逐字在 host 里）而 **W99 红**（剥注释后没有 step 跑它）。

### 5.3 基线

- 8 个 portable 套件合跑：**214 passed / 0 failed**（W98 的 192 + 本轮 22）。
- `tests/registry.test.mjs` **52**、`schema` **30**、`workflow` **13**、`xtask-commands` **17**、
  `gate-coverage` **24**、`npm-scripts` **34**、`tier-payment` **22**、`platform-matrix` **22**。

## 六、门控与 validate

### 6.1 全量门控

`cargo xtask check --profile full` → **`EXIT=0`**（**5m15s**，宿主 `source ~/.nai-msvc-env.sh` +
`CARGO_TARGET_DIR=E:/nai-build-target CARGO_INCREMENTAL=0`）。日志逐字核算（**不凭记忆**）：

| 项 | 值 |
| --- | --- |
| `^     Running ` 行 | **74** |
| `^   Doc-tests ` 行 | **18** |
| `^test result:` 行 | **91** |
| 汇总 `passed` / `failed` | **862 / 0** |
| `LNK1104` / `error[` / `out of memory` | **0 / 0 / 0** |

⇒ **91 / 862**（`74` + `18`），**与已记录的 W95／W96／W98 完全一致**（三轮同样只加登记行、
零 Rust 行为改动；W98 报告记 `74`/`18`/`91`/`862`/`0`，逐项对上）——这正是「未误改代码」的旁证。
Node 十套件（门控日志内顺序 = `lib.rs` 的登记顺序）：

```
registry 52 / schema 30 / platform 22 / workflow 13 / xtask-commands 17 /
gate-coverage 24 / npm-scripts 34 / tier-payment 22 / desktop-test 5 / test:ui 52
```

⇒ **`tier-payment` 的 22 个用例确实在门控里跑过**（不是只在本机手跑）。

### 6.2 两条 validate

| 时机 | 命令 | 结果 |
| --- | --- | --- |
| 归档前 | `openspec validate --all --strict --no-interactive` | **44 passed / 0 failed**（44 items = **43 规格 + 1 活动变更**） |
| 归档后 | `openspec validate --all --strict --no-interactive` | **43 passed / 0 failed**（43 规格；活动变更已归档） |
| 归档后 | `openspec validate --all --strict --archived --no-interactive` | **97 passed / 0 failed**（97 items） |

归档目录 **96 → 97**；归档名为 `2026-10-14-w99-tier-payment`（**系统日期戳**，跨零点会错、须改名后重跑）。
归档输出：`Task status: ✓ Complete`、`+ 1 added` / `~ 0` / `- 0` / `→ 0`
（`engineering-foundation/spec.md` **348 → 381 行**，新 Requirement 落在第 350 行）。
**本轮无 `CHANGE_WHY_TOO_LONG` 警告**（现测 `grep -c` = **0**；W98 那轮**有**——该警告按轮而定、
MUST 现测、MUST NOT 照抄上一轮）。

### 6.3 门控 / 归档 / validate 三件齐

① 门控 `EXIT=0`（91/862，`LNK1104` 0）；② 归档 `Task status: ✓ Complete`（`+1 added`）；
③ 两条 validate **43 / 97**，均 `0 failed`。规格正文 **483 字符** < 500。

## 七、CI

**结论：`success`。** run **#131**（id `38494125766`），`head_sha` **`e075f9a`**，
`2026-10-14T01:09:04Z → 01:18:38Z`（约 **9m34s**）。

**三件齐（判结论的输入）**：① URL 带**唯一查询参数** `cb=<time.time_ns()>`
（破坏沙箱代理的响应缓存）；② 请求头 `Cache-Control: no-cache`；③ 回读的 `head_sha`
**等于本次推送的 SHA**（`e075f9a`）。`curl` 一律 `--tlsv1.2 --tls-max 1.2 --max-time 45`
（schannel 对 TLS 1.3 握手失败、且无 `--max-time` 会挂死——两者都是既有实测教训）。
**单轮观测 3 次请求**（配额 `60/h` 按 IP；实测 `remaining 43`），MUST NOT 轮询。

本轮**没有失败**，因此不需要走「check-run 注释」那条无凭证读失败原因的路
（W98 那条路已沉淀在 `KNOWLEDGE.md` §11）。
顺带核对：上一轮的 runner 瞬态（`#128 failure` → 同源代码的 `#129`／`#130 success`）
在本轮 `per_page=3` 的回看里仍是同样的形态，**未再复现**。

## 八、遗留

1. **`stepIdentity` 让「具名 step」近乎恒真**：只有 `name`／`uses`／`run` 三者全缺的 step 才得到
   空身份 ⇒ T4 的「具名」过滤几乎不拦东西，其实质约束是「某个 step 的 `run` 文本逐字提到它」。
   **不修**：要收紧就得要求该 step 在 ADR 0073 的 `WORKFLOW_STEPS` 里在册，那会让 T4 与 W95
   **同源共红**（同一个未注册 step 报两条），把边界弄糊。
2. **`portable` 的拒否是「剥离注释后的文字域」，不是「marker 是否为一次真实调用」**：
   把某个 `ci`／`full` 站点的 marker 放进 `engineering_checks` 调用脚本的**字符串字面量**里，
   T2／T5 会放行。**如实登记**，见 §九。
3. **`\w+\(root` 形状的假红方向**：若将来某入口改了实参名（不再叫 `root`），对应站点的 portable
   兑付会**假红**。方向安全（假阳性而非假阴性），**不改**。
4. W90 §八 1/3/4/5、W91 跨语言键级消费、W92 基线收录、W93 锚点唯一性、W94 `needs_external` 审计
   （本轮实测无订正物）、W95 `ci-only` 成因、W96 五项（§八 3 已实测关闭）、W97 六项、W98 §八 1/2/7
   **均未动**。

## 九、如实披露

1. **`portable` 的判定域是文字域**（见 §八 2）：本轮的拒否靠 `stripRustComments` 之后的
   `engineering_checks` 区域里有没有那个串，**不是**「marker 是否是一次真实调用」。
   这是一处**有意的宽松**——收紧到语法级需要 Rust 语法树，收益不成比例；且它挡住的
   （W99 探针 P-A、P5）已经是当前能构造出的全部逃离形态。
2. **「具名 step」近乎恒真**（见 §八 1）。
3. **不验测试能跑通**：要执行（`pnpm`／protoc 在宿主上不可用），且**铁律二**——宿主瞬态
   MUST NOT 改变结论。
4. **不判 `frontend::check` 的真实执行**：只判驱动表这一层接线；真跑由 `check --profile full` 管。
5. **不判 CI 的 `if:`／`continue-on-error`**（W98 §六 2 遗留，仍在）。
6. **被审对象零改动**：`scripts/frontend-gate.mjs`、`.github/workflows/foundation.yml`、
   三份 `package.json`、`TEST_REGISTRATION_SITES` 的既有 11 条**一个字符未动**；
   `xtask/src/lib.rs` 只加 1 行登记 + 1 段注释。

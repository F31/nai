# W91：配置键的归因（读取点 MUST 绑定到拥有它的 schema）

- 轮次：**W91**（承接 W90 遗留 2 的母题）
- 日期：2026-10-13
- 变更：[`w91-config-key-attribution`](../changes/archive/2026-10-13-w91-config-key-attribution/)
- 裁决：[ADR 0069](../adr/0069-config-key-attribution.md)

## 一、判断

**不是**「有人写错了配置」，**也不是**「实现有缺陷」，而是**W90 的门控只覆盖了配置面的六分之一**：

| 面 | 载体 | 实测 |
| --- | --- | --- |
| 文件级守卫 | W73/W74 的 `coverageErrors` | 覆盖 `schemas/` **全部** schema（每份要么被执行体消费、要么具名例外） |
| **键级守卫** | W90 的 `config-consumption.mjs` | **只覆盖 `runtime.schema.json` 一个文件**（37 键） |
| **中间一层** | `schemas/config/` 其余 5 份 schema 的**键** | **空着**——`avatar-2d` 9／`backup` 10／`binding-plan` 6／`provider` 12／`upgrade-package` 22，**59 个键无人核对** |

⇒ W90 用了「**一个 `required` 键有人看吗**」这把尺子，却只量了一把尺子上的**一厘米**。本轮把同一把尺子
量完 `schemas/config/` **全部 6 份**，并且回答一个 W90 遗留 2 直接点名的问题：**「这个键的读者，凭什么是它？」**

## 二、核实（先测后设计；四轮迭代，每轮被实测推翻一次）

### 2.1 不能原样扩面（实测）

W90 的读取形状是**裸键名** `EXPR["<key>"]`，**不带对象上下文**。96 个受判键里：

- **3 个裸名跨 schema 重名**：`schema_version`（×5 份 schema 顶层）、`mode`（×3）、`currency`（×2）。
- **通用名词跨对象污染**：实测 `["sample_rate"]` 命中 `apps/render-worker`、`["session_id"]` 命中
  `crates/nai-control-api`、`["path"]`／`["sha256"]`／`["id"]` 命中 `crates/nai-plugin-sdk`——
  全是**同名不同物**。

⇒ 照 W90 的规则，这些键会**凭空获得读者**。**判定单元 MUST 是 `(schema, 键路径)`**，
且读取点 MUST 落在该 schema 的**归因文件集**内。

### 2.2 读取写法不止一种（实测四种）

| 写法 | 样本 | 裸键名形状能否扫到 |
| --- | --- | --- |
| 直接索引 | `manifest["database"]["sha256"]`（`snapshot.rs`） | 能 |
| **中间变量** | `let media = &config["media"]; media["lease_renew_ms"]`（`composition.rs`） | 能（裸名），但**完整路径形状扫不到** |
| **辅助函数** | `text(&value, "package_id")`／`amount(&value, "install_bytes")`（`manifest.rs`） | **不能**（键名是字符串实参） |
| **结构体解码** | `let config: Config = serde_json::from_value(...)`（`avatar-2d`） | **不能**（键名是结构体字段） |

### 2.3 三轮形状迭代（每轮都被实测推翻）

1. **裸键名形状**（W90 原样扩面）：96 键中 31 个「有命中」，但**假阳性成灾**。
2. **完整路径形状**（`["media"]["lease_renew_ms"]`）：能消歧，但**漏掉中间变量写法** ⇒
   `media.lease_renew_ms`／`lease_expire_ms` 变「无读者」（**假阴性，危险方向**）。
3. **父键锚形状**（`(?:\["media"\]|\bmedia)\["lease_renew_ms"\]`）：既消歧又不漏。**价值实测**：
   把 `backup#database.path` 从假阳性 [A] 纠正到 **[B]**——它被 `const` 钉死，而 `entry["path"]`
   （artifacts 数组元素的 path）曾被误算作它的读者。
4. **补上 `helper` 形状**：`manifest.rs` 的 `text`／`strings`／`amount` 让 `upgrade-package` 的
   14 个键从「无读者」被救回 [A]（不补这一态，它会被误判成「22 个键只有 7 个有读者」）。

### 2.4 一处**不实陈述的就地订正**（serde 形状）

起初以为 serde 形状的「`struct T` **定义在同一文件**」是用来排除插件 SDK 的 `Manifest` 的。
**实测不成立**：`struct Manifest`（line 76）与 `struct Config`（line 100）**同在**
`plugins/avatar-2d/src/lib.rs`。真正拦下 Manifest 字段的是**判定域**——`T` 的字段名 MUST 与
受判键名**相交**（Manifest 的 `output_width`／`kind`／`model_binding` 都不等于 avatar-2d 的任何受判键）。
⇒ 已在 design／spec／ADR 就地订正：Scenario 标题改为
`A struct decoded in the same file is not automatically this schema's reader`，「同文件」作为**安全方向的
收紧**保留。

### 2.5 三分类实测

扫描面 = **非测试、非生成、已跟踪**源码（**205 文件**）。

| 分类 | 数量 |
| --- | --- |
| 受判键 | **96** |
| [A] 有读取点 | **49** |
| [B] 无读取点、值被 `const` 钉死 | **13** |
| [C] 无读取点、也未被钉死 | **34** |

形状分布：**serde 7 / index 28 / helper 14**。

**关键回归**：`runtime` 的 37 键在扩面后 = **10 / 4 / 23**，与 **W90 逐项一致** ⇒
**扩面没有改变既有判定**（已写成测试断言 `[37, 10, 4, 23]`）。

### 2.6 [C] 的成因（8 条，每条逐条实测）

| 成因 | 条数 | 实测证据 |
| --- | --- | --- |
| `constructor-injected` | 12 | 同 W90：构造器/参数注入，调用方在生产代码里不存在 |
| `hardcoded-default` | 1 | `Session::new` 写死 `SessionMode::Assisted` |
| `surface-absent` | 7 | W90 的 5 + `provider.region`／`provider.data_policy_ref`（非测试非生成 `.rs` **零命中**） |
| `same-name-other-object` | 6 | `cloud.*` 3 + `provider.timeout_ms`／`max_concurrency`（命中 `nai-kernel/src/evaluate/skill.rs` 的**技能**字段）、`provider.max_retries`（命中 `nai-knowledge-core/src/remote.rs` 的**知识源**字段） |
| `binding-consumer-absent` | 1 | `ConfigKind` 只有 `Runtime`/`Provider`；`capability_bindings` 表在生产代码里只被测试提及 |
| `awaiting-decision` | 1 | `session_defaults.viewer_memory_enabled`（ADR 0067） |
| **`validator-only`** | 5 | `binding-plan` 全 6 键只有 Ajv 校验 + 生成 TS 类型，**无归因文件** |
| **`write-only`** | 1 | `backup.created_at_utc_ms`：`snapshot.rs:222` 写入侧 `crate::utc_ms()` 生成，读侧从不读 |

## 三、裁定

1. **判定单元是 `(schema, keyPath)`**（标签 `schemas/config/<x>.schema.json#<键路径>`）。
2. **归因文件集派生**：`attribution(schema)` = `include_str!` 该 schema 的文件 ∪ 具名外部读者；
   **每条外部读者 MUST 自清理**。实测 `binding-plan` **无**归因文件。
3. **形状三态 MUST 全部实现**（index 精确到父键锚／serde 限同文件 + 判定域／helper 只认读键辅助函数）。
4. **`[A]`/`[C]` 两侧双向**、**[B] 派生自 `const`**、**容器由结构判定**（沿用 W90）。
5. **受判面派生自磁盘**（`schemasOnDisk('schemas/config')`），MUST NOT 是手写清单。
6. **不改运行时行为**：新增 5 份 schema 的死键**只登记**。

## 四、实现

- **重写** `scripts/contracts/config-consumption.mjs`：`CONFIG_SCHEMAS`／`keyLabel`／`guardedKeys`／
  `indexShape`／`readShape`（兼容入口）／`serdeFieldsByFile`／`keyHelpersByFile`／`helperShape`／
  `inScan`／`attributionOf`／`consumersOf`／`readSites`／`EXTERNAL_READERS`／两张台账
  （`WIRED` 49、`UNWIRED` 34 → 成因 id）／`UNWIRED_CAUSES`（8）／`SCAN_EXCLUSIONS`（5）／
  五条判据（`partitionErrors`／`evidenceErrors`／`causeErrors`／`attributionErrors`／`scanExclusionErrors`）。
- **修改** `tests/contracts/schema.test.mjs`：把 W90 那条 test（`every required runtime-config key is
  read, pinned, or registered`）**替换**为 `every required config key is read, pinned, or attributed`
  （6 条非空洞断言 + 注入 F1–F17）；同步更新其上的说明注释块（W90 → W91 扩面）。
- **`cargo xtask` 零改动**（判据住在既有 `engineering_checks` 条目内）。

## 五、非空洞注入（F1–F17 + 6 条非空洞断言全部成立）

**6 条非空洞断言**：① 受判面派生自磁盘（`[...schemas.keys()] === schemasOnDisk('schemas/config')`）；
② 三类各自非空；③ 三种形状都**真的出现**；④ `binding-plan` 是**唯一**无 `include_str!` 消费者的 schema；
⑤ `.json` 凭**语言闭包**在扫描集外；⑥ **两张台账都承重**（清空 `WIRED` 或 `UNWIRED` 任一 ⇒ 必报错）。

**F1–F17 全部成立**（逐条见 `tests/contracts/schema.test.mjs`）。

### F1 与 F12/F13 **手工真跑改前/改后**（`sha256` 逐字一致）

真实改动**三处源码**、跑 `node --test tests/contracts/schema.test.mjs`、再逐字还原：

| 注入 | 真实改动 | 改后判据输出 |
| --- | --- | --- |
| F1 | `crates/nai-storage/src/store.rs`：`["max_buffer_ms"]` → `["max_buffer_ms_"]` | `runtime#media.max_buffer_ms is required by its schema, nothing reads it…` |
| F12 | `plugins/avatar-2d/src/lib.rs`：`max_buffer_samples` → `max_buffer_samples_x`（3 处） | `avatar-2d#max_buffer_samples is required by its schema, nothing reads it…` |
| F13 | `crates/nai-upgrade-core/src/manifest.rs`：`"install_bytes"` → `"install_bytes_x"` | `upgrade-package#install_bytes is required by its schema, nothing reads it…` |

- **改前**：**30 pass / 0 fail**；**改后**：**29 pass / 1 fail**，且失败消息**各只指名那一个键**。
- **还原**：三个文件 `sha256` **逐字等于**改动前（`5fc1b1aa…`／`c4657607…`／`299b2cda…`），复测 **30 / 0**。

## 六、验证

### 6.1 分面

| 面 | 命令 | 结果 |
| --- | --- | --- |
| 契约（Node） | `node --test tests/contracts/schema.test.mjs` | **30 passed / 0 failed**（**30 → 30**，替换而非新增） |
| 判据 harness | `node .w91-check.mjs` | 五条判据 `partitionErrors`／`evidenceErrors`／`causeErrors`／`attributionErrors`／`scanExclusionErrors` **全 `(none)`**；**96 = 49 + 13 + 34**；形状 **serde 7 / index 28 / helper 14** |
| `openspec validate <change> --strict`（归档前） | — | **valid** |

### 6.2 全量门控

`cargo xtask check --profile full` ⇒ **`EXIT=0`**：**91 个测试目标**（`test result:` 行 91 条）、
**862 passed / 0 failed**、`LNK1104` 命中 **0**。

- 该 profile **内含** `cargo fmt --all --check` 与 `cargo clippy --workspace --all-targets --locked -- -D warnings`
  ⇒ 两者随 `EXIT=0` 一并通过；`rustfmt` 另作**独立复跑**亦 `EXIT=0`。
- 门控前先做 `--no-run` 预链接（`cargo test --workspace --locked --exclude nai-xtask --no-run`，
  `CARGO_BUILD_JOBS=1`）**第 5 轮**收敛至 `EXIT=0`（前 4 轮 `exit=101` 折在**不同目标**上 ⇒ 环境级，
  非代码缺陷；`cargo test -p nai-xtask --lib --no-run` 第 1 轮即过）。
- 与 W90 同形：**91 目标 / 862 passed**。本轮新增的是 **Node 契约判据**（**30 → 30，替换**），
  **不进 cargo 目标计数** ⇒ 两侧数字不变属预期，不是「门控没跑到新判据」。

### 6.3 两条 validate

| 命令 | 结果 |
| --- | --- |
| `openspec validate --all --strict`（归档前） | **44 passed / 0 failed**（`change/w91-config-key-attribution` 在册） |
| `openspec validate --all --strict`（归档后） | **43 passed / 0 failed**（change 已移出在册面） |
| `openspec validate --all --strict --archived` | **89 passed / 0 failed**（88 → **89**，含 `change/2026-10-13-w91-config-key-attribution`） |

## 七、CI 复核

**CI_PLACEHOLDER**

## 八、遗留

1. **新增 5 份 schema 里的 [C] 键接线是产品决策**——本轮**只登记**，MUST NOT 读成「已处理」。
2. **`provider` 的 [C] 键有真实张力**：`provider.timeout_ms`／`max_retries`／`max_concurrency` 的
   **同名者**在别处（技能 `skill.rs`、知识源 `remote.rs`）是**真的被读**的 ⇒ 「配置里有这个键」
   MUST NOT 被读成「这三项策略已经生效」。是否接线是**接线轮次**的事。
3. **`validator-only` 不是豁免**：`binding-plan` 由 W73 的文件级守卫覆盖（Ajv 是它的消费者）；
   本轮成因只陈述「**运行时**不读它」。
4. **跨语言（TS／生成面）的键级消费未做**：生成物的字段名是同一份定义的**第二次书写**，不是消费者。
5. **`helper` 形状仍是启发式**：若将来出现别种取键封装（宏、trait method），会被报成「未接线」
   （**假阳性方向**，安全）。真出现时正确的修法是**把那条路径也纳入派生**，MUST NOT 把该键直接搬进 [A]。
6. **[B] 的 13 个键只陈述「值不可能不同」**：这些键是否是**死约束**，本轮**未裁定**。
7. **W90 的遗留 1／3／4／5 未动**（`runtime` 的 [C] 接线、`const` 交叠、基线样例对账、`budgets` 语义）。

## 九、方法论教训

1. **「一个键的读者」不是一个键的属性，是 `(对象, 键)` 的属性。** 裸键名形状在**单对象**上够用
   （W90），一跨对象就**污染**：`["path"]` 在 `crates/nai-plugin-sdk` 里命中，而它是**另一个对象**。
   ⇒ 凡「按名字找引用」的判据，MUST 先问「**这个名字属于谁**」，MUST NOT 把「同名」当「同一个东西」。
2. **「完整路径」看似更严，实则是假阴性机器。** `["media"]["lease_renew_ms"]` 漏掉中间变量写法
   ⇒ 危险方向（把**有主**的键判成**无主**）。**父键锚**（连续索引**或**标识符）才是既消歧又不漏的形状。
   ⇒ **形状的精化方向 MUST 用真样本验证**，不能凭「看起来更精确」。
3. **第三种读取写法（helper）是「假阴性」的典型来源**：`amount(&value, "install_bytes")` 在
   `index` 形状下**零命中**，14 个真有读者的键会被误判成无主。⇒ **少实现一种形状，整族键会安静地掉进
   「无主」而判据仍全绿**——这是本轮最贵的一课。
4. **一处「我以为的限定」被实测证伪**：serde 的「同文件定义」并不能拦下插件 SDK 的 `Manifest`
   （它与 `Config` 同在 `avatar-2d/src/lib.rs`）。真正承重的是**判定域**（字段 ∩ 受判键名）。
   ⇒ **「这个限定在拦什么」MUST 用反例验证**，否则它是一句自我安慰。
5. **回归断言是扩面的安全带**：`runtime` 的 37 键在扩面后 MUST 逐项不变（`[37,10,4,23]`）。
   少这一条，「顺手改了老口径」不可能被发现。
6. **注入要「改真源码」才算数**：F1/F12/F13 三处**真的动文件**、**真的让判据开火**、
   **真的逐字还原**（`sha256` 作证）——比在内存里造桩可信得多。
7. **只登记不等于逃避**（沿用 W90）：判据的价值不在于让门控变绿，而在于**新增死键会响**、
   每条在册状态**过期即响**。存量 [C] 是被记录的事实，不是被批准的豁免。

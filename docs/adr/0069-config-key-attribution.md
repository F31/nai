# ADR 0069：配置键的归因——读取点 MUST 绑定到拥有它的 schema

- 状态：接受
- 日期：2026-10-13
- 相关：[ADR 0068](0068-config-surface-key-consumption.md)（W90：受判 `required` 键的三分类闭世界，
  **只覆盖 `runtime` 一份 schema**、读取形状是**裸键名**）；[ADR 0051](0051-*.md)／
  [ADR 0052](0052-*.md)／[ADR 0053](0053-repository-scoped-evidence.md)（W73–W75 的**文件级**守卫）；
  [ADR 0067](0067-viewer-memory-single-answering-point.md)（`viewer_memory_enabled` 的未决接线）；
  `schemas/config/*.schema.json`、`crates/nai-storage/src/store.rs`、`crates/nai-storage/src/snapshot.rs`、
  `crates/nai-runtime/src/composition.rs`、`apps/engine/src/pipeline.rs`、
  `crates/nai-upgrade-core/src/manifest.rs`、`plugins/avatar-2d/src/lib.rs`、
  `scripts/contracts/config-consumption.mjs`、`tests/contracts/schema.test.mjs`

## 背景

W90 把「配置面 `required` 键 MUST 有消费者」下推成**键级**的门控（ADR 0068），但只覆盖
`schemas/config/runtime.schema.json` **一个**文件。W73/W74 的文件级守卫覆盖 `schemas/` **全部** schema。
**中间一层空着**：`schemas/config/` 下其余 5 份 schema 的**键**——`avatar-2d` 9、`backup` 10、
`binding-plan` 6、`provider` 12、`upgrade-package` 22，共 59 个——没有任何东西核对
「运行时看不看得见它的值」。

**不能把 W90 原样扩面**，理由在第一步就是实测的：W90 的读取形状是**裸键名** `EXPR["<key>"]`，
**不带对象上下文**。96 个受判键里 3 个裸名跨 schema 重名（`schema_version` ×5、`mode` ×3、
`currency` ×2），通用名词还会**跨对象污染**——实测 `["sample_rate"]` 命中 `apps/render-worker`、
`["session_id"]` 命中 `crates/nai-control-api`、`["path"]`／`["sha256"]`／`["id"]` 命中
`crates/nai-plugin-sdk`，全是**同名不同物**。⇒ 照 W90 的规则，这些键会**凭空获得读者**。

同时**读取写法不止一种**（这是 W90 遗留 2 直接点名的情形）。实测三种，每种都能骗过另一种形状：

| 写法 | 样本 | 裸键名形状能否扫到 |
| --- | --- | --- |
| 直接索引 | `manifest["database"]["sha256"]`（`snapshot.rs`） | 能 |
| **中间变量** | `let media = &config["media"]; media["lease_renew_ms"]`（`composition.rs`） | 能（裸名），但**完整路径形状扫不到** |
| **辅助函数** | `text(&value, "package_id")` / `amount(&value, "install_bytes")`（`manifest.rs`） | **不能**（键名是字符串实参，没有 `["…"]`） |
| **结构体解码** | `let config: Config = serde_json::from_value(...)`（`avatar-2d`） | **不能**（键名是结构体字段） |

## 证据（本轮实测，三轮迭代，每轮被实测推翻一次）

1. **裸键名形状**（W90 原样扩面）：96 键中 31 个「有命中」，但**假阳性成灾**。
2. **完整路径形状**（`["media"]["lease_renew_ms"]`）：能消歧，但**漏掉中间变量写法** ⇒
   `media.lease_renew_ms`／`lease_expire_ms` 变「无读者」（**假阴性，危险方向**）。
3. **父键锚形状**（`(?:\["media"\]|\bmedia)\["lease_renew_ms"\]`）：既消歧又不漏。实测把
   `backup#database.path` 从假阳性 [A] 纠正到 **[B]**——它被 `const` 钉死，而 `entry["path"]`
   （artifacts 数组元素的 path）曾被误算作它的读者。

最终口径：**96 = [A] 49 + [B] 13 + [C] 34**；形状分布 **serde 7 / index 28 / helper 14**。

**归因文件集**（派生）：`attribution(schema)` = Rust `include_str!` 该 schema 的文件 ∪ **具名外部读者**
（经 `composition.config()` 拿 `Value`、因而不 `include_str!`）。实测 `avatar-2d → plugins/avatar-2d/src/lib.rs`、
`backup → snapshot.rs`、`provider`/`runtime → store.rs`、`upgrade-package → manifest.rs`、
**`binding-plan` → 无**；`runtime` 另加两条外部读者 `composition.rs` 与 `pipeline.rs`——正是 W90 的 [A] 里
**唯二**跨出 `store.rs` 的条目。

**回归**：`runtime` 的 37 键在扩面后 = **10 / 4 / 23**，与 W90 **逐项一致** ⇒ 扩面**没有**改变既有判定。

### [C] 的成因（8 条，每条实测）

| 成因 | 条数 | 说明 |
| --- | --- | --- |
| `constructor-injected` | 12 | 同 W90：构造器/参数注入，而调用方在生产代码里不存在 |
| `hardcoded-default` | 1 | `Session::new` 写死 `SessionMode::Assisted` |
| `surface-absent` | 7 | W90 的 5 条 + `provider.region`、`provider.data_policy_ref`（非测试非生成 `.rs` **零命中**） |
| `same-name-other-object` | 6 | `cloud.*` 3 条 + `provider.timeout_ms`／`max_concurrency`（命中 `nai-kernel/src/evaluate/skill.rs` 的**技能**字段）、`provider.max_retries`（命中 `nai-knowledge-core/src/remote.rs` 的**知识源**字段） |
| `binding-consumer-absent` | 1 | `ConfigKind` 只有 `Runtime`/`Provider`；`capability_bindings` 表在生产代码里只被测试提及 |
| `awaiting-decision` | 1 | `session_defaults.viewer_memory_enabled`：接线是**未决的产品决策**（ADR 0067） |
| **`validator-only`** | 5 | `binding-plan` 全 6 键只有 Ajv 校验 + 生成 TS 类型，**无归因文件** |
| **`write-only`** | 1 | `backup.created_at_utc_ms`：`snapshot.rs:222` 在**写入**侧用 `crate::utc_ms()` 生成，读侧从不读 |

## 决策

1. **判定单元是 `(schema, keyPath)`**（标签 `<schema 路径>#<键路径>`），MUST NOT 是裸键名。
   `schema_version` 在 5 份 schema 的顶层同形 ⇒ 「同名」MUST NOT 被当作「同一个键」。
2. **形状三态（index／serde／helper）MUST 全部实现**：只做 index 会把 `avatar-2d` 的 7 个键与
   `upgrade-package` 的 14 个键误判成「无读者」（假阴性）。少一种形状，判据全绿而那一族键整体掉进「无主」。
3. **index 形状 MUST 精确到父键锚**：顶层要求 `[` 前是**表达式字符**（沿用 W90，排除 Rust 数组字面量）；
   嵌套要求紧邻锚是 `["<parent>"]`（连续索引）**或** `<parent>`（标识符 = 中间变量写法）。
   `parent` = 键路径**倒数第二段**（**派生**，MUST NOT 写死键名）。裸名会把 `entry["path"]` 算作
   `database.path` 的读者（实测）。
4. **serde 形状 MUST 限定两点**：`T` 由 `let x: T = serde_json::from_value(...)`（或 `from_*::<T>`）
   解码，且 **`struct T` 定义在同一文件**；判定域是 **`T` 的字段名 ∩ 该 schema 的受判键名**。
   ⚠️ **实测订正**：起初以为「同文件定义」是用来排除插件 SDK 的 `Manifest` 的，**不成立**——
   `struct Manifest`（line 76）与 `struct Config`（line 100）**同在** `plugins/avatar-2d/src/lib.rs`。
   真正拦下 Manifest 字段的是**判定域**（其 `output_width`/`kind`/`model_binding` 都不等于任何受判键）。
   「同文件」作为**安全方向的收紧**保留。
5. **helper 形状只认「读键辅助函数」**：函数体含**变量下标** `x[key]`（`[` 后是标识符），**或**传递调用
   已判定的辅助函数（实测 `amount` → `text`，固定点迭代）。MUST NOT 推广成「任何带字符串实参的调用」
   ——否则 `error(Code, "message")` 之类会凭空造出读者。
6. **归因集 MUST 自清理**：外部读者声明后若不再读该 schema 的任何受判键，判据开火（归因集不是豁免）；
   指向非受判 schema、已在 `include_str!` 集、不在扫描集都要报。
7. **`[A]` 与 `[C]` 两侧双向**（沿用 W90）：有读取点的键 MUST 具名在册、在册的键 MUST 真的没有读取点。
8. **[B] 派生自 schema 的 `const`**、**容器由结构判定**（`type === 'object'` 且有非空 `required`），
   都 MUST NOT 手写清单。
9. **不改运行时行为**：新增 5 份 schema 的死键**只登记**。这是本 ADR 与「接线轮次」的分界线。
10. **扫描面沿用 W90 的排除**；实测 Rust 生成面 `crates/nai-contracts/src/generated/` 对 index 形状
    **零命中** ⇒ **不设具名排除**（排除表每条必须承重，将来真命中再加）。

## 后果与实施

- **重写** `scripts/contracts/config-consumption.mjs`：多 schema 派生（`CONFIG_SCHEMAS`）+ 归因集
  （`consumersOf`／`EXTERNAL_READERS`／`attributionOf`）+ 三态形状（`indexShape`／`serdeFieldsByFile`／
  `keyHelpersByFile`／`helperShape`）+ 五条判据（`partitionErrors`／`evidenceErrors`／`causeErrors`／
  `attributionErrors`／`scanExclusionErrors`）+ 两张台账（`WIRED` 49、`UNWIRED` 34 → 成因 id）+
  成因表（8 条）+ 排除表（5 条）。
- **修改** `tests/contracts/schema.test.mjs`：把 W90 那条 test 换成扩面版（`every required config key is
  read, pinned, or attributed`），含 6 条非空洞断言 + 注入 F1–F17。
- **门控**：`tests/contracts/schema.test.mjs` **30 → 30**（**替换而非新增**）；`cargo xtask` **零改动**。
- **不改运行时行为**：24 个新增 [C] 键只登记。

## 遗留

1. **新增 5 份 schema 里的 [C] 键接线是产品决策**——MUST NOT 读成「已处理」。
   `validator-only`（`binding-plan` 5 键）说明「Ajv 校验过」**不等于**「运行时会读」。
2. **`provider` 的 [C] 键有真实张力**：`provider.timeout_ms`／`max_retries`／`max_concurrency` 的**同名者**
   在别处（技能、知识源）是**真的被读**的 ⇒ 「配置里有这个键」MUST NOT 被读成「这三项策略已经生效」。
   是否要把配置面的 `provider.*` 接到那些消费点，是**接线轮次**的事。
3. **跨语言（TS／生成面）的键级消费未做**：生成物的字段名是同一份定义的**第二次书写**，不是消费者。
4. **`helper` 形状仍是启发式**：只认「函数体含变量下标」这一族写法；若将来出现别的取键封装
   （宏、trait method），会被报成「未接线」（**假阳性方向**，安全）。真出现时正确的修法是把那条
   路径**也**纳入派生，MUST NOT 把该键直接搬进 [A] 台账了事。
5. **`binding-plan` 的 `validator-only` 不是豁免**：它由 W73 的文件级守卫覆盖（Ajv 是它的消费者）；
   本轮的成因只陈述「运行时不读它」。
6. **[B] 的 13 个键只陈述「值不可能不同」**：`const` 钉死 ⇒ 无需读者。这些键是否是**死约束**
   （写了也没意义），本轮**未裁定**。

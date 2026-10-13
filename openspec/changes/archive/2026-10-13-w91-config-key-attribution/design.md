# w91 设计：配置键的归因

## 一、问题的形状

W90 立了「配置面键级消费」的门控，但只覆盖 `schemas/config/runtime.schema.json` **一个**。W73/W74 的
文件级守卫覆盖 `schemas/` 全部 schema。**中间一层空着**：`schemas/config/` 下其余 5 份 schema 的
**键**——`avatar-2d` 9、`backup` 10、`binding-plan` 6、`provider` 12、`upgrade-package` 22，共 59 个——
没有任何东西核对「运行时看不看得见它的值」。

把 W90 的判据**原样扩面**在第一步就崩：W90 的读取形状是**裸键名** `EXPR["<key>"]`，它**不带对象
上下文**。96 个受判键里有 3 个裸名跨 schema 重名（`schema_version` ×5、`mode` ×3、`currency` ×2），
通用名词还会**跨对象污染**——实测 `["sample_rate"]` 命中 `apps/render-worker`、`["session_id"]` 命中
`crates/nai-control-api`、`["path"]`/`["sha256"]`/`["id"]` 命中 `crates/nai-plugin-sdk`。
⇒ 照 W90 的规则，这些键会**凭空获得读者**。

同时**读取写法不止一种**。实测三种，每种都能骗过另一种的形状：

| 写法 | 样本 | 裸键名形状能否扫到 |
| --- | --- | --- |
| 直接索引 | `manifest["database"]["sha256"]`（`snapshot.rs`） | 能 |
| **中间变量** | `let media = &config["media"]; media["lease_renew_ms"]`（`composition.rs`） | 能（裸名），但**完整路径形状扫不到** |
| **辅助函数** | `text(&value, "package_id")` / `strings(&value, "hosts")`（`manifest.rs`） | **不能**（键名是字符串实参，没有 `["…"]`） |
| **结构体解码** | `let config: Config = serde_json::from_value(...)`（`avatar-2d`） | **不能**（键名是结构体字段） |

## 二、实测（三轮迭代，每轮都被实测推翻）

1. **裸键名形状**（W90 原样扩面）：96 键中 31 个"有命中"，但**假阳性成灾**——
   `ai`/`id`/`path`/`bytes`/`model`/`kind`/`os`/`runtime` 等通用名互相污染。
2. **完整路径形状**（`["media"]["lease_renew_ms"]`）：能消歧，但**漏掉中间变量写法**
   ⇒ `media.lease_renew_ms` / `media.lease_expire_ms` 变成"无读者"（**假阴性，危险方向**）。
3. **父键锚形状**（`(?:\["media"\]|\bmedia)\["lease_renew_ms"\]`）：既消歧又不漏。
   实测把 `backup#database.path` 从假阳性 [A] 纠正到 **[B]**——它其实被 `const` 钉死，
   而 `entry["path"]`（artifacts 数组元素的 path）曾被误算作它的读者。**这就是形状精确性的价值。**

最终口径：**96 = [A] 49 + [B] 13 + [C] 34**。

## 三、三态形状

- **`index`**：`EXPR["<key>"]`。顶层键要求 `[` 前是**表达式字符**（沿用 W90，排除 Rust 数组字面量）；
  **嵌套键**要求紧邻的锚是 `["<parent>"]`（连续索引）**或** `<parent>`（标识符，即中间变量写法）。
  `parent` = 键路径的**倒数第二段**（派生，MUST NOT 写死键名）。
- **`serde`**：归因文件里 `let x: T = serde_json::from_value(...)`（或 `from_*::<T>`），且
  **`struct T` 定义在同一个文件** ⇒ `T` 的字段名 ∩ 该 schema 的受判键名 算有读取点。
  ⚠️ **实测订正**：起初以为「同文件定义」是用来排除插件 SDK 的 `Manifest` 的，实测**不成立**——
  `struct Manifest`（line 76）与 `struct Config`（line 100）**同在** `plugins/avatar-2d/src/lib.rs`。
  真正拦下 Manifest 字段的是**判定域**：`T` 的字段名 MUST 与受判键名**相交**
  （Manifest 的 `output_width` / `kind` / `model_binding` 都不等于 avatar-2d 的任何受判键）。
  ⇒ 两个限定都保留：「同文件」是**安全方向的收紧**（跨文件定义的 SDK 类型不是本 schema 的解码目标），
  「字段 ∩ 受判键名」才是判定域本身。
- **`helper`**：归因文件里定义的「**读键辅助函数**」——函数体含**变量下标** `x[key]`
  （`[` 后是标识符），**或**传递调用已判定的辅助函数（实测 `text`/`strings`/`amount`）；
  以字符串字面量为实参调用它（`amount(&value, "install_bytes")`）算一次键读取。
- **`validator-only` 不是形状**，是 [C] 的**成因**：只有 Ajv 校验、没有任何运行时读取（实测 `binding-plan`）。

## 四、归因文件集

读取点 MUST 绑定到 schema：
`attribution(schema)` = Rust `include_str!` 该 schema 的文件（**派生**）∪ **具名外部读者**
（经 `composition.config()` 拿 `Value`、因而不 `include_str!` 的文件）。

实测：`avatar-2d → plugins/avatar-2d/src/lib.rs`、`backup → snapshot.rs`、
`provider`/`runtime → store.rs`、`upgrade-package → manifest.rs`、**`binding-plan` → 无**。
`runtime` 另加两条外部读者 `composition.rs`（读 `max_active_sessions`/`lease_*`）与
`pipeline.rs`（读 `allow_auto_without_platform_grant`）——这两条正是 W90 的 [A] 里**唯二**跨出
`store.rs` 的条目。每条外部读者 MUST **自清理**（文件里 MUST 真有该 schema 受判键的读取点）。

## 五、三分类

```
受判键 96（= 106 个 required − 10 个「有 required 子键的容器」）
[A] 有读取点 49 = avatar-2d 7(serde) + backup 5 + provider 6 + runtime 10 + upgrade-package 21(index 7 + helper 14)
[B] 无读取点、值被 const 钉死 13
[C] 无读取点、也未被钉死 34
```

- **回归**：`runtime` 的 37 键在扩面后 = **10 / 4 / 23**，与 W90 **逐项一致** ⇒ 扩面**没有**改变既有判定。
- **`upgrade-package` 的 21 个读者里 14 个是 `helper`**：不把辅助函数算进来，它会被误判成
  「22 个键只有 7 个有读者」——那是**假阴性**，正是本轮要堵的方向。

## 六、[C] 的成因（8 条，每条逐条实测）

| 成因 | 条数 | 实测证据 |
| --- | --- | --- |
| `constructor-injected` | 12 | 同 W90：构造器/参数注入，而调用方在生产代码里不存在 |
| `hardcoded-default` | 1 | `Session::new` 写死 `SessionMode::Assisted`（`nai-kernel/src/session.rs:61`） |
| `surface-absent` | 7 | W90 的 5 条 + `provider.region`、`provider.data_policy_ref`（非测试非生成 .rs **零命中**） |
| `same-name-other-object` | 6 | W90 的 `cloud.*` 3 条 + `provider.timeout_ms`（命中 `nai-kernel/src/evaluate/skill.rs` 的**技能**超时）、`provider.max_retries`（命中 `nai-knowledge-core/src/remote.rs` 的**知识源**重试）、`provider.max_concurrency`（命中 `skill.rs` 的**技能**并发） |
| `binding-consumer-absent` | 1 | `ConfigKind` 只有 `Runtime`/`Provider`；`capability_bindings` 表在生产代码里只被测试提及 |
| `awaiting-decision` | 1 | `session_defaults.viewer_memory_enabled`：接线是**未决的产品决策**（ADR 0067） |
| **`validator-only`** | 5 | `binding-plan` 全 6 键只有 Ajv 校验（`scripts/contracts/validation.mjs` 的 `AJV_SOURCES`）+ 生成 TS 类型，**无任何运行时读取**；该 schema **没有归因文件** |
| **`write-only`** | 1 | `backup.created_at_utc_ms`：`snapshot.rs:222` 在**写入**侧用 `crate::utc_ms()` 生成，读侧从不读 |

## 七、决策

1. **判定单元是 `(schema, keyPath)`**，MUST NOT 是裸键名。`schema_version` 在 5 份 schema 的顶层同形
   ⇒ 「同名」MUST NOT 被当作「同一个键」。
2. **形状三态（index/serde/helper）MUST 全部实现**：只做 index 会把 `avatar-2d` 的 7 个键与
   `upgrade-package` 的 14 个键误判成"无读者"（假阴性）。
3. **index 形状 MUST 精确到父键锚**：裸名会把 `entry["path"]` 算作 `database.path` 的读者（实测）。
4. **serde 形状 MUST 限定两点**：`struct T` **定义在同一文件**，且 **`T` 的字段名 ∩ 受判键名**
   （判定域）。⚠️ **实测订正**：起初以为「同文件」是用来排除插件 SDK 的 `Manifest` 的，实测**不成立**
   ——`struct Manifest`（line 76）与 `struct Config`（line 100）**同在** `plugins/avatar-2d/src/lib.rs`；
   真正承重的是**判定域**，「同文件」是其**安全方向的收紧**。
5. **归因集 MUST 自清理**：外部读者声明后若不再读，判据开火（归因集不是豁免）。
6. **`[A]` 与 `[C]` 两侧双向**（沿用 W90）：有读取点的键 MUST 具名在册、在册的键 MUST 真的没有读取点。
7. **[B] 派生自 schema 的 `const`**，MUST NOT 手写。
8. **容器由结构判定**（`type === 'object'` 且有非空 `required`），MUST NOT 写成键名清单。
9. **不改运行时行为**：新增 5 份 schema 的死键**只登记**。
10. **扫描面沿用 W90 的排除**；实测 Rust 生成面（`crates/nai-contracts/src/generated/`）
    **零命中**裸名形状 ⇒ **不设具名排除**（排除表每条必须承重；将来真命中再加）。

## 八、注入

| 编号 | 注入 | 期望 |
| --- | --- | --- |
| F1 | 挖掉**真实源码文本**里 `store.rs` 的 `["max_buffer_ms"]` | 该键从 [A] 掉出并被**指名**（消息唯一，`deepEqual` 钉死） |
| F2 | [A] 的证据文件改成**不读**该键的文件 | 报「命名的读者不再读」 |
| F3 | 在册**遮蔽真读者**（把一个其实有读取点的键登记为未接线） | 报「shadows a real reader」 |
| F4 | **陈旧在册**（schema 里没有该键） | 报错——台账 MUST 自清理 |
| F5 | 缺成因／成因不在成因表里 | 两条都报 |
| F6 | 把一个**被钉死**的键登记为未接线 | 报「pins its value」 |
| F7 | 新增一个 `required` 键、不接线也不登记 | 立刻被指名（「不许新增死键」） |
| F7b | **新增一份 config schema** 却把它留在受判面之外 | 受判面**派生自磁盘** ⇒ 立刻红 |
| F8 | **取消钉死**（`const` → `boolean`） | 掉出 [B] 并被指名 ⇒ 证明 [B] 是**派生**的 |
| F9 | **容器边界**：有 `required` 子键的容器不受判；无 `required` 子键的 object 键 MUST 自己受判 | 三条（含 `empty_bag`） |
| F10 | Rust **数组字面量** `&["knowledge"]` vs 真索引 | 只认真索引 |
| F11 | **父键锚**：`entry["path"]` MUST NOT 算 `database.path` 的读者；**中间变量**写法 MUST 仍算 | 两条 |
| F12 | **serde 判定域**：`Manifest` 字段在解码集里但 MUST NOT 算 avatar-2d 的读者；逐键钉死 [A] = 7 字段 | `deepEqual`（多一个少一个都红） |
| F13 | **helper 形状**：`index` 形状对 `manifest.rs` **零命中**；`amount(…, "install_bytes")` 算读取；非读键调用的字符串实参 MUST NOT 算 | 三条 |
| F14 | **归因集外的裸名命中**（`binding-plan#session_id` ← `crates/nai-control-api`） | MUST NOT 算作它的读者 |
| F15 | **归因集自清理**：外部读者声明后不再读／指向非受判 schema | 报陈旧归因 |
| F16 | **整行注释**里的读取 | MUST NOT 算（`configSources` 已按语言去注释） |
| F17 | **陈旧排除**（匹配不到任何文件） | 指名 |

**F1 与 F12/F13 另做「真跑改前/改后」**：真实改动 `store.rs`／`lib.rs`／`manifest.rs` ⇒
改前 **30 pass / 0 fail**、改后 **29 pass / 1 fail** 且**各只指名那一个键**；还原后 `sha256` 逐字一致、复测 30/0。

## 九、明确不做

- **不接线** 24 个新增 [C] 键里的任何一个（接线是产品决策）。
- **不做跨语言（TS/生成面）的键级消费**：生成物的字段名是同一份定义的第二次书写，不是消费者。
- **不把 `helper` 判据推广成"任何函数调用"**：只认**函数体含变量下标**（或传递调用）的函数
  —— 否则 `error(Code, "message")` 之类会凭空造出读者。
- **不给 `binding-plan` 设具名例外**：它由 W73 的文件级守卫覆盖（Ajv 消费）；本轮的 `validator-only`
  只说明**运行时不读它**，是**事实登记**而非豁免。

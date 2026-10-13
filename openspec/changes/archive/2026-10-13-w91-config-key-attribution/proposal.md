# w91：配置键的归因 —— 读取点 MUST 绑定到拥有它的 schema

## Why

W90 把「配置面**键级**消费」立成了门控，但它只覆盖 `schemas/config/runtime.schema.json` **一个文件**：
37 个受判键、10 个有读者。W73/W74 的文件级守卫则覆盖 `schemas/` 下全部 schema（被消费或具名）。
⇒ 中间那一层——**`schemas/config/` 下其余 5 份 schema 的键**——仍然没人管：`avatar-2d` 的 9 个键、
`provider` 的 12 个、`upgrade-package` 的 22 个、`backup` 的 10 个、`binding-plan` 的 6 个。

把 W90 的判据**原样扩面**会在第一个文件上就崩：W90 的读取形状是**裸键名** `EXPR["<key>"]`，
它**不带对象上下文**。实测扩面后 96 个受判键里有 3 个裸名跨 schema 重名（`schema_version` ×5、
`mode` ×3、`currency` ×2），且通用名词会**跨对象污染**——`["path"]` / `["sha256"]` / `["id"]` 命中
`crates/nai-plugin-sdk`，`["sample_rate"]` 命中 `apps/render-worker`，`["session_id"]` 命中
`crates/nai-control-api`，全都是**同名不同物**。⇒ 照 W90 的规则，这些键会**凭空获得读者**，
而[真读者]与[别人的同名键]在扫描结果里长得一模一样。

同时，读取的**写法**也不止一种。实测 `plugins/avatar-2d/src/lib.rs` 走
`let config: Config = serde_json::from_value(...)`（结构体解码），`value["<key>"]` 索引一个都扫不到
⇒ W90 §八遗留 2 预告的假阳性**真的出现了**，且它有 7 个键。

## What Changes

- **扩面**：受判键集合从「runtime 一份」扩到 **`schemas/config/` 下每份 schema**（6 份、96 个受判键）。
- **归因**：读取点 MUST 通过**归因文件集**绑定到 schema —— 该集合 = Rust `include_str!` 该 schema
  的文件（派生）∪ **具名外部读者**（靠 `composition.config()` 拿 `Value`、因而不 `include_str!` 的文件）。
  每条外部读者 MUST 自清理（文件里 MUST 真有该 schema 受判键的读取点）。
- **三态形状**：`index`（`EXPR["<key>"]`，**只在归因集内计**；嵌套键锚到父键）、`serde`（归因文件里被
  `serde` 反序列化、且 **`struct` 定义在同一文件** 的结构体，其字段名 ∩ 该 schema 的受判键名）与
  `helper`（**读键辅助函数**：函数体含变量下标、或传递调用已判定者；以字符串实参调用算一次读取）。
- **同名不同物具名化**：跨 schema 重名的裸键名 MUST NOT 互相充当读者；这一类在 [C] 里记成因
  `same-name-other-object`（实测：`binding-plan#session_id` 的裸名在归因集外命中 `nai-control-api`）。
- **[C] 新增成因** `validator-only`：只有 Ajv 校验、**没有任何运行时读取**的键（实测 `binding-plan` 全覆盖）。

## Impact

- `scripts/contracts/config-consumption.mjs` 扩为多 schema（派生 + 归因 + 台账 + 五条判据）。
- `tests/contracts/schema.test.mjs` 扩既有那条 test（**`xtask` 零改动**）。
- **不改任何运行时行为**：新增的 5 份 schema 的死键**只登记，不接线**（接线是产品决策）。
- 回归：`runtime` 的 37 个键在扩面后**逐项不变**（10/4/23）—— 这是「扩面没有改变既有判定」的证据。
- 门控效果：任一份 `schemas/config/*` 新增 `required` 键而无归因读者、未被钉死、又未在册 ⇒ **门控红**。

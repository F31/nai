# w90：配置面的键级消费状态（受判 required 键的三分类闭世界）

## Why

W89 的 F1 抓到的形状是「**同层字段一个有主、一个无主**」：`schemas/config/runtime.schema.json` 的
`session_defaults` 是 `additionalProperties:false` 且 `required` 明列三个键，而全仓非测试代码里
`composition.config()["session_defaults"]` **只被读一次**——读的是 `allow_auto_without_platform_grant`，
`viewer_memory_enabled` 一个读者都没有。W89 把那条对照当作**判据**用了（同层有一个无主字段 ⇒ 可判），
但**没有**回答「这个配置面上一共有多少个这样的键」。

W73 的守卫管的是**文件级**：每份 `*.schema.json` 要么被执行体消费、要么具名例外。**键级**没有守卫，
于是「这个键有人看吗」只能靠一次一次性的人工核实——而 W89 就是这么做的。这类核实**会过期**：
下次有人加一个 `required` 键，没有任何东西会告诉他「运行时看不见它」。

实测（扫描非测试、非生成、已跟踪源码，读取形状 `EXPR["<key>"]`）：

| 分类 | 数量 |
| --- | --- |
| `required` 键总数 | 45（其中 8 个是**有 `required` 子键的容器**，其消费由子键蕴含） |
| 受判键 | **37**（36 叶键 + `bindings`——唯一「无 `required` 子键的 object 键」） |
| [A] 有读取点 | **10** |
| [B] 无读取点但值被 `const` 钉死 | **4** |
| [C] 既无读取点、也未被钉死 | **23** |

⇒ `viewer_memory_enabled` **不是特例，是 23 分之 1**。改这 23 个键的值**不改变任何行为**。

## What Changes

把文件级守卫下推**一层**，落到**键级**，形状沿用既有纪律（在册 == 派生 ∪ 台账）：

- **受判键集合派生**：`required` 键中减去「自己有 `required` 子键的容器」。容器由**结构**判定
  （`type === 'object'` 且有非空 `required`），MUST NOT 写成键名清单；一个 `required` 的 object 键
  若**没有** `required` 子键，它不能被任何子键蕴含 ⇒ 必须自己受判（`bindings` 就是这一条）。
- **三分类双向闭世界**：`[A] 有读取点 ∪ [B] 值被 const 钉死 ∪ [C] 逐键在册` 两两不相交、
  并集恰等于受判键集合。**[A] 与 [C] 两侧都是双向的**——有读取点的键 MUST 具名在册（观测不能替人签名）、
  在册的键 MUST 真的没有读取点（签名不能遮住观测）。
- **读取形状只认一种写法**：`EXPR["<key>"]`（`[` 前必须是表达式字符）。实测 `xtask/src/deps.rs` 的
  `&["knowledge"]`（Rust 数组字面量）会骗过宽松正则，让一个零读者的容器键凭空获得读者。
  ⇒ **假阳性方向**安全（漏判「有人读」比误判「没人读」危险）。
- **扫描面按结构排除，且每条排除付理由**：测试面（按**路径段**，含 `crates/*/tests/`、`src/tests.rs`）、
  生成面（`packages/contracts-ts/src/generated/`）、判据自身。定义面（schema 自己）**不靠具名排除**——
  `.json` 不在语言表里，它凭语言闭包就在扫描集之外；测试侧另有一条断言钉住这件事。
- **两层台账手写、逐键具名**：[A] 的键 → **证据文件**（自清理：文件里 MUST 真的含该键的读取形状）、
  [C] 的键 → **成因 id**（成因表每条 MUST 可核实、且 MUST 至少被一个键引用）。

## Impact

- 新增 `scripts/contracts/config-consumption.mjs`（纯函数：派生 + 台账 + 四条判据）。
- `tests/contracts/schema.test.mjs` 新增一条 test（含 12 条注入），**`xtask` 零改动**
  （该文件已在 `engineering_checks` 里）。
- **不改任何运行时行为**：23 个死键**只登记，不接线**。接线是产品决策（见 ADR 0068 遗留 1）。
- 门控效果：新增 `required` 键若既无读者、也未被钉死、又未在册 ⇒ **门控红**。

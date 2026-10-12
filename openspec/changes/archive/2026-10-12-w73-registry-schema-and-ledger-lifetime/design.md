# 设计（w73 注册表形状契约接电与台账存活期）

## 1 注册表如何被校验

**载体选择**：断言落在 `tests/registry.test.mjs`（注册表本体）与 `tests/contracts/schema.test.mjs`
（`schemas/` 全集）。两者都已在 `xtask/src/lib.rs` 的 `engineering_checks` 内
（`registry::check` 与 `schema::node`），故自动进入 `portable` 与 `full`，**不需要新的 xtask 步骤**。

**为什么注册表不走 `valid()` 那套 JSON 夹具**：`valid()` 校验的是 `tests/fixtures/contracts/*.json`
的**测试夹具**；注册表是 **YAML** 且是**生产数据**。因此：

- `validation.mjs` 的 `sources` 原样保留（夹具侧契约），**另导出** `AJV_SOURCES`，
  把「有哪些 schema」变成可被守卫读取的**单一清单**；
- 注册表用 `YAML.parse`（`yaml@2.9.1`，`registry.mjs` 已在用）读入，再交给同一份 Ajv 校验器。

**为什么必须接电而不是删掉**：三条既有规格要求以「schema 校验」为前提；且它比手工检查多一层——
`additionalProperties: false` 会拒绝**未知键**，而手工解析对未知键**静默忽略**
（W72 的 `baseline_deviations` 就是这样在没有被抓的情况下进去的）。

**重叠处的处置**：不删任何一侧。schema 管形状，`registry.mjs` 管语义；两者在
**id 家族 pattern / 发布线枚举 / 状态枚举**三处重叠，加**一致性断言**（逐字比较），
不一致时指名是哪一处。⇒ 重叠允许，漂移不允许。

## 2 台账的 `since_baseline` 与跨版不变式

```
条目 = { id, since_baseline, registered, carried_at?, reason }
不变式：∀ 条目. since_baseline == 当前基线版本
```

- **为什么用「版本」而不是「轮次」**：`reconcile` 已经通过 `readBaseline()` 拿到**当前基线版本**
  （`docs/` 中版本号最大者，且与 `openspec/config.yaml` 的指针一致）；而「某个轮次对应哪个基线版本」
  在仓库里**没有机器可读的映射**（三份升基线 ADR 的文内轮次写法各不一致）。⇒ 用版本，**不需要新的解析面**。
- **为什么不是「N 轮之内」**：轮数是外部观察值，取 3／9／11 都无依据；而升基线轮次是仓库内的离散事件，
  且是**唯一**能改 §57.3 的时刻。锚事件，不锚计数。
- **非空洞的必要性**：`since_baseline` 一旦可选或被忽略，这条不变式就恒真。因此
  `reconcile(registry, parsed, version)` 的**第三个入参是必需的**——缺失或形状非法时
  **具名报错**（`对账缺少基线版本（无法判定台账是否跨版）`），MUST NOT 静默跳过。

## 3 「被消费」的派生口径

守卫 MUST NOT 写死「哪些 schema 被消费」——那会把守卫变成一张会过期的清单。三条派生来源：

| # | 来源 | 形态 | 覆盖的现有实例 |
|---|---|---|---|
| 1 | `validation.mjs` 的 `AJV_SOURCES` 取值 | `schemas/<值>.schema.json` | 11 份 |
| 2 | Rust 的 `include_str!` | 正则 `include_str!\(\s*"[^"]*schemas/([^"]+\.schema\.json)"` | 8 处（含 `config/upgrade-package`，**它只在 Rust 侧被消费**） |
| 3 | `scripts/**`、`tests/**` 的 `*.mjs` 字面量 | 正则 `schemas/([A-Za-z0-9_./-]+\.schema\.json)` | `platform/matrix.mjs` 的 `SCHEMA_PATH` |

⇒ 扫描**只认代码**（`*.mjs`/`*.rs`），**不认数据文件与文档**——否则 `tests/registry.yaml` 的
`notes` 里提一句 `case-set.schema.json` 就会把它算成「被消费」，守卫当场变成瞎的。
（这与平台矩阵的核心门控「文件子集」同形：**判据来自源码扫描，边界来自具名清单**。）

**具名例外恰一条**：`rag-golden/case-set.schema.json`——执行体是 `xtask/src/golden.rs` 的
`parse_case_set`（对每类缺陷产出**稳定 token**），再接一份 JSON Schema 会形成同一形状的第二处判定，
而两份判定的失败**措辞不同** ⇒ 接线只会让「为什么被拒」有两个答案。⇒ **具名声明为「文档陈述」，附理由。**

## 4 非空洞注入（每条守卫 MUST 被单独验证会开火）

| 组 | 注入 | 期望 |
|---|---|---|
| S1 | 注册表加一个未知顶层键 | schema 校验失败 |
| S2 | 某 `acceptance` 条目加一个未知字段 | 失败 |
| S3 | `id` 改成 `B1`（少一位） | 失败，且语义层也失败 |
| S4 | `release_line` 改成 `r4` | 失败 |
| S5 | `status` 改成 `done` | 失败 |
| S6 | `acceptance` 清空 | 失败（`minItems`） |
| S7 | `mapping.d01_to_d14` 删一个 D 键 | 失败 |
| C1 | 把 `ID_RE` 的家族白名单改小（只在内存里比对） | 一致性断言失败并指名 `ID_RE` |
| C2 | schema 的 `release_line.enum` 删一项 | 失败并指名发布线枚举 |
| C3 | schema 的 `status.enum` 加一项 | 失败并指名状态枚举 |
| L1 | 条目缺 `since_baseline` | 失败 |
| L2 | `since_baseline` 写上一个版本（跨版存活） | 失败，报出该条目与两侧版本 |
| L3 | 缺 `version` 入参 | 失败（具名「无法判定」） |
| L4 | `carried_at` ≤ `registered` | 失败 |
| L5 | 结转后（版本更新 + `carried_at`）条目仍相称 | **通过**（逃生通道可用） |
| E1 | 删掉 `AJV_SOURCES` 的一项 | 该 schema 变成孤儿 ⇒ 失败 |
| E2 | 给一份**已被消费**的 schema 也加例外 | 失败 |
| E3 | 例外条目缺理由 | 失败 |

## 5 已知代价与边界

- 门控现在**依赖 schema 文件本身**：改 schema 的形状（如把 `additionalProperties` 放开）会同时弱化 S1/S2。
  缓解：S1/S2 是**注入验证**，改形状会让它们在门控里当场失败。
- 台账的版本判据要求**每次升基线回头处置台账**——这是有意的（把沉默变成记录）；台账为空时成本为零。
- **不做**：注册表之外的形状漂移（`docs/reports/evidence/*` 等无形状契约）、
  §56.2 自身的两处张力（需一次基线修订轮次）、语义类漂移的自动对账（沿用 ADR 0050 遗留 1）。

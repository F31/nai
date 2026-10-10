# W31 实测报告：黄金集载荷载体与就位门禁（B01 机制侧收口）

- 日期：2026-10-10
- 变更：`openspec/changes/w31-golden-set-payload/`
- 决策：[ADR 0033](../adr/0033-golden-set-payload-boundary.md)；指南：[docs/golden-set.md](../golden-set.md)；位置声明：[tests/rag-golden/README.md](../../tests/rag-golden/README.md)
- 判据：`tests/registry.yaml` B01

## 结论

W30 之后，「100 题黄金集未建」仍是一句无落点的陈述：§30.1 指定的 `tests/rag-golden` 目录不存在，§6 的 100 题门槛标注为（设计值）且未指定生成方。本轮给「题从哪里来」一个**机器可寻**的答案：

1. **一处位置**（§30.1 指定的 `tests/rag-golden/*.json`）+ **一份结构规范**（`schemas/rag-golden/case-set.schema.json`）；
2. **一道就位门禁**（`cargo xtask check --golden`），把「从未提供」「结构缺陷」「结构合规但不达标」三种结论**分开**报告；
3. **载荷边界上的确认人要求**：§5.9 的人工确认前移到读取阶段，含未确认候选的题集在参与任何统计之前即被拒。

**本轮不产生任何题内容**：题集是场景包载荷，由场景包产出方按 §5.9 的三条来源填入。构造 100 题示例会把构造数据读成质量水平。

同时修正一处口径：§655 中 B01 的验收是「知识质量：**100 题集达到**第 6 章门槛」＝**实测达标**。题集不存在时不可能达成，因此 registry 的 B01 由 `implemented` **回调为 `blocked`**——已交付的判定机制（W30）与载荷载体（W31）在 notes 中完整保留。

## 目标行为

| 判据 | 实现 | 关键设计 |
| --- | --- | --- |
| §30.1 指定位置 | `PAYLOAD_DIR = "tests/rag-golden"` | 唯一位置，「100 题在哪里」有单一答案 |
| 结构规范 | `schemas/rag-golden/case-set.schema.json` | `additionalProperties: false`；`expectation` 五形、`origin` 两形 |
| 修订协商 | `CASE_SET_SCHEMA = 1` | 声明的修订不认识即拒，不宽松解释 |
| 未提供 ≠ 通过 ≠ 失败 | `PayloadAudit { files, sets, defects }` | `is_unsupplied()` 以 `files == 0` 为**唯一**判据 |
| 缺陷全量报告 | `Report` 收集器 | 每条缺陷带文件名 + JSON 路径 + 稳定 token |
| 缺陷不归因于系统 | `audit_case_set` 在读入阶段拒绝 | 空 `required` / 空 `forbidden_claims` 是题集缺陷，不是运行失败 |
| 确认人在载荷边界 | `parse_origin` 要求非空 `confirmer` | 空白与缺失同罚；未确认候选无法进入统计 |
| 重复用例拒绝 | `GoldenSet::add` + `duplicate_case_id` | 重复用例会在召回比例中被重复计权 |
| 判定逻辑单一真相 | 复用 kernel 的 `GoldenSet`/`audit_coverage` | 不复制类别表、最低题数或覆盖审计 |
| 不干扰工程门控 | 独立命令，不并入 `--profile portable` | 业务题集未建不应让工程基础变红 |

## 测试

`cargo test -p nai-xtask --lib`：**27 passed / 0 failed**（golden **17 项** + 原有 deps 8 + schema 2）。

| 组 | 覆盖 |
| --- | --- |
| 规模与覆盖（3） | 8 题集具名报 `below_minimum_cases: 8 of 100`；100 题覆盖八类无 finding；100 题全同类 ⇒ 恰好 7 条 `category_uncovered` |
| 期望与类别（2） | 不兼容期望报 `expectation_category_mismatch` 并指向用例 id；空 `required` 与空 `forbidden_claims` 均被拒 |
| 修订与字段（3） | 非 1 的 `case_set_schema` 被拒且报实际/期望值；未知顶层字段与空 `cases` 同时报出并带路径；`{` ⇒ 单条 `json_invalid` |
| 词汇表（1） | 未知 `category` 与未知 `expectation.kind` 各自被拒 |
| 确认人（1） | 缺 `confirmer` ⇒ `field_missing`；`"  "` ⇒ `field_empty`；具名 ⇒ 读入成功 |
| 重复 id（1） | `duplicate_case_id` 并报出该 id |
| 目录（6） | 目录不存在 ⇒ `unsupplied()` 且 `check_dir` 返回以 `CAPABILITY_NOT_READY` 开头、含目录名的错误；只读 `.json` 且按名排序（`notes.md` 被忽略）；100 题载荷 `passed()` 且 `check_dir` 通过、8 题载荷以「structural rules」结束；解析失败报「structural defect」而**不是**「未就绪」或「题数不足」；修订不受支持经目录扫描也被拒 |

## 实跑（两种结论确实分开）

```
$ cargo xtask check --golden
golden case set payload: tests/rag-golden
  files: 0
CAPABILITY_NOT_READY: no golden case set under tests/rag-golden; the §6 questions are a
scenario-package payload, supplied per scenario and never fabricated here
EXIT=1

$ cargo xtask check --golden --dir tests/fixtures/rag-golden
golden case set payload: tests/fixtures/rag-golden
  files: 1
  example-min.json [fixture-min] 8 case(s)
    below_minimum_cases: 8 of 100
tests/fixtures/rag-golden: the golden case set does not meet the structural rules (1 finding(s))
EXIT=1
```

第一条是「未提供」，第二条是「读出来了但不达标」——两者都以非零退出，但报道的是**不同的事实**。夹具那 8 例覆盖全部八类（故只有 `below_minimum_cases` 一条 finding），说明类别覆盖判定确实在工作。

## 依赖边界

- `xtask` 新增 `nai-kernel` 依赖：**tool 层允许**（deps.rs 对 tool 层不设禁止规则）。
- `cargo xtask check-deps` ⇒ **18 包**，依赖方向不变；`nai-kernel` 仍只依赖 contracts。
- **外部依赖零新增**：复用 `xtask` 已有的 `serde_json`；`Cargo.lock` 仅新增 1 行（依赖边本身）。

## 门控

- `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=4`）：**EXIT=0**，78 个测试目标（61 `Running` + 17 `Doc-tests`），0 失败，无 `LNK1104`、无编译错误。
  - 目标数与 W30 相同（78）：本轮**没有新增测试目标**——17 项 golden 测试位于 `xtask/src/golden.rs` 的 `#[cfg(test)]`，属于**既有的** `nai-xtask` lib 目标。门控的最后一步正是 `cargo test --package nai-xtask --lib`，实测 **27 passed**（日志末行），因此这些测试确实被工程门控覆盖。
- `cargo test -p nai-xtask --lib`：**27 passed / 0 failed**（golden 17 + 原 deps 8 + schema 2）。
- `cargo clippy -p nai-xtask --all-targets --locked -- -D warnings`：通过（零告警）；`cargo fmt --all` 已跑。
- `cargo xtask check-deps`：**18 包**，依赖方向不变（新增 `xtask → nai-kernel` 属 tool 层允许；`nai-kernel` 仍只依赖 contracts）。
- `cargo xtask check --registry`：通过（B01 由 `implemented` 回调为 `blocked`，理由与证据一致）。
- `openspec validate --all --strict --archived --no-interactive`：**37 passed / 0 failed**；`w31-golden-set-payload --strict` 归档前为 `is valid`，归档时以 `+ 4 added` 并入既有 capability `knowledge-quality-gate`（10 → 14 条需求，**无新 capability**）。

> 注：本轮宿主 `LNK1104` 未出现——预链第 1 次即 `rc=0`，完整门控一次通过（`LNK=0`）。与前几轮相比，这是预链顺序与清理策略稳定后的正常结果。

## 未覆盖边界

- **题集内容未提供**：`tests/rag-golden` 中没有题集文件；`check --golden` 以 `CAPABILITY_NOT_READY` 结束是如实反映。夹具不是真值。
- **真实模型评估与真实检索质量未接**：答案正确性由调用方判定后注入（`AnswerVerdict`），本轮不做自动判分。
- **无自动出题路径**：§5.9 要求坏案例经人工确认；「从日志自动生成题集」会让黄金集自己给自己出题，故不提供。
- **不做评估 UI**；不改 `knowledge-pipeline`（它管检索，本模块管判定与载荷）。
- **schema 未接自动校验**：`schemas/rag-golden/case-set.schema.json` 是结构声明，`cargo xtask schema --check` 只比对由 proto 生成的契约代码，不校验数据文件；载荷的实际校验由 `check --golden` 承担（两者规则一致，但由 Rust 侧实现）。

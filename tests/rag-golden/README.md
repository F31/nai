# RAG 黄金集载荷（tests/rag-golden）

§30.1 把「RAG 黄金」指定为一个独立的测试类，本目录就是它指定的位置。这里是**唯一**允许存放黄金集题集的地方，因此「100 题在哪里」有一个单一答案。

## 题从哪里来

本仓库**不生成**题集内容，也不在代码里硬编码任何一道题。题集是**场景包载荷**：§5.9 列举场景包字段时写的是「角色口吻、产品集合、禁答范围、批准模板、节目节奏、音色与形象、知识来源、预算与**黄金集**」。来源有三条，全部需要人工投入：

| 来源 | 依据 | 谁产出 |
| --- | --- | --- |
| 按场景人工编写 | §6「至少 100 个**贴近真实直播的问题**」；注意整条质量门槛标注为**（设计值）** | 场景包作者 / 产品与知识运营 |
| 随场景包发布 | §5.9 场景包字段含「黄金集」⇒ **逐场景各一份**，不是全局单例 | 场景包作者 |
| 坏案例回流 | §5.9「被改写或驳回的候选进入坏案例池，**经人工确认后**扩充黄金集」 | 值守 / 审核人在场次复盘中确认（具名） |

因此本目录**只在题集就位后**由上述产出方填入。仓库里不存在也**不应**存在一份「示例真值」——那会让人把构造出来的题误读成质量水平。

## 文件格式

- 一个场景一个文件：`<scenario>.json`。
- 结构见 [`schemas/rag-golden/case-set.schema.json`](../../schemas/rag-golden/case-set.schema.json)。
- 必须声明 `case_set_schema: 2`（本构建认识的唯一修订）与 `scenario_version`。
- 未知字段**被拒绝**而非忽略：拼错的字段名否则会读成「字段缺失」，静默改变用例的含义。

```json
{
  "case_set_schema": 2,
  "scenario_version": "demo-product-2026-10",
  "cases": [
    {
      "id": "battery-life-001",
      "category": "normal_answer",
      "question": "这款耳机充满电能听多久？",
      "expectation": { "kind": "evidence", "required": ["kb0-battery-hours"] },
      "expected": { "kind": "reference", "must_include": ["7 小时"], "must_exclude": ["终身"] },
      "origin": { "kind": "authored" }
    },
    {
      "id": "no-answer-001",
      "category": "no_answer",
      "question": "你们明年会出二代吗？",
      "expectation": { "kind": "must_clarify_or_escalate" },
      "expected": { "kind": "reference", "must_include": ["暂时无法确认"] },
      "origin": { "kind": "promoted", "confirmer": "ops-zhang" }
    }
  ]
}
```

`category` 取 §5.9 的八类（`normal_answer`、`fact_update`、`injection`、`conflict`、`privilege_escalation`、`no_answer`、`cancelled`、`tool_return_attack`）；`expectation.kind` 取 `evidence` / `high_risk_fact` / `must_clarify_or_escalate` / `must_not_act` / `must_cancel`。

`expected` 是**必填**的参考答案，用来判生成侧的 `answer_accuracy`：`must_include` 至少一条（全部出现才算对），`must_exclude` 可选（一旦出现即判错）。缺失、空数组都会在读取时被拒——判定见 [docs/golden-set.md](../../docs/golden-set.md)。

`origin.kind = promoted` **必须**带非空 `confirmer`，否则读取即被拒——见下文。

## 校验

```bash
cargo xtask check --golden                    # 校验 tests/rag-golden/*.json
cargo xtask check --golden --dir <目录>        # 校验别处（用于夹具与演练）
```

三种结论**互不冒充**：

| 情况 | 结论 |
| --- | --- |
| 目录内没有 `*.json` | `CAPABILITY_NOT_READY`，并指出被搜索的目录 —— **不是**通过，也**不是**失败 |
| 有文件但结构不合规 | 逐个报出**全部**缺陷（文件 + JSON 路径 + 稳定 token） |
| 结构合规但违反 §5.9/§6 结构规则 | 报出全部 finding（`below_minimum_cases` / `category_uncovered` / `expectation_category_mismatch`） |

「未提供」与「不达标」被分开报告，是为了让任何一方都无法替另一方背书：题集从未提供时，没有任何一次运行能声称「质量达标」。

本命令**不并入** `cargo xtask check --profile portable`：题集内容不是工程基础门控的一部分，缺少它不代表工程基础有问题。

## 跑一次评估

就位检查通过后，由评估执行体逐题驱动真实模型：

```bash
NAI_LLM_BASE_URL=... NAI_LLM_API_KEY=... NAI_LLM_MODEL=... \
  cargo run -p nai-xtask --example golden_eval
```

它读的就是上表的读取器，因此「这里读得进」与「门禁放行」永远是同一件事。缺题集、结构不合规或缺凭据时它以 `CAPABILITY_NOT_READY` 结束且**不发起任何调用**（凭据只报变量名）。

**它只驱动生成侧**（问题进、答案出、按 `expected` 判定）：检索召回、检索 P95 与行为类判据（澄清/转义、无副作用、取消）均**未驱动**，会具名打印为 `unmeasured`，不会填零。因此它的输出**不是** B01 达标。见 [ADR 0040](../../docs/adr/0040-golden-eval-runner-boundary.md)。

## 当前状态

**未提供**（`files: 0`）。`cargo xtask check --golden` 当前以 `CAPABILITY_NOT_READY` 结束，这是如实反映。

期望的 100 题与真实模型评估由外部投入，见 [`docs/golden-set.md`](../../docs/golden-set.md)、[ADR 0033](../../docs/adr/0033-golden-set-payload-boundary.md) 与 [ADR 0040](../../docs/adr/0040-golden-eval-runner-boundary.md)。

> 说明：`tests/fixtures/rag-golden/example-min.json` 是**夹具**（构造的最小集，用于验证加载器与门禁本身），**不是**真值，也不在本目录内，不会被默认命令读到。

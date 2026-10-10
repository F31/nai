## ADDED Requirements

### Requirement: Every golden case declares an expected answer

题集中的**每个**用例 SHALL 声明其参考答案（`expected`）。参考答案 SHALL 以「必须包含」与「不得包含」两组要点表达，
而 MUST NOT 依赖相似度阈值或自由文本比对——阈值本身会成为判据，且不可复现。

`must_include` SHALL 至少有一条：一个没有任何要求的参考答案，会让判定在「自动正确」与「永远未判定」之间二选一，
两者都等于把判定关掉。`must_exclude` MAY 为空。

题集 schema 修订的语义是「本构建认识的**唯一**修订」：不认识时文件 MUST 被拒绝（`case_set_schema_unsupported`），
MUST NOT 被宽松解释或与旧修订**同时**接受。旧修订被接受会让一份漏写 `expected` 的题集读成「没有参考答案要求」，
而不是结构不合规——那是静默改变用例的含义。

#### Scenario: A case without an expected answer is refused

- **WHEN** 读取一份用例缺少 `expected` 的题集
- **THEN** 报告 `field_missing` 并指出 `cases[i].expected` 的 JSON 路径；该文件 MUST NOT 通过就位门禁

#### Scenario: An empty must-include list is refused

- **WHEN** 读取一份 `expected.must_include` 为空数组的题集
- **THEN** 报告 `field_empty`（或等价的具名缺陷），MUST NOT 被读成「没有要求」

#### Scenario: The previous revision is refused rather than read leniently

- **WHEN** 读取一份 `case_set_schema` 为上一修订的题集
- **THEN** 报告 `case_set_schema_unsupported` 并给出实际值与期望值；该文件 MUST NOT 被按新修订解读

### Requirement: The expected-answer verdict is a pure function and an unjudged answer is not a pass

参考答案的判定 SHALL 是**纯函数**：给定参考答案与候选答案文本，只依据两者返回 `Correct`、`Incorrect` 或
`Unknown`，MUST NOT 读取环境、文件或网络。判定 SHALL 在比较前对两侧做归一化（大小写折叠与空白折叠），
使换行与缩进不产生假不匹配。

判定顺序 SHALL 让**否定式判据优先**：任一「不得包含」命中即为 `Incorrect`，无论「必须包含」是否满足。
「必须包含」全部命中才为 `Correct`。

判定结果 SHALL 复用既有的 `AnswerVerdict`，其中 `Unknown` 表示**未判定**并 MUST 从正确率的分母中排除——
「未判定」与「判定为错」是两种不同的结局，MUST NOT 混同。

#### Scenario: All required points present passes

- **WHEN** 候选答案包含全部 `must_include`、且不含任何 `must_exclude`
- **THEN** 返回 `Correct`

#### Scenario: One missing required point fails

- **WHEN** 候选答案缺少任意一条 `must_include`
- **THEN** 返回 `Incorrect`

#### Scenario: A forbidden phrase fails even when every required point is present

- **WHEN** 候选答案包含全部 `must_include`，但同时出现一条 `must_exclude`
- **THEN** 返回 `Incorrect`——否定式判据优先于肯定式

#### Scenario: Whitespace and case do not change the verdict

- **WHEN** 同一个答案在大小写与空白（含换行、连续空格）上有差异
- **THEN** 判定结果不变

### Requirement: A golden evaluation is driven by real seams, never by a substitute

组织一次黄金集评估的执行体 SHALL 通过**真实接缝**驱动系统：真实文本生成与真实检索，而 MUST NOT 用演练桩
替代后声称评估已完成。

缺前置时执行体 SHALL 报 `CAPABILITY_NOT_READY` 并**不发起任何调用**：

- 题集未提供（`tests/rag-golden` 下无 `*.json`）；
- 题集结构不合规；
- 凭据缺失——报出的是**变量名**，凭据值 MUST NOT 被打印、写进证据文件或写入日志。

真实调用失败 SHALL 与「前缺失」区分（前者说明链路有问题，后者说明条件未就绪），MUST NOT 以同一个退出状态收场。

检索侧 MUST NOT 在无真实知识库时伪造结果：缺少检索证据时该字段按既有口径记为**未测量**（具名），
MUST NOT 记为零值参与门禁判定。

执行体 MUST NOT 被并入 `check --profile portable`：业务题集未建不代表工程基础有问题。

#### Scenario: A missing payload makes no call

- **WHEN** 在题集目录为空时运行评估执行体
- **THEN** 输出 `CAPABILITY_NOT_READY` 并指出被搜索的目录，MUST NOT 发起任何 provider 调用

#### Scenario: A missing credential names the variable and stops

- **WHEN** 缺少文本生成所需的环境变量
- **THEN** 输出 `CAPABILITY_NOT_READY` 与缺失变量的**名称**，不发起调用，且输出中 MUST NOT 出现任何凭据值

#### Scenario: An unmeasured field stays named rather than zero

- **WHEN** 题集就位、真实生成成功，但没有可用的检索证据
- **THEN** 报告把召回率与检索 P95 记为**未测量**（各以其字段名出现），MUST NOT 记为 0 或 0%

#### Scenario: The evaluation is not part of the engineering profile

- **WHEN** 运行 `cargo xtask check --profile portable`
- **THEN** 题集是否提供、评估是否可跑 SHALL NOT 影响该门控的结论

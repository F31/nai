# operations-review Specification

## Purpose
把 B04、R06、B06 从声明变成可执行判据：整链路延迟 MUST 以**端到端样本**计算分位，MUST NOT 由各阶段分位相加得出；延迟分位 MUST 在样本不足时拒绝给出数字；§12.5 的设计目标 MUST 被判定而非假设；长播判定 MUST 以窗口时长为下界且基于两个半段的中位数，跨度或样本不足 MUST 报「不足以判定」；场次复盘报告 MUST 含 §5.9 所列字段，MUST 拒绝不自洽的计数，且未测量的字段 MUST NOT 写成零。本 capability 不授予真实设备长播、真实时延采样或真实场次数据的认证。

## Requirements

### Requirement: End-to-end latency is measured on the whole path

延迟分位 MUST 从**端到端样本**计算，MUST NOT 由各阶段分位相加得出。各阶段分位 MAY 作为**对照**给出，但 MUST 被明确标注为对照而非常规结果。分位定义 MUST 可复现（同一组样本给出同一结果）。端到端样本数低于门槛时，分位 MUST 以「未测量」拒绝，MUST NOT 返回小样本巧合值；阶段样本 MUST NOT 参与端到端分位，缺少阶段样本 MUST NOT 由端到端样本顶替。

#### Scenario: The end-to-end percentile ignores stage samples
- **WHEN** 端到端样本全部为 100，而每个阶段各有足够的 40 样本
- **THEN** 端到端分位仍为 100，阶段之和为另一个（更小的）数，二者被分别给出

#### Scenario: Too few samples is not a percentile
- **WHEN** 样本数低于门槛时请求任一分位
- **THEN** 请求被拒绝为能力未就绪

#### Scenario: A stage percentile needs its own samples
- **WHEN** 端到端样本充足但某阶段没有任何样本
- **THEN** 该阶段的分位被拒绝，且阶段之和也无法给出

### Requirement: Reliability targets are evaluated, not assumed

系统 MUST 能对照 §12.5 的设计目标判定「达标 / 超标」。判定 MUST 在样本不足时以「未测量」拒绝，MUST NOT 记为达标。恰好等于限值 MUST 判为达标。

#### Scenario: A target at the limit meets
- **WHEN** 端到端分位恰好等于目标的限值
- **THEN** 判定为达标

#### Scenario: An unmeasured target is refused
- **WHEN** 样本不足时判定某个目标
- **THEN** 判定被拒绝为能力未就绪，不产生达标结论

### Requirement: A soak window is a minimum and the verdict is robust

长播判定 MUST 以窗口时长（2 小时 / 8 小时 / 24 小时）为**下界**：观察跨度不足或样本数不足时 MUST 报「不足以判定」并如实给出观察到的跨度与样本数，MUST NOT 报「稳定」。判定 MUST 基于**两个半段的中位数**，MUST NOT 只看首尾两点：末端或起始的单个离群点 MUST NOT 改变结论。增长超过阈值 MUST 报「增长」，否则报「稳定」。样本时间 MUST 非降序。

#### Scenario: A short run is insufficient, not stable
- **WHEN** 样本数不足，或样本数足够但跨度小于窗口
- **THEN** 报「不足以判定」并给出实际跨度与样本数

#### Scenario: An even run is stable and a rising run is growing
- **WHEN** 全段数值相同
- **THEN** 报稳定且漂移为零；当后半段中位数比前半段高出超过阈值时报增长

#### Scenario: A single spike does not decide the verdict
- **WHEN** 末端出现单个远高于其余的样本，而前半其余样本持平
- **THEN** 结论仍为稳定；镜像地，起始的单个低值 MUST NOT 掩盖真实的上升

#### Scenario: The window is a minimum, not a nominal label
- **WHEN** 一次 2 小时量级的运行按 8 小时窗口判定
- **THEN** 报「不足以判定」

### Requirement: A review report is coherent by construction

报告 MUST 由调用方给定的计数投影而来。构造 MUST 拒绝不能同时成立的计数：入选多于互动、回答多于入选、批准/修改/拒答之和多于入选、拒答条数未被原因完全解释。比例 MUST 保留分子与分母，MUST NOT 只给一个圆整过的百分数。

#### Scenario: Incoherent counters are refused
- **WHEN** 以回答多于入选、或拒答条数与原因之和不符的计数构造报告
- **THEN** 构造被拒绝为参数非法

#### Scenario: A ratio keeps its denominator
- **WHEN** 一次有互动的场次生成报告
- **THEN** 每个比例都带有其分子与分母，可据此复核

### Requirement: An unmeasured field is never zero

未测量的字段 MUST 被标注为未测量并列入清单，MUST NOT 被写成零。分母为零的比例 MUST 为「未测量」。未定价的成本 MUST 为「未知」这一独立状态，MUST NOT 为 ¥0。

#### Scenario: A session with no interactions has no coverage ratio
- **WHEN** 一次会话的互动量为零
- **THEN** 入选率与覆盖率均为「未测量」并被列入清单，而不是 0

#### Scenario: An unknown cost is not zero
- **WHEN** 会话未提供成本
- **THEN** 成本为「未知」并被列入未测量清单，而不是 ¥0

### Requirement: The review carries the §5.9 fields

报告 MUST 含 §5.9 所列字段：互动量与入选率、回答覆盖率、批准/修改/拒答比例与原因、延迟分位、故障与降级事件、成本与预算、守门命中分类。守门分类 MUST 按 §5.4 的四阶段（输入/证据/输出/播出前）与四结果（通过/重写/拦截/升级）展开，非「通过」的结果 MUST 被计为命中。

#### Scenario: All §5.9 fields are present
- **WHEN** 生成一份报告
- **THEN** 上述七类字段均存在，延迟分位来自本 capability 的分位聚合

#### Scenario: Guard hits are classified by stage and verdict
- **WHEN** 报告含若干守门结果
- **THEN** 命中数只计入非「通过」的结果，并按阶段与结果分别列出

### Requirement: A review audit over reported state

审计 MUST 是对**上报报告值**的纯函数，MUST 检出自我矛盾：分母为零的比例、分子大于分母、拒答无原因或原因数不匹配、未定价成本未标注、延迟分位缺失未标注、分位次序颠倒。审计 MUST 报出**全部**违规并以稳定标记标识每一类。

#### Scenario: A clean report has no violation
- **WHEN** 对一份自洽的报告执行审计
- **THEN** 不报出任何违规

#### Scenario: Each contradiction is reported by a stable token
- **WHEN** 分别构造只触发一类矛盾的报告
- **THEN** 每次报出且仅报出该类对应的标记

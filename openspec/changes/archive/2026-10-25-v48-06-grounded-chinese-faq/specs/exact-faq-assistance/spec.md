## MODIFIED Requirements
### Requirement: Exact FAQ lookup is grounded and bounded
系统 SHALL 只在显式请求时执行有界FAQ匹配，精确优先，未精确命中时可按声明的保守中文词法策略检索已发布FAQ；返回原答案、原问题、当前已发布事实的来源、条件和版本。无足够匹配、冲突或缺依据 SHALL 不产生猜测答案，Provider调用与API费用为零。
#### Scenario: Unknown or conflicting question
- **WHEN** 已发布FAQ没有足够检索依据或存在无法区分的候选
- **THEN** 返回明确未命中/歧义状态，没有可采纳建议且不调用模型。
#### Scenario: Rephrased Chinese question
- **WHEN** 精确匹配未命中且唯一有依据FAQ满足词法检索阈值
- **THEN** 显示检索匹配及原问题，返回原答案和引用供人工核对，结果不授公共输出权。

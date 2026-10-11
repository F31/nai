## ADDED Requirements

### Requirement: External corpus is sourcing material, never a case set

外部语料 SHALL 仅作为「贴近真实直播的问题」（§6）的**取材素材**，供问题的**措辞与语境**；
MUST NOT 被读作题集来源，MUST NOT 提供产品知识库或每题的证据与参考答案。素材存在
MUST NOT 使 §6 的 100 题门槛被视为满足。

素材产生的**候选** MUST NOT 直接进入 `tests/rag-golden/`；候选成为用例 MUST 走既有的**具名人工确认**
（用例来源为 `promoted` 且 `confirmer` 非空）。

从**不可信容器格式**（如 pickle）摄取素材时，MUST 先对字节流做**可执行性**核验——拒绝任何可命名或
调用可调用者的指令——再解析；MUST NOT 先反序列化。素材的来源 MUST 可追溯：出处、许可与逐文件字节
摘要 SHALL 被记录。

#### Scenario: A candidate is not a case

- **WHEN** 从外部语料得到一个提问候选
- **THEN** 它 MUST NOT 直接出现在 `tests/rag-golden/`；只有经具名人工确认（`promoted` + 非空 `confirmer`）后才可进入

#### Scenario: Material does not satisfy the threshold

- **WHEN** 素材已登记但题集仍未提供
- **THEN** §6 的 100 题门槛 MUST NOT 被视为满足，就位门禁仍以未就绪结束并指出被搜索的目录

#### Scenario: An untrusted container is verified before parsing

- **WHEN** 素材以 pickle 这类可携带可执行指令的格式提供
- **THEN** MUST 先核验其指令流不含可命名或调用可调用者的指令；含此类指令者 MUST 被具名拒绝且 MUST NOT 被解析

#### Scenario: A source is traceable

- **WHEN** 素材被登记
- **THEN** 其出处、许可与逐文件字节摘要 MUST 被记录，使问题措辞的来源可回溯

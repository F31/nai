## ADDED Requirements

### Requirement: A drill that asserts causal order establishes it by construction

演练若断言两个事件的**先后**（「A 先于 B 时不得发生 C」），SHALL 通过**构造**使该顺序成立——
在被测的工作进程上设置一个确定性的阻塞点或协商点，使被断言为「先发生」的一侧确实先被观察到。
MUST NOT 依赖「A 的请求在墙钟上发得足够早」这一调度假设：该假设在负载不同的宿主上会时真时假，
使演练结论随调度变色，而波动会被误读为被测实现的不确定性。

#### Scenario: An ordering assertion does not race the scheduler

- **WHEN** 演练断言「控制请求（如 graceful halt）先于某副作用（如提交一帧）到达 ⇒ 该副作用不得被记录」
- **THEN** 演练 SHALL 使被测工作进程停在一个确定性的阻塞点上，令控制请求必然在该副作用之前被观察到；
  MUST NOT 以「控制请求发得足够早」代替该构造

#### Scenario: The asserted premise is proven, not assumed

- **WHEN** 演练断言的是「控制请求到达时被测工作处于某个前置阶段（如 prewarm）」
- **THEN** 演练 SHALL 先观察到该前置阶段**确已进入**（例如该阶段的子进程已被派生）再发出控制请求；
  MUST NOT 让控制请求可能落在更早的阶段而使断言名不副实

#### Scenario: A blocking fixture does not turn the drill into a tautology

- **WHEN** 演练用阻塞点固定了顺序
- **THEN** 被断言的可观测属性 SHALL 仍在被测控制路径失效时失败——
  阻塞点只移除时序赌博，MUST NOT 使演练恒过

#### Scenario: A drill whose fixture the supported platform cannot provide fails by name

- **WHEN** 阻塞点依赖的宿主能力在受支持平台上不可用（例如夹具所用的工具缺失）
- **THEN** 演练 SHALL 以具名失败呈现，MUST NOT 静默跳过或改用更弱的断言

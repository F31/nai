# engineering-foundation Specification

## Purpose
为 nai 后续契约、内核和插件开发提供可复现的工程入口及可自动执行的架构边界检查，使开发者能够区分可移植逻辑检查、Windows 基础构建与产品真实运行认证。

## Requirements

### Requirement: Reproducible workspace
工程 SHALL 使用统一 nai 命名的 Rust 和前端 workspace，提交工具链版本与真实锁文件，支持锁定依赖的编译、类型检查和前端构建。

#### Scenario: Clean dependency installation
- **WHEN** 开发者使用规定工具链执行锁定安装和 portable 检查
- **THEN** workspace 编译、测试和前端检查可执行，不要求 GPU、SDK 或自建云服务

### Requirement: Enforced dependency boundaries
依赖检查 SHALL 从实际 Cargo 元数据检查所有 workspace 包、依赖类别及目标条件，拒绝契约依赖业务、kernel 依赖插件/宿主/UI/平台 SDK、插件依赖另一插件，以及媒体层依赖公网 Provider；包层级必须与目录职责一致。

#### Scenario: Valid graph
- **WHEN** workspace 符合声明的层级和依赖方向
- **THEN** `cargo xtask check-deps` 输出已检查包数并成功退出

#### Scenario: Forbidden or disguised dependency
- **WHEN** normal/build/dev 或条件依赖违反边界，或包用错误层级隐藏其目录职责
- **THEN** 检查非零退出并标明包及违规原因

### Requirement: Explicit build and certification boundaries
任务工具 SHALL 区分 portable 与 Windows 基础检查，在非 Windows 环境拒绝 windows-local；未知或尚未实现的 schema/certify/package 命令 SHALL 非零退出，不输出认证通过。

#### Scenario: Unsupported profile or task
- **WHEN** Linux 请求 windows-local，或请求尚未实现的认证/打包/生成任务
- **THEN** 工具明确说明限制并非零退出

#### Scenario: CI foundation checks
- **WHEN** Linux 或 Windows CI 执行工程检查
- **THEN** 仅报告已运行的工程结果，不将其视为设备、GPU、官方 SDK 或安装验收

### Requirement: Self-consistent harness entry point
`cargo xtask check --profile <name>` SHALL 能在其自身正在运行的宿主上跑完整轮检查：workspace 测试 MUST NOT 重新链接 harness 正在执行的映像，且「排除 harness 自身」所用的包名 MUST 由包自身推导而非字面量。排除 harness 自身的测试步 MUST NOT 使 harness 失去检查——其库与 bin 目标仍 MUST 被编译与类型检查。

#### Scenario: Profile completes on a host that locks running images
- **WHEN** 在会把运行中的可执行文件锁定、拒绝对其重链的宿主上执行 profile
- **THEN** 检查跑完全部步骤并以 0 退出，不出现对运行中映像的链接失败

#### Scenario: Harness package is renamed
- **WHEN** harness 包被改名
- **THEN** 排除与自测仍指向同一个包，不出现静默排除一个不存在包的空排除

#### Scenario: Harness itself stays checked
- **WHEN** profile 运行到测试步
- **THEN** harness 的库目标被测试，且其 bin 目标仍被类型检查，检查范围不因回避链接而缩小

### Requirement: Platform-limited drills declare themselves
需要 Unix 进程模型或原生子进程管道的演练 MUST 由平台事实显式门控；在该平台之外 MUST NOT 被编译进测试目标。当受支持的平台仍无法提供所需宿主能力时，失败 MUST 给出具名诊断并指向记录该事实的位置，MUST NOT 以成功结局报告未执行的演练。

#### Scenario: Drill outside its platform
- **WHEN** 在非 Linux 宿主构建依赖原生子进程管道的演练
- **THEN** 该演练不出现在测试目标中，且同一能力由该平台上实际执行的检查覆盖

#### Scenario: Supported host still refuses a required capability
- **WHEN** 受支持的宿主仍拒绝演练所需的宿主能力
- **THEN** 演练失败并给出具名原因与事实记录位置，MUST NOT 报告为通过

#### Scenario: Rule that only exists through one platform's mechanism
- **WHEN** 某条规则只在特定平台的机制下成立（例如 Unix mode 位）
- **THEN** 该平台的缺口 MUST 由特征化断言钉住，MUST NOT 静默跳过

### Requirement: The complete engineering profile covers the frontend workspace

`cargo xtask check --profile full` SHALL 执行 `portable` 的**全部**步骤，并在其后额外执行前端 workspace 的类型检查、构建与测试。
前端步骤所用的命令 SHALL 取自各包 `package.json` 的 `scripts`，MUST NOT 由门控另行定义一套可能与之漂移的检查。
`portable` MUST 保持「只需 Rust 工具链即可运行」的语义：`full` MUST NOT 给 `portable` 增加任何前端前置。

#### Scenario: The full profile covers every portable step

- **WHEN** 运行 `cargo xtask check --profile full`
- **THEN** 它先执行 `portable` 的全部步骤，再执行前端步骤；`portable` 任一步失败 SHALL 使 `full` 失败，MUST NOT 越过它继续

#### Scenario: The portable profile keeps its Rust-only prerequisite

- **WHEN** 在无 `node`、无前端依赖的环境运行 `cargo xtask check --profile portable`
- **THEN** 该检查的结论不受前端是否存在影响

### Requirement: Missing frontend prerequisites and failing steps are named, never skipped

前端步骤的失败 SHALL 以**具名诊断**非零退出，MUST NOT 被跳过、降级或计入已通过的步骤。前端前置缺失——运行时无 `node`，
或 `node_modules` 未安装导致 `typescript`/`vite` 不可解析——SHALL 同样以具名诊断失败，MUST NOT 静默通过。

#### Scenario: A missing frontend prerequisite fails by name

- **WHEN** 运行时找不到 `node`，或 `typescript`/`vite` 因依赖未安装而无法解析
- **THEN** 非零退出并给出具名诊断，指出缺失的工具链或包；MUST NOT 跳过该步后仍报告门控通过

#### Scenario: A failing frontend step stops the run

- **WHEN** 某个前端步骤以非零状态结束
- **THEN** 门控以非零退出并指出失败的**包与 script 名**，后续步骤 MUST NOT 被继续执行

### Requirement: Unrecognised script commands are refused rather than guessed

门控不认识的 `scripts` 内容 SHALL 被拒绝并具名指出包名、script 名与出问题的 token：包括不认识的可执行名，
以及 `|`、`>`、`||` 等门控不实现的 shell 语法。MUST NOT 落到 PATH 上猜测执行，也 MUST NOT 近似解释。

#### Scenario: An unrecognised command is refused

- **WHEN** 某个包的 `scripts` 里出现门控不认识的可执行，或门控不实现的 shell 语法
- **THEN** 具名拒绝并指出该包、该 script 与出问题的 token；MUST NOT 执行它，也 MUST NOT 忽略它后继续

### Requirement: A passing frontend check is engineering evidence, not product acceptance

前端检查通过只表示工程基础可执行。类型检查、构建与单测的结果 MUST NOT 被报告为浏览器渲染、人工 UI 或任何产品验收的达标。

#### Scenario: A green frontend check claims nothing beyond the baseline

- **WHEN** `full` 的前端步骤全部通过
- **THEN** 结论只声称前端 workspace 的工程检查通过，MUST NOT 声称渲染正确、界面可用或任何 `O` 系列验收达标

### Requirement: The CI pipeline runs the complete engineering profile

持续集成（CI）SHALL 执行完整工程 profile（`cargo xtask check --profile full`），其步骤覆盖 `portable` 的**全部**步骤，
并在其后执行前端 workspace 的类型检查、构建与测试。
CI SHALL 在 Linux 与 Windows 两个平台上执行同一命令，MUST NOT 按平台选用不同的 profile。

#### Scenario: CI covers the frontend workspace

- **WHEN** CI 在 Linux 或 Windows 上执行工程检查
- **THEN** 前端 workspace 的类型检查、构建与测试被实际执行；任一步失败 SHALL 使流水线非零退出

#### Scenario: CI reports only what it ran

- **WHEN** CI 的全部工程步骤通过
- **THEN** 结论只声称工程基线可执行，MUST NOT 声称渲染正确、界面可用、设备/GPU/官方 SDK/安装验收达标

### Requirement: CI does not define the frontend commands beside the profile

CI MUST NOT 用「只覆盖 Rust 侧的 profile + 另行定义的前端命令」来代替完整工程 profile——那会让前端检查在流水线中出现第二套定义。
CI 执行的前端命令 SHALL 与门控同源，即取自各包 `package.json` 的 `scripts`；MUST NOT 在流水线中另写一份可能与之漂移的命令清单。

#### Scenario: No second frontend command list

- **WHEN** 检查流水线的步骤与门控所执行的前端命令
- **THEN** 两者指向同一处定义（各包 `package.json` 的 `scripts`），MUST NOT 出现只覆盖部分前端检查（例如只做类型检查与构建、
  不做包内单测）的第二套命令

### Requirement: A failing CI gate publishes a credential-free failure summary

持续集成（CI）的工程门控步骤失败时，MUST 把失败摘要以 check-run **annotation** 的形式发布，
使**不持有仓库凭据**的读者也能读到失败原因。CI 的执行日志端点需要仓库 admin 权限，
因此 MUST NOT 成为失败原因的唯一载体。

#### Scenario: Failure reason is readable without credentials

- **WHEN** 门控步骤在 CI 中失败
- **THEN** 该次运行的 check-run annotations 中出现门控输出的关键行；
  `GET /repos/{owner}/{repo}/check-runs/{id}/annotations` MUST 在**不携带凭据**时即可读到这些内容

#### Scenario: The summary never replaces the verdict

- **WHEN** 诊断步骤自身失败（例如脚本缺失或抛错）
- **THEN** job 的结论仍由门控步骤决定；MUST NOT 被「诊断失败」掩盖或替换

### Requirement: Collecting the gate output must not change the gate's verdict

为采集日志而对门控命令施加的管道或重定向，MUST NOT 改变该步骤的退出码。实现 MUST 显式启用管道失败传播；
MUST NOT 出现「命令失败而步骤报告成功」的情形。

#### Scenario: Failure survives output collection

- **WHEN** 门控命令的输出经由会吞掉退出码的管道采集到文件
- **THEN** 该步骤仍以门控的退出码结束；失败步骤 MUST NOT 被报告为成功

### Requirement: No summary means no annotation

诊断步骤 SHALL 只在确有可发布内容时产生 annotation。门控日志缺失、为空或只有空行时，MUST NOT 产生任何 annotation，
且诊断步骤 MUST 成功退出——诊断 MUST NOT 凭空制造噪音或伪失败。

#### Scenario: Missing or empty log stays silent

- **WHEN** 诊断步骤被调用时门控日志不存在或不含任何内容
- **THEN** 不产生任何 annotation，步骤成功退出

### Requirement: A gate-failure summary surfaces signals, not filler

失败摘要 SHALL 以**指向失败的行**为中心：编译器与测试框架的错误报告、其紧邻的失败详情，
以及失败所属的**测试目标**。MUST NOT 用日志里的无关行把配额填满。判定一行是否为信号的依据
SHALL 是「该行是否报告了失败」，而 MUST NOT 是「该行是否含某个关键词」——测试名、依赖名都可能含信号词。

#### Scenario: A passing test whose name contains a signal word is not a signal

- **WHEN** 某测试**通过**，但其名字里含信号词（例如 `…_unimplemented_routes_…`）
- **THEN** 该行 MUST NOT 被判为失败信号，MUST NOT 因其把后续无关行带入摘要

#### Scenario: Scarcity is preserved over fullness

- **WHEN** 整份日志只有一条真正指向失败的行
- **THEN** 摘要 SHALL 以该行为中心输出少量注解；MUST NOT 用无关行补足到条数上限而把该行淹没

#### Scenario: Failure details travel with the signal

- **WHEN** 日志含失败测试的汇总段（失败名列表与各自的标准输出）
- **THEN** 摘要 SHALL 连带其紧邻的详情（断言消息、失败名）以及失败所属的**测试目标头**

### Requirement: An incomplete capture is declared as such

失败摘要的来源日志若明显不完整——判据是「出现测试框架的逐测试行，却缺少其汇总行」——
诊断 MUST 在注解中**明示**该不完整并给出日志总行数。MUST NOT 只罗列出现过的行而不提这份日志并不完整；
否则读者会把一份被截断的日志当成全部证据。

#### Scenario: Truncated capture is declared

- **WHEN** 采集到的门控日志含逐测试行，但没有 `test result:` 汇总行
- **THEN** 摘要中出现一条警示，说明日志在汇总行之前结束，并给出总行数

#### Scenario: A complete capture is not flagged

- **WHEN** 采集到的日志含 `test result:` 汇总行
- **THEN** MUST NOT 出现「不完整」警示

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

### Requirement: Every CI workflow step is declared with its local coverage

CI 工作流的每一个 step SHALL 在一个在册登记表里有且只有一条登记，并 SHALL 归入一个闭集类别之一：
`infra`（环境准备，不产生判定）、`local`（本机门控同样执行）、`ci-only`（只有 CI 执行）、
`diagnostic`（仅在失败时跑的诊断通道）。`local` 的每条 SHALL 引出一处本机门控落点并证明它存在；
`ci-only` 的每条 SHALL 给出成因而成因 SHALL 取自一个闭集。新增、删除或改名一个 step，
SHALL 使检查失败并具名指出该 step。

#### Scenario: An unregistered step fails by name

- **WHEN** 往 workflow 插入一个没有登记的 step，或删掉一个已登记的判定 step
- **THEN** 检查失败，且错误里具名指出该 step 的身份

#### Scenario: Claiming local coverage has to pay for itself

- **WHEN** 一个 step 声明 `local`，但它引出的门控落点在该文件里不存在
- **THEN** 检查失败并具名该 step——「本机也跑」不是可以空口说的话

#### Scenario: A reason outside the closed set is a label, not a reason

- **WHEN** 一个 `ci-only` 的 step 给出的成因不在闭集里，或它同时又挂了一处本机落点
- **THEN** 检查失败并具名该 step

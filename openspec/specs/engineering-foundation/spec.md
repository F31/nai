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

### Requirement: Every cargo xtask command is declared with its gate coverage

`cargo xtask` 的每一条命令（判定单元 SHALL 是**签名**，即空格连接的顶层字面量序列，而非命令名）
SHALL 在一个在册登记表里有且只有一条登记，并 SHALL 归入一个闭集类别之一：`gate`
（执行体在本机门控闭包里被执行）、`standalone`（是判定命令但不在门控闭包里）、`artifact`
（生成产物、不产生判定结论）、`help`（不产生动作的入口）。`gate` 的每条 SHALL 引出一处
真的存在的门控落点并证明它逐字落在门控闭包内；`standalone` 的每条 SHALL 给出成因而成因
SHALL 取自一个闭集。`Usage:` 字符串 SHALL 与分发集合对账：它 MUST NOT 声称一条不存在的命令，
且每一条非 help 命令 SHALL 出现在它里面。新增、删除或改名一条命令 SHALL 使检查失败并具名
指出该命令。

#### Scenario: An unregistered or vanished command fails by name

- **WHEN** 往 `execute()` 插入一条没有登记的臂，或删掉一条已登记的臂
- **THEN** 检查失败，且错误里具名指出该命令的签名

#### Scenario: Claiming local gate coverage has to pay for itself

- **WHEN** 一条 `gate` 命令引出的门控落点不在门控闭包内，或该落点所指的文件没有被提供给判据
- **THEN** 检查失败并具名该命令——「本机门控真的跑它」不是可以空口说的话

#### Scenario: The usage string must not drift away from the dispatch

- **WHEN** `Usage:` 字符串声称一条 `execute()` 里不存在的命令，或漏写一条非 help 命令
- **THEN** 检查失败并具名那条命令

#### Scenario: A reason outside the closed set is a label, not a reason

- **WHEN** 一条 `standalone` 命令给出的成因不在闭集里，或它同时又挂了一处门控落点
- **THEN** 检查失败并具名该命令

### Requirement: The tested package set is computed, not asserted

被测面（本机门控跑哪些 Rust 包、每个 `test --suite` 跑哪些包）SHALL 由实参**解析**成包名集合，
再 SHALL 做包含关系核验，禁止仅以「某个 marker 在源码里逐字存在」充当覆盖证明。每个 suite 的包集
SHALL 被门控闭包里**单一条** `cargo test` 命令的覆盖集包含（不得由若干条命令的并集拼出）；
每个 workspace 成员 SHALL 被这些命令的并集覆盖；每条 suite 引用的 `-p` 值 SHALL 落在一个真实成员的
包名上；CI 执行面调用的每条 `cargo xtask` 命令 SHALL 在本机命令面在册。三处集合 SHALL 各自
独立校验，任一处缺失或不可解析 SHALL 具名失败而非静默降级。

#### Scenario: A shrunken gate loses suites and members by name

- **WHEN** 本机门控的全 workspace 测试命令被改成只测试少数几个包
- **THEN** 检查失败，并逐一点名每个失去覆盖的 suite 包集与每个不再被跑到的成员包名

#### Scenario: A package reference that resolves to nothing fails

- **WHEN** 某个 suite 的 `-p` 值不是任何 workspace 成员的包名，或 `--exclude` 的标识符无法解析
- **THEN** 检查失败并具名该值——不得把「没给它输入」当成「空集」静默通过

#### Scenario: A CI command that does not exist is named

- **WHEN** CI 执行面调用了一条本机命令面没有注册的命令
- **THEN** 检查失败并具名该命令的签名

### Requirement: Every package script has a named disposition, and its consumer is computed

`package.json` 的每个脚本 SHALL 有具名处置：或引出一处**真调用**（`{file, marker}`，在剥离注释后的
引用扫描面里逐字存在），或落入**成因闭集**并逐格付代价。判定单元 SHALL 是「包目录 + 脚本名」的配对，
MUST NOT 是裸脚本名。**文档里的命令 SHALL NOT 充当消费者**，**自引用 SHALL NOT 充当消费者**。
驱动表的每一项 SHALL 落在该包脚本上，且其首个 token SHALL 在门控认得的可执行名集合内。
声明面与台账 SHALL 双向一致；引用不可解析、成因落域外、证据不逐字存在或输入缺失
SHALL 具名失败而非静默降级。

#### Scenario: A script nobody calls is named

- **WHEN** 某个 `package.json` 里新增（或原有）一个脚本，而没有任何真调用点
- **THEN** 检查失败并具名该「包目录 + 脚本名」，要求它给出成因与代价

#### Scenario: A driver step that names an unknown script fails

- **WHEN** 驱动表里出现一个 `(包目录, 脚本名)` 在该包 `scripts` 里不存在，或首个 token 不在门控认得的可执行名内
- **THEN** 检查失败并具名该项与那个 token——不得等到真跑前端门控才以运行时错误暴露

#### Scenario: Neither a document nor the script itself is a consumer

- **WHEN** 一处「消费者」证据落在 `.md` 文档里，或落在这个脚本自己的身体里（自引用）
- **THEN** 检查失败并具名该证据——文档是给读者的指引，自引用是恒真的自我满足

### Requirement: Every registration site's gate tier is paid for by the parsed region that runs it

登记站点的 `tier` SHALL 由**宿主解析出的区域**兑付。`portable` 站点的 marker SHALL 在
`xtask/src/lib.rs` 的 `engineering_checks` 区域（**剥离注释后**）里逐字存在；其 host 不是该文件时，
该 host 的模块 SHALL 在该区域里被调用。`full` 站点 SHALL 由驱动表某个 STEP 兑付——该 host
SHALL 解析出的包目录是该 STEP 的 `dir`，且该 STEP 的 `script` 的脚本体 SHALL 含 marker。
`ci` 站点的 marker SHALL 逐字出现在某个**具名** workflow step 的 `run` 文本里（剥离整行注释）。
`full`/`ci` 的 marker SHALL NOT 落在 portable 区域里。兑付规则的键集 SHALL 等于 `TEST_TIERS`，
每个取值 SHALL 至少有一个站点。缺输入、区域不可解析或 tier 无兑付规则
SHALL 具名失败而非静默降级。

#### Scenario: A tier that no parsed region pays for fails

- **WHEN** 一个站点的 marker 从 `engineering_checks` 里被搬进 `["check","--profile","full"]` arm，
  而该站点仍声明 `tier: 'portable'`
- **THEN** 检查失败并具名该站点与它声明的层级，指出 portable 区域里没有它的兑付串

#### Scenario: A tier paid only by a comment fails

- **WHEN** 一个 `ci` 站点的 marker 只出现在 `engineering_checks` 的**注释**里，或只出现在某个
  workflow step 的 `run` 的注释行里
- **THEN** 检查失败并具名——注释是散文，不执行任何东西

#### Scenario: A declaration that understates the local gate fails

- **WHEN** 一个声明 `tier: 'ci'`（或 `full`）的站点的 marker 出现在 portable 区域里
- **THEN** 检查失败并具名该站点——该声明少报了本机门控真正跑到的东西

#### Scenario: A tier value with no rule or no site fails

- **WHEN** `TEST_TIERS` 新增一个取值而没有对应的兑付规则，或某个取值没有任何站点
- **THEN** 检查失败并具名那个取值，而不是让它静默地不被认识

### Requirement: Every registered workflow step's execution configuration is paid for by a parsed platform set

每个在册 workflow step 的**执行配置** SHALL 由解析得出的**平台集合**兑付。该集合 SHALL 由 job 的
`matrix`（或字面 `runs-on`）与步的 `if:` **静态求值**得出，且 SHALL 非空。产生判定的步
（`local` / `ci-only`）SHALL NOT 带真值 `continue-on-error`；`diagnostic` 步 SHALL 带真值
`continue-on-error`。`ci-only` 的 `platform-toolchain` 成因 SHALL 由**真子集**平台条件兑付，
其余成因 SHALL NOT 被平台限定。`matrix` 形状、`if:` 形状、`continue-on-error` 形状、
runner 标签与 `runner.os` 取值 SHALL 落在各自的闭集里。域外取值、不可解析的输入、
空注册表或不可兑付的声明 SHALL 具名失败，而非静默降级。

#### Scenario: A judged step that can never run fails

- **WHEN** 一个在册步的 `if:` 与 job 的矩阵在静态求值后得到**空的执行平台集合**
  （例如矩阵只剩 `windows-latest`，而某步带 `if: runner.os == 'Linux'`）
- **THEN** 检查失败并具名该步与它声明的条件，指出它在任何一条矩阵腿上都不会执行
  ——一个不跑的判定就是一个静默消失的判定

#### Scenario: A judgement that can fail without failing the job fails

- **WHEN** 一个 `local` 或 `ci-only` 的步带上真值 `continue-on-error`
- **THEN** 检查失败并具名该步——能失败而不改变 job 结论的判定是装饰，不是判定

#### Scenario: A diagnostic that can replace the verdict fails

- **WHEN** 一个 `diagnostic` 步**没有**真值 `continue-on-error`
- **THEN** 检查失败并具名该步——诊断自身的问题不得替换它所转译的那次失败的结论

#### Scenario: A cause that disagrees with its condition fails

- **WHEN** 一个 `ci-only` 步的成因是 `platform-toolchain` 却没有平台限定条件，
  或成因**不是** `platform-toolchain` 却带了平台限定条件
- **THEN** 检查失败并具名该步与它声明的成因，指出成因与执行条件互相矛盾

#### Scenario: A configuration value outside the closed sets fails

- **WHEN** 矩阵轴不是唯一的 `os`、`runs-on` 没有消费 `matrix.os`、`if:` 不落在条件形状表里、
  `continue-on-error` 不是布尔字面量、`runner.os` 取值或 runner 标签不可分类
- **THEN** 检查失败并具名该处的取值——不可解析的配置 MUST NOT 静默降级或静默跳过

#### Scenario: A platform-specific command on a platform that lacks it fails

- **WHEN** 一个步的 `run` 文本命中平台专属命令形状（`sudo` 需 POSIX、`apt-get` 仅 Linux），
  而它的执行平台集合**不**是该形状支持集的子集
- **THEN** 检查失败并具名该步与命中的形状，指出它会在不支持该命令的平台上执行

### Requirement: Every registered judging step's target is paid for by parsing its command

每个在册判定步（`kind` ∈ `local` / `ci-only`）SHALL 声明它所跑的**目标**（`target`），
且该目标集合 SHALL 由该步的 `run` 文本**解析**得出并与声明**双向相等**。解析出的目标的**形状**
SHALL 落在闭集里。`infra` 与 `diagnostic` 步 SHALL NOT 声明目标，且其 `run` SHALL 解析出
**零个**目标。缺输入、空注册表或不可解析的输入 SHALL 具名失败，而非静默降级。

#### Scenario: A step whose target was swapped to another suite fails

- **WHEN** 一个声明跑 `test --suite kernel` 的判定步，其 `run` 被改成
  `cargo xtask test --suite hosts`
- **THEN** 检查失败并具名该步，指出解析出的目标与声明的目标不一致——一条声明与它实际跑的
  目标分家，就是那条判定面从 CI 静默消失

#### Scenario: A step whose judgement was replaced by another command fails

- **WHEN** 一个判定步的 `run` 被整条换成**另一条合法的**判定命令
  （例如把 `test --suite provider` 换成 `check --secrets`）
- **THEN** 检查失败并具名该步——「这一步像不像一条判定」与「这一步跑的是哪个目标」
  是两件事，前者为真不能让后者免检

#### Scenario: A declared target with no command fails

- **WHEN** 一个判定步声明了目标，而它的 `run` 解析出**空**目标集合
- **THEN** 检查失败并具名该步——一条没有兑现物的目标声明是一句空话

#### Scenario: A judgement hidden in an infrastructure step fails

- **WHEN** 一个 `infra` 或 `diagnostic` 步的 `run` 解析出**非空**目标集合
  （例如把 `node --test …` 塞进「环境准备」）
- **THEN** 检查失败并具名该步——把一条判定标成环境准备是绕过整套检查的后门

#### Scenario: A declared target shape outside the closed set fails

- **WHEN** 一个声明的目标形状不在 `TARGET_SHAPES` 里，或声明了一个空值的目标
- **THEN** 检查失败并具名该处的取值——域外的形状是一个标签，不是一个分类。
  解析一侧的形状由构造保证落在闭集里（`targetsOf` 只产出闭集内的形状），
  **能由结构蕴含的 MUST NOT 再具名一次**；于是「命令不是这三种形状之一」这件事
  由「解析出零个目标」两个方向分别具名

#### Scenario: A missing input or an empty registry fails

- **WHEN** workflow 文本未被提供、为空、或注册表为空
- **THEN** 检查失败并具名缺的是哪一项——一条会因入参缺失而恒真的断言比没有断言更坏

### Requirement: Every workflow's trigger surface is paid for by parsing its `on:` block

每个在册 workflow 的**触发面**（`on:` 里的事件集合及其过滤器）SHALL 由该文件的 YAML **解析**得出，
并与在册的触发器清单**双向相等**；至少一条 SHALL 能由仓库事件**自动**驱动。每条在册触发器 SHALL
恰好携带 `host`／`event`／`filters` 三个字段，其事件名与过滤器键 SHALL 落在各自的闭集里，
自动与手动 SHALL 由事件表推导而非手写。缺输入、空注册表、`on:` 缺失或为空、不可解析的输入
SHALL 具名失败，而非静默降级。

#### Scenario: An event removed from the trigger surface fails

- **WHEN** 一个在册被 `push` 与 `pull_request` 触发的 workflow，其 `on:` 里被删掉 `pull_request`
- **THEN** 检查失败并具名该事件，指出它被声明却没有出现在触发面上——一条消失的触发事件意味着
  那条路径从此不再进门控，而所有步层判据**照样全绿**

#### Scenario: A workflow with no automatic trigger fails

- **WHEN** 整个 `on:` 被缩成仅 `workflow_dispatch`（或任何只在手动事件表里的事件）
- **THEN** 检查失败并具名该 workflow——一个永不自动运行的 workflow 不是门控，
  而它里面的每一条判定步都仍然是「在册」的

#### Scenario: A trigger restriction that nobody declares fails

- **WHEN** `push` 被限定到一个**不存在**的分支（例如 `push: {branches: [nope]}`），
  而在册的 `push` 触发器没有声明该过滤器
- **THEN** 检查失败并具名 `(event, filter-key)` 这个单元及两侧取值——事件集合一个字没变，
  但推送事实上不再触发；这正是「门控静默停止运行」的形状

#### Scenario: A declared filter that the workflow does not apply fails

- **WHEN** 在册触发器声明了过滤器，而该 workflow 的触发面**没有**施加它
- **THEN** 检查失败并具名该单元——一条没有兑现物的过滤器声明是一句空话，
  与「施加了却没人声明」是**同一件事的两个方向**，两者都必须具名

#### Scenario: An event outside the classified set fails

- **WHEN** 触发面或在册清单里出现一个既不属于自动事件表、也不属于手动事件表的事件名
  （例如把 `push` 拼成 `pus`）
- **THEN** 检查失败并具名该事件——若把未知事件一律当成自动触发，一个拼错的名字会让
  「至少一条自动触发」在一条**永不触发**的 workflow 上通过

#### Scenario: A hand-written trigger field fails

- **WHEN** 一条在册触发器带上了 `host`／`event`／`filters` 之外的键
  （例如手写一个 `kind`，或把 `filters` 错拼成 `filter`）
- **THEN** 检查失败并具名该字段——自动与手动由事件表推导，一条**没人会读**的字段是一条
  没有代价的声明：写它的人以为它有意义，而判据从来不看它

#### Scenario: A filter key outside the closed set fails

- **WHEN** 触发面或被声明的一方出现一个不在过滤器键闭集里的键，
  或过滤取值不是非空字符串数组
- **THEN** 检查失败并具名该键与其持有者——域外的键是一个标签，不是一个过滤器；
  它可能是笔误，也可能是一条**没人会去读**的抑制条件

#### Scenario: A missing input, an empty registry, or an empty trigger surface fails

- **WHEN** workflow 文本未被提供、为空、无法解析，在册表为空，或 `on:` 缺失／为空
- **THEN** 检查失败并具名缺的是哪一项——一条会因入参缺失而恒真的断言比没有断言更坏，
  而一个空的触发面本身就是一个「永不运行」的判定

### Requirement: Every registered cause is paid for by evidence the repository can show

`ci-only` 与 `standalone` 的每个成因取值 SHALL 在一张兑付规格表里有且只有一条规格，且规格表的键集
SHALL **恰等于**对应的成因闭集。每条规格 SHALL 至少给出一条凭据（`file` / `marker` / `why` 三字段），
其 `marker` SHALL **逐字存在于** `file` 的**内容**里，`file` SHALL 是本仓库**已跟踪**的路径，
且 SHALL NOT 落在被审注册表自身所在的目录（`scripts/contracts/`）；`why` SHALL 非空。
每个成因取值 SHALL 至少有一条在册条目。凭据文件的内容或其跟踪状态未被提供、路径未被跟踪、
`marker` 不存在、缺字段 SHALL 具名失败，MUST NOT 静默降级。

#### Scenario: A cause whose evidence is written nowhere

- **WHEN** 某格凭据的 `marker` 在它的 `file` 内容里不存在（凭据是一句空话）
- **THEN** 校验 MUST 失败并具名该成因与该凭据——「这条成因凭什么成立」不是可以空口说的话

#### Scenario: A cause whose evidence is not repository content

- **WHEN** 某条凭据的 `file` 未被 git 跟踪（例如它落在 `target/` 下的产物或工作树里的临时文件里）
- **THEN** 校验 MUST 失败并具名该凭据——工作树里一个**能读到**的文件不是**仓库事实**

#### Scenario: A cause whose evidence the registry writes itself

- **WHEN** 某条凭据的 `file` 落在被审注册表自身所在的目录里
- **THEN** 校验 MUST 失败并具名该凭据——表自己写的凭据对表恒真，那是**自我满足**

#### Scenario: A cause cell with no instance

- **WHEN** 一个成因取值在闭集与兑付表里都在，却没有任何在册条目的成因是它
- **THEN** 校验 MUST 失败并具名该成因——一条**没有实例**的规则对空集恒真

#### Scenario: A cause that nobody pays for, and a payment with no cause

- **WHEN** 成因闭集里有一个取值没有兑付规格，或兑付表里有一个键不在成因闭集里
- **THEN** 校验 MUST 在这两个方向**各报一次**并具名那个取值——**欠付**（有人声明、没人付）
  与**空规格**（有人付、没对象）是两个方向，MUST NOT 被合并成一个信号

#### Scenario: A receipt that explains nothing

- **WHEN** 某条凭据的 `why` 缺失或为空
- **THEN** 校验 MUST 失败并具名该凭据——凭据回答了什么 MUST 写在明面上，供人审；
  判据读 `marker`，人读 `why`

#### Scenario: A missing input makes the check fail rather than pass

- **WHEN** 凭据文件的内容集或其跟踪状态没有被提供给校验
- **THEN** 校验 MUST 具名失败并指出缺的是哪一项——一条会因入参缺失而恒真的断言比没有断言更坏

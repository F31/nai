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

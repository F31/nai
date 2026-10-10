## Purpose
让工程校验回路能在**它自己运行的宿主**上如实完成整轮检查：harness 不得把自身变成不可通过的检查；跑不了的演练必须由平台事实显式声明，而不是被口头解释或静默算作通过。

## ADDED Requirements

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

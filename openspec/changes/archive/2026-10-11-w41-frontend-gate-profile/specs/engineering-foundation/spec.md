## ADDED Requirements

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

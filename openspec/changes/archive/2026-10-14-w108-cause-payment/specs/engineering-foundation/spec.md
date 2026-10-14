## ADDED Requirements

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

## MODIFIED Requirements

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

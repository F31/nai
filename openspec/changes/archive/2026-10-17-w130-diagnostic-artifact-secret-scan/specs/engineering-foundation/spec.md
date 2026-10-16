## MODIFIED Requirements

### Requirement: Every registered workflow step's execution configuration is paid for by a parsed platform set

每个在册 workflow step 的**执行配置** SHALL 由解析得出的**平台集合**兑付。该集合 SHALL 由 job 的
`matrix`（或字面 `runs-on`）与步的 `if:` **静态求值**得出，且 SHALL 非空。产生判定的步
（`local` / `ci-only`）SHALL NOT 带真值 `continue-on-error`；`diagnostic` 步 SHALL 带真值
`continue-on-error`。`ci-only` 的 `platform-toolchain` 成因 SHALL 由**真子集**平台条件兑付，
其余成因 SHALL NOT 被平台限定。`matrix` 形状、`if:` 形状、`continue-on-error` 形状、runner 标签与
`runner.os` 取值 SHALL 落在各自的闭集里。域外取值、不可解析的输入、空注册表或不可兑付的声明
SHALL 具名失败，而非静默降级。

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

## ADDED Requirements

### Requirement: A step that judges an artifact produced by an earlier step declares always()

一个**产物由更早的 CI 步产生**的在册步 SHALL 带 `always()` 条件。`always()` 在**平台集合**上与
「无条件」等价，差别只在**失败语义**：该步 MUST NOT 因前序步的失败而被跳过——依赖 GitHub 默认跳过
语义的判定会在门控一红的那一刻消失，而那恰是它最该跑的一次。

#### Scenario: A scan of a CI-produced artifact must cover the failure path

- **WHEN** 一个 `ci-only` 步的成因是 `ci-produced-artifact`（它判定的产物由**更早的 CI 步**产生），
  而它的 `if:` 不是 `always()`——包括**完全没有** `if:` 的情形
- **THEN** 检查失败并具名该步与它声明的条件，指出前序步失败会让它被跳过，
  扫描恰在产物最有价值的那一次消失。GitHub 的默认语义是「前序步失败 ⇒ 后续步被跳过」，
  于是没写条件等于「门控一红，扫描就消失」——这是一个**在需要它的那一刻才消失**的判定

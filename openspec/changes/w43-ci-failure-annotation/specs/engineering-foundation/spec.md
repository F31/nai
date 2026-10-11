## ADDED Requirements

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
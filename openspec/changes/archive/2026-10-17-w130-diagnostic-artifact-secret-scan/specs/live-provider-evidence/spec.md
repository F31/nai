## MODIFIED Requirements

### Requirement: Credentials come from the process environment, never from a published file

凭据 MUST 只经**进程环境变量**注入（`NAI_LLM_API_KEY`、`NAI_TTS_API_KEY` 等）。**git 会发布的内容**中 MUST NOT 出现明文凭据。`cargo xtask check --secrets` MUST 以「已跟踪文件 **加** 未跟踪但未被忽略的文件」（`git ls-files --cached --others --exclude-standard`）为扫描域并断言其为零命中。扫描域 MUST NOT 是工作区磁盘，也 MUST NOT 只取已暂存集合：**被忽略的本地文件（如 `.env`）不在扫描域**，否则本机凭据无法存在；而**新增但尚未暂存**的文件 MUST 仍在域内，否则检查只能在错误几乎铸成之后才发现。扫描 MUST 覆盖已知高熵凭据前缀，并 MUST NOT 声称覆盖任意形态的密钥。

#### Scenario: A published file carries a high-entropy credential
- **WHEN** git 会发布的任一文件含有前缀加足额长度的凭据串
- **THEN** `cargo xtask check --secrets` 失败，并列出文件名与行号

#### Scenario: A new file is scanned before it is staged
- **WHEN** 一个新增文件尚未被 `git add`
- **THEN** 它已在扫描域内，凭据在暂存之前即被拦下

#### Scenario: The ignored local credential file is not in the scan domain
- **WHEN** 被忽略的本地 `.env` 持有真实凭据，而待发布内容均无凭据
- **THEN** `cargo xtask check --secrets` 通过

#### Scenario: A short sentinel is not mistaken for a credential
- **WHEN** 测试夹具含有同前缀但长度低于阈值的哨兵串
- **THEN** 扫描不命中

## ADDED Requirements

### Requirement: The registered diagnostic artifact domain is scanned by the same command

`cargo xtask check --secrets` SHALL 同时扫一份**在册登记表**给出的**诊断产物目录**集合，每条登记为
`{dir, producer, source, marker}`（`source` 由编译期从 `producer` 的路径抓取）。递归遍历 MUST NOT
跟随符号链接；**缺目录 SHALL 报「没有这个目录」，MUST NOT 被说成「跳过」**；空集 SHALL 报 `0`。
非 UTF-8 的产物（崩溃转储是二进制）SHALL 仍被扫描，MUST NOT 因解码失败整条丢弃。命中 SHALL 只报
路径、行号与族，**MUST NOT 回显匹配到的文本**。登记的 `marker` SHALL 逐字存在于它自己声称的生产者
文本里并点名扫描器要走的目录；生产者 SHALL 是本仓库已跟踪的文件，且 MUST NOT 是登记表自己的
实现文件。

#### Scenario: A diagnostic artifact carries credential material
- **WHEN** 一个在册产物目录里的文件（例如 CI 门控日志）含有前缀加足额长度的凭据串
- **THEN** `cargo xtask check --secrets` 失败，并按路径与行号具名该产物，**不回显匹配到的文本**

#### Scenario: A binary artifact is scanned instead of being dropped
- **WHEN** 产物目录里有一个非 UTF-8 的文件（崩溃转储）而它含 ASCII 段的凭据串
- **THEN** 该文件被具名为非文本**且仍被扫过**、命中被报出——MUST NOT 因解码失败而整条丢弃

#### Scenario: A missing artifact directory is a value, not a failure
- **WHEN** 一个在册产物目录在本机不存在
- **THEN** 扫描报出「没有这个目录（nothing to scan）」并通过——它 MUST NOT 被读成「存在但为空」，也 MUST NOT 被说成「跳过」

#### Scenario: A registration that names no producer fails
- **WHEN** 一条登记的 `marker` 不在它自己声称的生产者文本里，或那个 `marker` 不点名扫描器要走的目录
- **THEN** 校验失败并具名该生产者——登记即核验，MUST NOT 是一句空话

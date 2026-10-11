# supply-chain-sbom Specification

## Purpose
把「软件物料清单」从一个没有定义的空位变成**可判真伪的产物**：明确它覆盖哪一个依赖闭包、每个组件必须记录什么、
许可如何被记录、如何保证两次生成可比，以及它**不能**被当作什么。基线要求「每次发布附 SBOM」（§11.6）、
发布目录含「SBOM 与许可」（§29.5），并以 SBOM 作为供应链威胁 `T-08` 的控制之一；本能力规定这份产物的构成与边界。

## Requirements

### Requirement: 物料清单覆盖锁定依赖闭包

工程工具 SHALL 从 cargo 自身的 metadata 与锁文件生成覆盖**锁定依赖闭包**的物料清单，
包含每个 workspace 成员；生成 MUST NOT 依赖 workspace 已有依赖之外的新依赖，
且 MUST NOT 要求锁定解析之外的网络访问。

#### Scenario: 在锁定的 workspace 上生成
- **WHEN** 开发者在已锁定的 workspace 上运行物料清单任务
- **THEN** 产出的文档覆盖锁文件中的每一个包，workspace 成员亦在其中

#### Scenario: 锁文件与 metadata 不一致
- **WHEN** 锁文件列出某个包，而 cargo metadata 未返回对应包
- **THEN** 生成**具名失败**，而 MUST NOT 产出一份静默省略该包的文档

### Requirement: 组件记录可溯源的字段

文档中每个组件 SHALL 记录包名、版本、包 URL 与**声明的许可**；其依赖来自 registry 的组件
SHALL 记录锁文件为该包保存的 SHA-256 校验和。

#### Scenario: 来自 registry 的组件
- **WHEN** 某组件的依赖来自 registry
- **THEN** 该组件记录一个取自锁文件的 SHA-256 哈希

#### Scenario: workspace 成员
- **WHEN** 某组件是 workspace 成员（锁文件中本就没有校验和）
- **THEN** 它与其它包一样被覆盖，且**不带**哈希——而不是被省略

### Requirement: 声明的许可不被静默改写

工具 SHALL 仅在**每一个词元**都是它愿意断言的标识符时，才把声明的许可记录为 SPDX 标识或表达式；
否则 MUST 原样记录为许可名称。声明中未指定许可的组件 MUST 仍带一条显式记录。

#### Scenario: SPDX 无法表达的写法
- **WHEN** 清单使用 SPDX 未定义的写法声明许可（例如旧式斜杠写法）
- **THEN** 它被原样记录为名称，MUST NOT 被改写成 SPDX 表达式

#### Scenario: 未声明许可
- **WHEN** 清单未声明任何许可
- **THEN** 该组件带一条显式记录，表明未作任何断言

### Requirement: 生成是确定性的

同一份锁文件上的两次生成 SHALL 逐字节相同；组件 MUST 按名称与版本排序。
文档 MUST NOT 包含在锁文件不变时会变化的字段。

#### Scenario: 同一锁文件的两次运行
- **WHEN** 任务在同一份锁文件上运行两次
- **THEN** 两份文档逐字节相同

### Requirement: 产物自述其范围与限度

生成的文档 SHALL 在**自身内部**声明它覆盖什么、不覆盖什么，以及不能作为什么证据；
该声明 MUST NOT 仅存在于本仓库的文档中。

#### Scenario: 在仓库之外阅读该产物
- **WHEN** 有人在无法访问本仓库文档的情况下阅读该文档
- **THEN** 未覆盖的范围与它的证据限度均已在文档内写明

### Requirement: 物料清单不是审计结论，也不是验收

生成成功 MUST NOT 被当作满足基线同样要求的**依赖漏洞与许可政策审计**，
亦 MUST NOT 被当作插件与供应链验收的证据；它只记录锁定了什么、以什么许可见授权，
且 MUST NOT 改变任何验收状态。

#### Scenario: 生成成功之后
- **WHEN** 物料清单生成成功
- **THEN** 它记录锁定与许可事实，且不改变任何验收项的状态、不声称安全或无漏洞

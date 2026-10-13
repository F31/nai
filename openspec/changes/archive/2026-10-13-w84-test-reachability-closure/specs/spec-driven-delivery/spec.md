# spec-driven-delivery 变更（w84）

## ADDED Requirements

### Requirement: Every tracked test file is reached by a registration site

仓库里每个**在册**（已跟踪）的 Node 测试文件 SHALL 被某个**登记站点**覆盖，或被登记在
**未覆盖台账**里。覆盖集 MUST 由站点的 marker **原文解析**得出，MUST NOT 另行声明。
每个站点声明的 marker MUST 在其 host 里**逐字存在**，且 MUST 至少覆盖一个在册文件。

#### Scenario: A new test file nobody registers

- **WHEN** 一个测试文件被加入仓库（已在 `.git/index` 里）却没有任何登记站点覆盖它，
  且它不在未覆盖台账里
- **THEN** 校验 MUST 失败并**指名该文件**——一个谁都不跑的测试文件此前**完全静默**
  （门控 `EXIT=0`、目标数与用例数不变、其用例名在日志里零出现）

#### Scenario: A site that is declared but not written

- **WHEN** 某个登记站点在表里声明了 marker，而它的 host 文件里**没有**这段 marker 原文
- **THEN** 校验 MUST 失败并**指名该站点与该 marker**——「名字在表里」不等于「线上真的有它」

#### Scenario: A site that covers nothing

- **WHEN** 某个登记站点的 marker 解析后在在册集合上**匹配不到任何文件**
- **THEN** 校验 MUST 失败并指名该站点——一条覆盖不到东西的站点是空声明，
  它与「站点数大于零」是两回事

#### Scenario: A stale exemption in the unreachable ledger

- **WHEN** 未覆盖台账里的某个文件**已经**被某个站点覆盖，或它不是被识别的测试文件、不在册，
  或缺少非空理由
- **THEN** 校验 MUST 失败并指名该条目——台账**不是豁免**，它必须自清理

#### Scenario: A test extension nobody scans

- **WHEN** 仓库里出现一个**看起来像测试**的在册文件，其扩展名不在被识别的清单里
- **THEN** 校验 MUST 失败并指名该文件与扩展名——文件层的闭世界对它**看不见**，
  因为「不是被识别的测试文件」意味着连「未被覆盖」都不会被报出

#### Scenario: An empty coverage set

- **WHEN** 覆盖集为空（站点表被清空，或全部站点解析失败）
- **THEN** 校验 MUST 失败——台账为空时「覆盖与台账不相交」退化成恒真，
  这条断言是「全不覆盖也算绿」的唯一守卫

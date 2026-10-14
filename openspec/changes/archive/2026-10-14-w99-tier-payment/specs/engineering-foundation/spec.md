## ADDED Requirements

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

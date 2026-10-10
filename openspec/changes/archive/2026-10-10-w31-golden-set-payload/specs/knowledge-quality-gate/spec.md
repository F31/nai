## ADDED Requirements

### Requirement: The case set payload has one declared location and one declared revision

§6 的 100 题与 §5.9 的黄金集 MUST 作为**场景包载荷**提供，而不是由本仓库生成。题集文件 MUST 位于 §30.1 指定的 `tests/rag-golden` 目录，MUST 为 `.json`。每个文件 MUST 声明其 schema 修订（`case_set_schema`）与场景版本（`scenario_version`）。修订不是本构建认识的那一个时，文件 MUST 被拒绝，MUST NOT 被宽松解释。未知字段 MUST 被拒绝，MUST NOT 被忽略——被忽略的拼写错误会读成「字段缺失」并静默改变用例的含义。

#### Scenario: A case set that names an unknown revision is refused
- **WHEN** 一个题集文件声明 `case_set_schema: 2`
- **THEN** 该文件被拒绝，并报出「修订不受支持」与所期望的修订号

#### Scenario: A misspelled field is refused, not ignored
- **WHEN** 一个题集顶层多出 `extra` 字段
- **THEN** 该文件被拒绝，并报出未知字段及其 JSON 路径

#### Scenario: Only case set files in the declared location are read
- **WHEN** 载荷目录中同时存在 `a.json`、`b.json` 与 `notes.md`
- **THEN** 只读取两个 `.json` 文件，并按文件名顺序处理

### Requirement: An absent payload is reported as not supplied, never as a pass and never as a failure

载荷目录中不存在任何题集文件时，校验 MUST 以 `CAPABILITY_NOT_READY` 结束，MUST 指出被搜索的目录，MUST NOT 报为通过，MUST NOT 报为结构缺陷。就位门禁 MUST NOT 并入 `check --profile portable`。三种结论——「结构合规」「从未提供」「结构缺陷」——MUST 互不冒充。

#### Scenario: An empty payload directory is not a pass
- **WHEN** `tests/rag-golden` 中没有任何 `.json` 文件
- **THEN** 校验以未就绪结束，指出该目录，并说明题集是场景包载荷

#### Scenario: A payload that is present but malformed is a defect, not a shortfall
- **WHEN** 载荷目录含一个无法解析的文件
- **THEN** 校验以「结构缺陷」结束，MUST NOT 以「未就绪」或「题数不足」结束

#### Scenario: The readiness gate stays out of the engineering profile
- **WHEN** 运行 `cargo xtask check --profile portable`
- **THEN** 不因题集未提供而失败

### Requirement: A promoted case names its confirmer at the payload boundary

§5.9 只允许坏案例**经人工确认后**进入黄金集。因此 `origin.kind` 为 `promoted` 的用例 MUST 在**读取载荷时**给出非空 `confirmer`；缺失或空白 MUST 被拒绝，MUST NOT 被推迟到运行期。本条在载荷边界生效，使「未经确认的候选冒充已确认用例」不可表达。

#### Scenario: A promoted case with no confirmer is refused while reading
- **WHEN** 一个用例的 origin 为 `{"kind":"promoted"}`
- **THEN** 读取被拒绝，并指出缺失的字段路径

#### Scenario: A blank confirmer is as absent as a missing one
- **WHEN** 一个用例的 `confirmer` 为 `"  "`
- **THEN** 读取被拒绝

#### Scenario: A named confirmer is accepted
- **WHEN** 一个用例的 origin 为 `{"kind":"promoted","confirmer":"ops-zhang"}`
- **THEN** 该用例被读入，且来源保存了确认人

### Requirement: Every structural defect in the payload is reported in full

载荷存在结构问题时，校验 MUST 报出**全部**缺陷，MUST NOT 只报第一条。每条缺陷 MUST 带稳定 token、所在文件名与 JSON 路径。用例期望与其类别不相容时，MUST 报告为**用例配置缺陷**（运行前，指向用例 id），MUST NOT 在运行期计为系统失败。用例 id 重复时 MUST 拒绝该文件——重复的用例会在召回比例中被重复计权。

#### Scenario: Several defects are reported together
- **WHEN** 一个文件同时含未知字段与空的 `cases` 数组
- **THEN** 两条缺陷都被报出，各自带文件名与 JSON 路径

#### Scenario: An expectation incompatible with its category is a set defect
- **WHEN** 一个「无答案」用例配了一条证据期望
- **THEN** 审计报出期望与类别不匹配，并指向该用例 id

#### Scenario: A duplicated case id is refused
- **WHEN** 同一文件内出现两个相同 id 的用例
- **THEN** 该文件被拒绝，并报出重复的 id

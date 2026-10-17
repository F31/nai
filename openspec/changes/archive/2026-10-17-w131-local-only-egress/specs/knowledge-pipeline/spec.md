## ADDED Requirements

### Requirement: A document marked local-only is withheld from the release list by name
候选集 SHALL 被分成「可外发」与「扣留」两桶，且两桶的元素**恰好**是输入：每个候选 SHALL 落进且只落进一个桶。被标记的文档 SHALL 带着一个稳定标记落进扣留桶，MUST NOT 只是从可外发桶里缺席——「清单短了一点」不是原因，没人能从中得知少了什么。判定的单元 SHALL 是 `(document_id, revision)` 这个身份而不是文档的名字，重复身份 SHALL 被拒而不是被计数两次：计数正确不等于清单正确。空候选集 SHALL 被清点并报 `0`，MUST NOT 被短路成跳过，也不被当成错误。

#### Scenario: Withholding keeps the reason
- **WHEN** 候选集里有一份被标记 `local_only` 的文档
- **THEN** 它落在扣留桶并带 `local_only` 标记，两个桶的元素之和等于输入，且它不出现在可外发桶里

#### Scenario: The judgement unit is the revision, not the name
- **WHEN** 同一个 `document_id` 以两个不同修订出现
- **THEN** 它们是两份候选，各自独立判定

#### Scenario: An empty domain is counted
- **WHEN** 候选集为空
- **THEN** 判定成功、两个桶皆空、计数报 `0`

#### Scenario: A repeated identity is refused
- **WHEN** 同一个 `(document_id, revision)` 出现两次
- **THEN** 判定具名失败，MUST NOT 静默去重

### Requirement: The release list is read from storage rather than supplied by the caller
面向远端的清单 SHALL 由存储按「活动快照 ∩ 未撤销」这一域读出，标记本身 SHALL 从 `documents.local_only` 读出，MUST NOT 由调用方作为参数给出——调用方给不了、也改不了那份上传范围。清单 SHALL 按 `kb_scope` 界定，另一个域的文档 MUST NOT 混进来；重新导入并改标记后，同一份文档的归属 SHALL 跟着变。列上的取值只有 `0` 是「未被标记」，任何其它值 SHALL 按**被标记**读：外发是危险方向，存疑时选不放行那一边。

#### Scenario: The mark follows storage, not the caller
- **WHEN** 一份文档被重新导入并改了标记
- **THEN** 清单的归属跟着变，判定面拿到的标记来自存储

#### Scenario: A stored value that is neither zero nor one
- **WHEN** `documents.local_only` 读到 `0` 与 `1` 之外的值
- **THEN** 该文档按被标记处理，MUST NOT 按「它看起来像 0」放行

#### Scenario: The domain is the active snapshot
- **WHEN** 同一份文档在多个曾发布过的快照里都有行且修订未变
- **THEN** 只有活动快照的那一行算数，该文档 SHALL 只被数一次

#### Scenario: A revoked document is outside the domain
- **WHEN** 文档已撤销
- **THEN** 它不在任何一个桶里，撤销与扣留是两个不同的原因

### Requirement: Withholding is not removal from local retrieval
被扣留的文档 SHALL 仍然被本地词法检索返回。§281 说的是「只走本地词法检索或人工」，不是「不可用」：不外发不等于不可检索，这两半 MUST 同时成立——只钉住了前半的判据会让「扣留」在实现上退化成删除。

#### Scenario: A withheld document is still retrievable locally
- **WHEN** 一份被标记 `local_only` 的文档被扣留在面向远端的清单之外
- **THEN** 本地词法检索仍能命中它

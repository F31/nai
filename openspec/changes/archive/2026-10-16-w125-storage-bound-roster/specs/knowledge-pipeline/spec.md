## ADDED Requirements

### Requirement: The published snapshot bound is the sum over the selected documents
快照发布 SHALL 按**所选文档的合计**判定分块数与分块字节的上限；两份各自合法的文档合起来越界时 MUST 被拒绝，且计数越界与字节越界 MUST 是**可区分**的具名拒绝。

#### Scenario: Two admissible documents whose sum crosses a bound
- **WHEN** 合集内每份文档单独看都在上限内
- **THEN** 该合集仍可被拒绝
- **WHEN** 越界来自分块数而字节远低于上限
- **THEN** 拒绝理由指向计数上限
- **WHEN** 越界来自合计字节而分块数远低于上限
- **THEN** 拒绝理由指向字节上限

## Purpose
让操作员从已发布且当前有效的产品FAQ取得有来源的私有答案，明确无命中与歧义，并通过有期限、可取消、可复检的建议将人工确认内容追加到草稿，始终与自动发布及公共输出分离。
## ADDED Requirements
### Requirement: Exact FAQ lookup is grounded and bounded
系统 SHALL 只在显式请求时执行有界精确FAQ匹配并返回当前已发布事实的来源、条件和版本。无命中、冲突或缺依据 SHALL 不产生猜测答案，Provider调用与API费用为零。
#### Scenario: Unknown or conflicting question
- **WHEN** 已发布FAQ不存在精确匹配或存在冲突答案
- **THEN** 返回明确未命中/歧义状态，没有可采纳建议且不调用模型。
### Requirement: Suggestion validity follows preparation context
任务与建议 SHALL 有独立状态、期限和精确输入引用；读取与采纳 SHALL 复检actor/角色版本、当前对象版本及事实撤销。取消、失效和切换上下文后迟到响应 MUST 不恢复旧卡片。
#### Scenario: Product changes after lookup
- **WHEN** 查询完成后产品草稿、主持、系列、节目或来源许可变化
- **THEN** 旧候选被标失效，不能继续读取候选正文或采纳。
### Requirement: Adoption creates only a reviewed draft
采用/编辑 SHALL 在单一事务中CAS追加产品话术、写结案和审计，同一任务最多结案一次；编辑须显式人工确认。任何结案 SHALL 不授予发布、播放或外发权限。
#### Scenario: Failed audit or competing adoption
- **WHEN** 采纳时审计失败或同一任务已经结案
- **THEN** 不产生部分草稿更新或重复段落，已有产品和有效撤销保持正确。

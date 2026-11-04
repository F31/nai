## Purpose
让操作员在独立工作区配置LLM并以显式预算发起有依据的私有短备稿，保持控制面可取消、费用可观察、凭据不泄露，候选必须经人工核对进入草稿而不自动发布或外发。
## ADDED Requirements
### Requirement: Model settings isolate credentials and require explicit execution
系统 SHALL 保存有版本的模型元数据和绑定工作区/端点的密文，不从旧安装取Key或向页面返回Key；打开/保存配置 MUST 不调用模型。
#### Scenario: Endpoint change or missing native credential
- **WHEN** 用户修改端点但未提供新Key，或本机凭据不可用
- **THEN** 拒绝危险复用或拒绝生成，不回退旧Key/本地模型或假成功。
### Requirement: Bounded tasks preserve financial uncertainty
任务 SHALL 先持久预留预算与派发意图，再单次调用有界API；控制面在请求中仍可取消。已发送后的取消/超时/崩溃 MUST 保留未知费用且不自动重发。
#### Scenario: Cancel and restart after dispatch
- **WHEN** 在请求派发后取消或引擎退出
- **THEN** 迟到正文不成为可用候选，重启不重放请求，未知金额仍占用额度。
### Requirement: Draft adoption rechecks exact context
输出 SHALL 只成为有来源的私有待核对建议，读取/采用复检精确版本、权限、事实和期限；采用/编辑只原子追加草稿。
#### Scenario: Context or audit changes before adoption
- **WHEN** 产品/主持/节目/模型/许可变化，或采纳审计失败
- **THEN** 不发布/播放陈旧文本，不出现部分草稿更新或重复采纳。

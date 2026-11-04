# llm-local-readiness Specification

## Purpose
在操作员发起私有备稿前，以无供应商请求、无持久任务副作用的本机检查说明当前上下文、凭据绑定、预留估算与预算限制。检查结果不替代生成时复检，不授予远端认证或调用许可。

## Requirements

### Requirement: Local checks have no paid or durable task side effects
系统 SHALL 只读检查当前任务的本机资料、凭据绑定和预算，不请求供应商、不预留费用或创建任务。
#### Scenario: Repeated checks with insufficient budget or broken binding
- **WHEN** 操作员打开发布页或更改本次模型/额度，工作区预算不足或本机凭据不可解密
- **THEN** 返回明确原因和可取得的估算，任务/outbox/预算/审计及幂等账本不变，密钥不返回页面。

### Requirement: A successful local check grants no execution authority
系统 MUST 在实际生成时重新验证当前资料、权限、预算和模型，界面 SHALL 区分本机检查与远端认证。
#### Scenario: Context or budget changes after a successful check
- **WHEN** 检查后资料、模型或预算变更，再执行生成
- **THEN** 重新计算并拒绝不再满足的条件，不以旧检查放行；界面迟到检查不能覆盖新上下文。

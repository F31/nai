## Why
RF04模型配置缺少独立API测试，真实验收仅有笼统ProtocolError。依据V4.3预算/任务约束及INV-11/35/40，提供明确计费的设置页测试，ACC保持pending。
## What Changes
新增已保存模型的显式单次测试，固定无业务数据输入，独立预算预留/结算和取消；显示有限脱敏响应失败分类、正文及费用状态。测试不创建产品/节目，不产生可采用建议，不授予直播Ready。
## Capabilities
### New Capabilities
- `model-api-test`: 模型设置中的受预算单次API响应测试。
### Modified Capabilities
无。
## Impact
先修改protobuf/JSON Schema生成，再修改writer、单worker、adapter、host和UI。无新依赖/常驻服务，不搬取旧Key。本轮自动验证仅用本地明确合成Provider，不追加真实付费请求。

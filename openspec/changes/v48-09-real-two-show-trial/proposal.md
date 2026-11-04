## Why
V48-08已实现问题记录到复盘及独立下场草稿，但隔离样例、实际API与原生IPC证据不能代替真实产品的第二场使用。按V48-09和V4.8附录O，先执行一名操作者的一对真实任务，保留来源、实际使用、人工/等待用时和未满足条件，再推进3–5人及自然机会观察。
## What Changes
建立可执行的首对场次现场记录和任务卡；预先固定入口条件、步骤、失败条件和证据口径。核实真实资料/使用权与本机或平台范围，实际执行第一场→问题/复盘→独立草稿→重新复核发布→第二场，记录版本链与用户实际采纳。不补造基线、录音同意、用户反馈或平台完成；资料未确认时停在入口条件。
## Impact
本轮首先是现场验收，不预设新增运行时功能。允许路径docs/acceptance/trials、docs/development/tasks、docs/development-plan.md、docs/v48-development-plan.md、docs/reports、相关OpenSpec；发现缺陷另按源契约→消费者→定向测试处理。无新增常驻依赖，无数据库迁移。涉及ACC-VALID-01、ACC-UE-02、ACC-REUSE-01/02、ACC-FLOW-01/02、ACC-LIVE-02及INV-01–62适用闭包；局部试验不升级164ACC。

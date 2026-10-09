## 1. 规格
- [x] 1.1 核对 O02/S02 与审批契约，strict 校验工件。
- [x] 1.2 修改 control.yaml 标实现；生成并校验契约无 drift。
## 2. 存储审批闭环
- [x] 2.1 storage 实现待审批列表、审批读取与拒绝推进（有界/过期/权限），补测试并接入决策日志。
- [x] 2.2 control-api 接线 GET /approvals 与 approve/reject，补鉴权/上限/幂等测试。
## 3. 交付
- [x] 3.1 portable、contracts、schema 检查通过，更新 registry/计划/报告/指南。
- [x] 3.2 strict 校验并归档，声明真实候选/UI/手机审批未实现（归档结果见报告）。

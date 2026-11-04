## Why
创建工作区在持久成功后会停止本机音频并切换，新音频命令未进入Tauri AppManifest/ACL，导致后续操作拒绝而只显示通用错误。重复点击可能新增重复工作区，旧烟测空根未覆盖原生命令。
## What Changes
- 增加只含停止/读取停止代次的独立窄权限，manifest与主窗口能力绑定。
- 连接就绪前实际验证这两个原生命令，使桌面烟测覆盖权限边界。
- 创建已成功但列表/切换失败时保留新ID，下一次仅恢复读取/切换，不再次写创建。
- 显示缺原生宿主/缺音频权限的具体提示。
## Capabilities
### New Capabilities
- `workspace-create-recovery`: 工作区创建后失败恢复与实际原生权限就绪检查。
### Modified Capabilities
## Impact
新桌面壳权限、前端连接/创建流程和定向测试。无Schema/协议变更，不删除重复或现有工作区，不改变Key，不调用Provider。

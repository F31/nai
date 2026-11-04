# 模型列表直接删除（2026-10-22）

原“移除模型”隐藏在高级区，易用性不足。本轮改为列表每行直接“删除”，高级区保留参数与独立删除Key。删除仅修改草稿并清除对应待保存Key，其他记录和Key保留；选中项被删后回到剩余首项。至少保留一个同类模型，最后一项禁删并解释可编辑更换；运行中继承禁用。明确提示保存后生效，不新增确认弹窗。

未修改契约/数据库格式/原生逻辑。Provider及易用性测试16/16、typecheck/生产build与Chrome生产交互通过（本地宿主夹具，覆盖删除主模型、选中项回退、最后一项禁删、不自动保存及既有Key隔离）。未调用付费API或重跑全量测试，registry不升级。桌面壳已在用户退出后完成维护更新；日志model-delete-ui.log、model-delete-browser.log、model-delete-build.log。

同身份Engineering静态CRT桌面壳已构建并暂存，OpenSpec归档为2026-10-22-r1-model-list-delete。已在用户退出后替换安装壳；候选及维护脚本为model-delete-installed-candidate.exe/model-delete-install-shell.ps1。

安装检查：SHELL_UI_READY，GUI subsystem=2；8个用户数据文件数量/哈希不变，引擎和keeper哈希不变。备份：`E:\projects\nai\target-windows\installation-backups\before-model-delete-20261022-082647`。安装壳SHA256：`88ca51be7ffb9217478b4e14d2a98185e5a748d170ab3ca1cfb5291db7dcd89f`。日志及结果：model-delete-installed-smoke.log/model-delete-install-result.json。无付费调用，未启动引擎。

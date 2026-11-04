## Why
RF03草稿与发布后端已实现，普通用户仍无法通过新界面备播。本任务完成工作区/内容/节目/发布复核与私有纯文字预演纵向链路，对应V4.3第13–16章、INV-05/13/23/26/30及ACC-CONT-01/02、ACC-MODE-01局部证据，整体ACC不自动通过。
## What Changes
- 增加有界工作区/备播对象列表和经门控的私有预演、前端呈现回执、停止与查询契约。
- 桌面壳只经固定新引擎通信，可信宿主持有本地主体和临时认证；常驻owner支持用户编辑期间空闲，不因4秒无请求退出。
- 新界面提供产品事实/FAQ/话术、场次与主持偏好、影响复核/发布/撤销、私有预演；无技术文件路径或Provider要求。
## Capabilities
### New Capabilities
- `preparation-workbench`: 普通用户备播与私有呈现链路。
### Modified Capabilities
## Impact
新线Protobuf/JSON Schema、SQLite/engine/host、新桌面壳及src/v43。复用已有依赖，不新增服务、端口或付费调用。新线格式2根与本地主体隔离，旧安装/数据/Key不改动；公共输出/媒体/平台及智能生成仍未实现。

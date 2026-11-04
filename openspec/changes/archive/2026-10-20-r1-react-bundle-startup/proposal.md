## Why
用户安装后黑屏，控制台useState dispatcher为null。生产bundle含两份React，旧smoke只判断导航完成，漏检应用根实际渲染。
## What Changes
Vite显式去重React/react-dom解析；以真实浏览器加载生产构建验证界面和错误；安装冒烟增加应用实际挂载握手，不能以导航完成报成功。
## Impact
R1桌面启动缺陷修复，不改内核/Provider/凭据与跨语言契约。复用本机缓存Chromium及既有Tauri事件；无新增运行依赖。只执行前端/壳启动和安装相关验证，不跑无关全量。

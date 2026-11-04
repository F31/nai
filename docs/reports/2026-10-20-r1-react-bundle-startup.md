# 安装后黑屏：React重复实例修复（2026-10-20）

## 根因与证据
用户从开始菜单运行D:/nai Engineering/nai-desktop-shell.exe，窗口标题正常但root为空。实机WebView已加载http://tauri.localhost/与深色CSS；用户控制台显示index-DVNtQIsM.js中的useState dispatcher为null。该生产bundle含两份React，组件和react-dom未共享实例。无头Chromium也复现同一异常，故不是用户代理问题。开发工具请求.well-known/appspecific/com.chrome.devtools.json被CSP拒绝只是工具探测，与React异常分开；没有放宽CSP或修改系统代理。

apps/desktop/vite.config.ts新增resolve.dedupe=['react','react-dom']，参考[Vite官方解析去重说明](https://vite.dev/config/shared-options.html#resolve-dedupe)，使用同一份React。修复构建69个模块，原构建73个；实际浏览器加载后原生设置、LLM/TTS表单与启动按钮可见，无useState异常。

## 漏检修正
之前SHELL_PAGE_LOADED只证明导航完成，React崩溃同样可能被标成功。因此此前安装报告中的“页面加载通过”不得解释为界面已完成渲染或人工验收。

新增无参数frontend_ready诊断握手：App实际提交DOM、主机设置及Provider配置区存在后才发送。宿主导航完成和挂载两条件同时满足才SHELL_UI_READY；20秒未挂载非零退出。该握手不授予引擎/设备/Provider就绪，不包含Key。权限逐条加入既有allow-host-setup，安装烟测同步改查新标记。没有改跨语言数据结构或业务协议。

## 验证范围
按用户要求，仅验证受影响前端构建/界面、原生启动判据和安装产物，不重复知识、Provider、升级等无关全量。

- 前端typecheck/build、119项UI测试通过。
- 启动状态2项有意义测试：导航完成但应用崩溃不通过；先挂载仍需等导航。独立壳all-targets clippy干净。
- scripts/test-frontend-bundle.mjs通过：真实生产dist、产品同一CSP、缓存Chromium headless shell、明确的只读原生宿主测试桩，验证真实控制台/两组配置/按钮及握手，不调用API。命令需显式NAI_TEST_CHROMIUM，无浏览器具名失败不跳过。
- 首次完整版Chromium因环境启动超时无DOM，未计通过；切换已缓存独立headless shell后通过，不放宽渲染断言。错误浏览器文件名一次具名ENOENT已修正测试命令。

日志：/tmp/nai-react-startup-build.log、/tmp/nai-react-bundle-test3.log、/tmp/nai-react-ui.log、.nai-diagnostics/react-startup-native.log。Windows新二进制--smoke实际返回SHELL_UI_READY、EXIT=0；非smoke真实窗口已用computer-use读取和截图确认：工作区/预检、两家模型与音色、保存Key状态、主备路由、启动按钮均已渲染。当前未启动引擎。

不删除或重建用户数据库/OS凭据；没有新增付费API请求；现有安装器仍拒绝活动/覆盖安装，不能当升级器。源码改动尚未提交。

新NSIS工程包10,924,851字节，SHA256 374bfef4fed642f6779bc84737888552844bb0c1684a6f8782f7fef9fba199b9，NotSigned。日志.nai-diagnostics/react-startup-package.log、react-startup-windows-smoke.log。用户现用D:/nai Engineering安装及窗口未替换；新构建在E:/projects/nai/target-windows/shell/x86_64-pc-windows-msvc/debug/运行供体验。本轮没有重新卸载/安装用户正在使用的版本，安装器逻辑未变；安装验证脚本已改查实际挂载标记，后续正式安装回归使用此判据。

OpenSpec归档2026-10-20-r1-react-bundle-startup；43个主规格严格校验通过，168 项归档一致性3/3通过。本轮没有执行无关全量测试。

## 用户实际安装残留修复

用户再次遇到NAI_FIRST_INSTALL_ONLY。实机查明卸载登记已无，但D:/nai Engineering旧二进制和uninstall.exe仍在；期间旧黑屏壳再次启动，确认无engine/keeper后仅关闭该旧壳，再用产品自带卸载器清理。卸载器复制自身后异步收尾，不能只用首个进程退出认定文件已删；确认目录已消失后才安装已验证的374bfef4…修复包。NSIS含空格/D尾部参数按[官方文档](https://nsis.sourceforge.io/Docs/Chapter3.html)传递，不经cmd拼接。

已将新包实际安装回D:/nai Engineering；该目录二进制--smoke EXIT=0并返回SHELL_UI_READY。用户AppData配置/数据库6个文件逐项SHA256保持不变，无API调用。安装二进制与未封装构建仅3字节差异，未将其报告为逐字节相同；以验证过的安装器哈希、实际安装界面挂载及数据保留证据确认本次结果。实际安装exe SHA256 6f77cdfa44fb2d679705a455b009238c59b57301054c83f68196e7d0606b6d84；证据.nai-diagnostics/repair-existing-install-result.json和repair-installed-ui-smoke.log。

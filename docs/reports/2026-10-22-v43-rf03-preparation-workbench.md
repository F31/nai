# V43-RF03 备播工作台与私有预演

2026-10-22。实现/定向实测：M1首场手工备播工程链路贯通。RF03的产品/草稿、节目、发布复核与私有ContentOnly预演已接入独立新工作台；RF03完整活动会话/媒体影响处置仍须RF05/06，90项ACC全部pending，不授予整卡/产品认证。

## 本轮交付

- 新页面提供工作区、产品事实/来源/确认/有效期、FAQ/话术/禁说项、主持偏好、系列、场次单元排序/段落选择、发布影响复核与历史快照。普通用户不用编辑场景/profile文件或JSON。默认主持与系列是可编辑的真实偏好草稿，不代填产品事实。
- 修改事实清除旧确认；金额以分的十进制字符串保存，时间保留毫秒；删除事实不会自动把话术重新绑定到所有事实。未保存修改阻止切换，CAS冲突不覆盖。
- 桌面可信宿主持有本地主体清单与临时父子token，固定foundation进程为唯一writer。浏览器不能提交主体/token/目录，不能调用原始PutObject、授权或通用外发接口。只通过有界protobuf访问新格式2数据根；旧安装不参与。
- 新desktop-owner profile允许空闲编辑，单请求4秒deadline、单槽输入/输出、100万序号上限；EOF/壳退出回收，写入结果未知不重发。原64请求/60秒演练入口保留其显式限制。列表按ACL过滤、每页最多32条、最多256条的开发容量，容量不足明确拒绝；不自动展示历史预演对象。
- 预演仅取精确发布快照/hash/cue的共享话术，经当前角色版本、主体/session/epoch、事实有效期及产品版本撤销复检。60秒有效，前端每秒复检，隐藏/停止/过期/失联清空并拒绝迟到响应。停止时在途请求尚未确认则提示再次确认，不声称后端已停止。
- Submitted与FrontendPresented分开：回执绑定精确卡片hash及当前上下文，只代表前端报告已显示，不代表人已读、硬件播放、OBS或平台完成。私有正文不经过TTS/keeper/媒体，不调用Provider。

## 定向验证

- WSL：storage 39项通过；包括分页/越权、预演提交/呈现回执/停止/重启、错误hash/cue、到期、角色版本变化与产品版本撤销。既有发布水位/竞态/审计失败撤销用例继续通过。
- WSL真实host-owner新增2项通过：空闲4.2秒后仍可编辑、连续66次读取超越旧64请求限制、关闭重开后记录及主体不变、原始写接口拒绝、缺失/损坏主体清单不覆盖数据。
- Windows MSVC：新增host-owner 2项通过；storage 38项通过（Linux专用1项不计入）。独立壳构建通过、真实Tauri生产挂载/权限/owner连接冒烟EXIT=0，PE子系统GUI。受限运行环境首轮通信失败，同一原生用例在允许子进程通信环境通过；storage首轮LNK1104，单目标重试通过，均保留诊断，不归因为代理。
- 新表单/通信及bootstrap/CI定向Node 12项通过；覆盖金额精度、事实/毫秒有效期往返、确认清除、空绑定不补全、写入未知结果不重试、有界分页及卡片hash/状态/期限。
- `cargo xtask check --profile workbench`通过：仅新模块和直接消费者，含核心/引擎回归、依赖/registry/静态检查、前端typecheck/生产build。`schema --check`无漂移，157条protobuf消息；`test --suite contracts`九个显式夹具双向Rust/TS/C++互通通过。没有全workspace测试。OpenSpec增量已归档，全部50项严格校验通过。
- computer-use浏览器操作生产构建，通过显式临时本机测试bridge连接真实Windows新引擎：创建工作区→产品保存→默认节目→保存引用→影响复核→发布→预演→前端呈现回执→停止→重连恢复记录且不重播；预演中撤销旧事实版本后清空/禁用；更正450毫升、重新确认/保存场次/发布后显示新内容，旧500毫升快照仍阻断。最终构建确认“修改来源清除确认”已实际生效，60秒到期也已实际清空。

诊断与截图位于忽略目录`.nai-diagnostics/rf03-*`。旧RF01专用浏览器脚本已被新场景替代；`scripts/serve-preparation-test.mjs`仅显式测试使用，不打包/生产启动。测试数据均为临时虚构内容，与用户库隔离。

## 使用与剩余边界

人工体验入口：`scripts/start-preparation-workbench.ps1`，只启动独立新工作台并设置本次子进程所需程序路径，避免用户手填参数。开发壳为`target-windows/shell/debug/nai-workbench-desktop.exe`；debug设置绝对路径`NAI_FOUNDATION_ENGINE`指向`target-windows/rf01/debug/nai-foundation-engine.exe`。诊断页另需`NAI_WORKBENCH_ENGINE`。release只接受同目录固定新程序；尚未制作新安装包。标准开发数据位于新壳app_data_dir的`foundation-v2`与`operator-v43.json`，不得把旧数据库放入或手工重建主体清单。

本轮没有更新已安装旧线程序、读取旧Key、付费API或公共媒体操作。Windows ACL/远程身份、真实用户可用性认证未验收；完整LiveContext/媒体队列/缓存/索引传播、公共声音形象、真人接管/插问归位、OBS和平台权限尚未实现。下一主线RF04：先实现有依据的私有备稿/问答候选与独立预算/取消，真实API任务另取明确预算授权。


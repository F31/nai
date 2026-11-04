## Decisions
采用独立nai.workbench.v1消息命名空间、wire major=2、64KiB小端长度前缀，拒绝旧v1/未知枚举；Schema为唯一类型源。最小引擎只提供inspect/prepare依赖分析，不授予业务输出，未实现ContentOnly业务也明确not_ready。新root不引用legacy runtime/provider/media，运行不触碰旧数据库。native壳通过继承私有pipe和随机短时进程token认证单次读取；UI只看到能力报告，不见token或Key。桌面提供独立新入口，保留旧文件作为待清理历史，不建立接口兼容adapter。注册表切换包含旧语料只读归档与新检查消费者同卡更新，所有ACC保持pending，仅记录RF01前置证据。依赖方向包含传递和目标分支。
## Verification
契约生成无漂移/三语言编译、正负例/协议拒绝/有界输入/认证、真实引擎进程和实际桌面组件/原生宿主集成；ACC逐项完整和禁止旧状态搬运、INV/任务引用，依赖注入负例。仅相关模块和直接消费者，不全仓。Windows构建/进程证据独立于WSL逻辑，不调用付费或替换安装。

## Final assembly
原生入口为apps/workbench-desktop独立Tauri workspace/标识，复用crates/nai-workbench-host窄宿主；Vite独立dist只含v43.html，不修改旧壳命令或混用旧安装。活跃CI使用workbench profile、新contracts suite及Windows新壳构建/冒烟；旧CI与契约编排归档追溯。ADR0148和kernel迁出清单记录边界。

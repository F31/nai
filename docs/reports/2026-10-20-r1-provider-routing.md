# LLM / TTS 独立配置、一主多备与智能路由（2026-10-20）

## 交付

原生设置面板可分别填写LLM/TTS协议、基础地址、模型、会话Key、超时、预算币种/价格版本与TTS音色；每组1主最多4备，顺序可调整。支持健康主优先、已观测低延迟优先、失败阈值及熔断冷却。新增停止引擎动作，运行中拒绝改配置，未保存编辑阻止启动/预检。

保存到数据根独立场景副本并将素材引用绝对化，源演示包不改。所有Key只注入本次engine环境，启动/预检后界面清空；没有密钥持久化。新环境名只能NAI_PROVIDER_*，ID/环境名唯一，币种一致，URL禁止内嵌凭据或查询参数。原生权限allow-host-setup精确增加read_provider_routes/save_provider_routes/stop_host_engine并同步枚举测试。

路由位于engine应用装配层，每次操作固定实际profile/client/model/voice，装配记录和持久化usage保持同一身份。失败只改变后续新请求的选择；已派发请求不会跨供应商重发。熔断、单次恢复试探与延迟均值内存有界，不持健康锁跨await。预算未派发、取消、换代不计供应商故障。无新服务；壳复用workspace已锁定jsonschema 0.58.6及其锁文件闭包。

## 验证

- 路由状态机3项：主失败后的下一请求转备、冷却到期单试探/恢复、取消不计故障、缺凭据/全部熔断显式拒绝、延迟模式及阈值。
- 真实engine+本地HTTP服务：LLM主503后本次不重发、下一提问转备；TTS主503后本次不重发、下一获批候选转备；核对独立模型、不同Authorization、备用音色、实际批准文本，持久化账本精确5条（主文1、备文2、主语音1、备语音1）。未访问付费账号。
- Windows独立壳：lib 10项、真实engine集成6项通过；新增完整配置复制/多Key注入/预检/停止后修改闭环。all-targets clippy -D warnings通过。
- 前端：独立能力字段、优先级重排、秘密不进入保存payload、非法配置、数量上界与实际组件渲染/运行中禁改测试；115项前端检查通过，其中6项配置面板测试。
- schema --check无漂移：117条Protobuf、10个TS源。此功能只改配置schema，未手改生成文件。

## 失败与修复记录

首次引擎进程回归继承代理导致本机协议超时，清代理后重跑；新测试初版使用错误手工入口/空的可选音色及遗漏的fixture鉴权变量，均修正后真实协议测试通过。首次Windows壳因未指定已有侧车路径失败，指定NAI_SIDECAR_DIR后运行；补齐ApplyRequest新字段和独立测试引擎路径。新集成用例与既有用例并行碰到HOST_PREFLIGHT_BUSY，改为测试共享锁串行，产品忙碌保护保持不变。首次full指出新宿主文件的平台分支未登记，已补入data-root能力，未放宽平台断言。实际组件渲染测试首次因pnpm链接路径解析出不同React实例导致useState为空，测试改用react-dom实际peer实例，6项配置UI测试通过，未替换产品组件。

## 边界

预检只联网探测当前LLM；TTS和备用未实际探测时connectivity仍not_ready，不把配置当认证。低延迟策略只比较本次进程已成功调用的观测值，不额外付费探测、不并发竞速。故障后的同一候选不自动再次合成，需要新候选及审批。无每节点实时健康仪表盘、凭据持久化或跨币种自动成本路由。真实付费认证、商业音色、供应商账单和真实生产延迟未验证，registry状态不提升。

操作见[配置指南](../provider-routing.md)，决策见[ADR 0135](../adr/0135-provider-routing-boundary.md)。改动未提交。

## 最终检查与安装产物

cargo xtask check --profile full 最终EXIT=0：107个Rust目标、1232 passed/0 failed，Node合计459 passed/0 failed（含前端115）。schema --check无漂移，Windows壳lib 10+真实engine集成6通过，all-targets clippy干净。非生成文件git diff --check通过。

新版未签名工程安装包：target-windows/shell/x86_64-pc-windows-msvc/debug/bundle/nsis/nai Engineering_0.1.0_x64-setup.exe；10,033,188字节，SHA256 9c4019a6beae3bb2d4125abb376d41e79ae2b4a65791432a7d97fdf5b9d633ed，NotSigned。实际安装/活动进程拒绝/覆盖与备用目录拒绝/随包预检/页面加载/卸载通过，显式保留旧数据，数据库等3个文件哈希不变；没有重新初始化或删除用户数据。上轮卸载器已归档，仅清理已核实的空烟测目录。

检查日志：WSL /tmp/nai-routing-full2.log、/tmp/nai-routing-substitution3.log、/tmp/nai-routing-schema-check.log；Windows .nai-diagnostics/routing-shell-integration2.log、routing-package.log、routing-install-smoke.log；工程包与安装证据位于windows-package/summary.json和windows-install-smoke/summary.json。

OpenSpec已归档为2026-10-20-r1-provider-routing，43个主规格严格校验通过；165项归档的提案一致性3项测试通过。

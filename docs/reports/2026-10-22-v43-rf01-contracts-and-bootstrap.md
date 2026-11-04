# V43-RF01：新契约、验收体系与最小可构建装配

日期：2026-10-22。状态：RF01范围已实现并定向实测；90项ACC均pending，产品业务与安装发布未验收。设计依据V4.3、ADR0147/0148；代码未提交，保留既有未提交改动。

## 交付

- Schema先行生成Rust/TS/C++：五维配置、major=2认证有界IPC、Prepare/CapabilityView、错误（含取消/背压/冲突/未知结果）、ID/revision/scope、最小DeliveryUnit及分层证据。配置/清单用JSON Schema，HTTP OpenAPI 3.1为contract-only，无虚构HTTP服务。
- 34项能力目录仅冻结inspect/prepare两项，32项业务目标draft。新engine真实拒绝旧协议、错误认证/seq/TTL/未知配置、超长/截断输入及旧数据根；不加载旧runtime/kernel/Provider/media，不写数据库或Key。
- 独立GUI壳通过窄宿主启动新engine（Windows CREATE_NO_WINDOW），256-bit随机短时token与请求ID独立；单并发、5秒回收。新应用标识与数据根隔离，前端只接收有界protobuf结果；新dist不包含旧控制台入口。
- ContentOnly/HumanAssist依赖不含TTS/形象，Voice不含avatar，Avatar显示相应未开始依赖；所有模式业务未就绪。UI读取与Prepare不调用付费API、不启动媒体进程，也不假装已取得租约/授权。
- 原registry/Schema/catalog/checker/test/CI/契约及工程profile编排只读归档并记录SHA256，active tests/registry.yaml为90 ACC/40 INV/12 RF唯一真相。Schema/检查器拒绝缺失、重复、需求漂移、旧编号、非法引用及以不完整证据标tested。
- 新core传递依赖门槛、adapter互依赖负例及kernel业务迁出责任清单完成。CI已切换RF01实际范围；远程GitHub CI本轮未触发，不能称远程流水线实测。

## 实测与产物

| 范围 | 本机结果与证据 |
|---|---|
| WSL Rust1.96/Node22.23.1/protoc3.21.12 | schema生成后--check无漂移；见.nai-diagnostics/rf01-schema.log、rf01-contracts.log |
| 契约互通 | Rust/TS/C++编译，3种显式夹具，Rust→TS→C++→Rust及Rust→C++→TS→Rust；含uint64最大值；不是运行/设备证据 |
| 定向Rust | runtime-core 3、真实engine进程6、依赖守卫10、旧profile拒绝1、wire2、生成检查器2，均通过；无全仓测试 |
| 定向Node | registry6、Schema/catalog5、CI2、bootstrap客户端/原生入口4，均通过；恶意状态/未知枚举/重复能力/长度/自动重试负例有断言 |
| 新前端 | 契约TS及desktop typecheck、新入口Vite生产构建；61 modules，无旧页面产物 |
| Windows MSVC原生 | 新engine进程6、宿主真实集成2通过；新桌面构建及clippy -D warnings通过；见rf01-native-process/host/desktop/clippy.log |
| Windows桌面 | 实际WebView加载生产包→窄宿主→新engine→校验后frontend_ready，--smoke EXIT=0；脚本复检PE GUI subsystem，见rf01-native-smoke.log |
| Chrome生产交互 | 真实新engine测试bridge完成默认ContentOnly、切Avatar读取、Prepare；3次读取、0付费/媒体/DB写；见rf01-browser.log及rf01-workbench.png |

命令：cargo xtask check --profile workbench；cargo xtask test --suite contracts；Windows宿主测试需显式NAI_WORKBENCH_ENGINE指向本轮新引擎。scripts/test-workbench-desktop.ps1接收两程序绝对路径做原生冒烟；scripts/test-workbench-bundle.mjs显式测试bridge不属于生产HTTP API。日志/截图为忽略的本机诊断产物，不提交大型生成物。

OpenSpec已归档为2026-10-22-v43-rf01-contracts-and-bootstrap，主规格workbench-bootstrap生效。凭据扫描覆盖可发布文件及本机诊断，无已知前缀凭据；该扫描不宣称识别任意无前缀密钥。

## 本轮发现并修复

OpenAPI 3.0不接受外部JSON Schema元字段，改为3.1；契约包运行dist须先构建，避免TS类型已新而打包运行导出仍旧；初次core测试过滤器未覆盖单元测试，已补跑3项；新Tauri应用缺图标资源，改为源码生成原创图标；新壳与旧壳完全分包，独立dist避免夹带旧UI；归档CLI自动创建的Purpose占位已替换为实际用途，再验证主规格。前期WSL自动审批服务网络中断已恢复，未绕过安全否决。以上失败不计Passed，最终以日志实际结果为准。

## 未完成与下一卡

RF02–12仍未开始：没有新业务库、用户工作区/产品/节目、发布、Session/DeliveryGate、真实API任务、音画设备、平台接线或新安装器。当前新窗口只用于架构验证，不能直播。所有ACC继续pending；MODE/DEP/SEC少量前置证据complete=false，不代替整项功能。

下一卡V43-RF02：先定义存储/身份/版本及命令Schema，再实现唯一writer、ACL/CAS、审计/outbox和私有ContentOnly交付门；验证并发/重启/取消/跨空间/OutcomeUnknown，之后RF03提供普通用户能操作的手工备播闭环。旧用户资产导入、维护安装与付费请求另行明确，不在本轮发生。

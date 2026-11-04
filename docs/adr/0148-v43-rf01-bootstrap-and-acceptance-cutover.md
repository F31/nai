# ADR 0148：RF01契约、验收切换和独立最小装配

日期：2026-10-22。状态：已采纳并完成RF01范围实现/定向实测；业务验收未完成。

## 决策

1. schemas/workbench/bootstrap.proto使用nai.workbench.v1类型命名空间，线协议major=2；64KiB小端长度前缀、单请求seq=1、最长4秒TTL。每次由固定父进程生成256-bit临时认证token和独立request_id，经继承pipe单次调用，宿主最迟5秒回收；不开放loopback HTTP。OpenAPI只是后续目标声明。
2. 配置五维独立，内容/真人辅助不强制TTS、Avatar/Voice按声明列出媒体依赖。当前只实现能力读取和Prepare依赖报告；business_ready恒false，业务能力not_started，Prepare不生成Session/授权/租约。最小DeliveryUnit、证据层次及取消/冲突/未知结果错误是契约，执行留RF02之后。
3. 新组合为nai-runtime-core、nai-workbench-engine、nai-workbench-host、独立nai-workbench-desktop。新GUI应用标识cn.nai.workbench.bootstrap，数据根bootstrap-v43隔离；不读取旧库/Key，不写业务数据，已有非空root/reparse路径拒绝。GUI和子进程启动均无控制台窗口；只包含新v43页面。
4. tests/registry.yaml切换为V4.3 v2唯一实际注册表，完整对账90 ACC/40 INV/12 RF；tests/legacy/v311保存原始语料/hash和退役CI/契约编排，不提供备用验收入口。所有90项pending，RF01只记不完整前置证据；旧测试通过不沿用。新workbench-bootstrap注册表条款显式取代admission-registry的旧编号/发布线规则，后者仅保留V3.11历史适用域。
5. 活跃CI使用check --profile workbench和新的contracts suite：改动模块/直接消费者、Linux三语言契约、Windows真实子进程/独立桌面构建冒烟。旧portable/windows-local/full工程编排已归档，命令具名LEGACY_PROFILE_RETIRED拒绝，不再运行旧注册表消费者；不能当新V4.3工程或发布通过。保留历史源码不构成接口兼容承诺；后续逐卡清理。
6. 内核迁出按架构清单实施；目前无新生产adapter，不创建空端口工程。新core禁止访问旧kernel/shared业务实现和具体adapter/SDK；新增职责须扩展metadata/负例检查。

## 限制

继承pipe短时认证不等于Windows命名管道ACL、远程配对或恶意同用户进程隔离认证。声明的离线/资源零调用只适用于本次无业务能力的bootstrap，不表示真实直播已可用。RF02负责新单写owner、身份/ACL/CAS/outbox及DeliveryGate，RF03之后才有可用备播任务。旧安装、数据库和凭据保持原状，无付费请求。

证据见[RF01报告](../reports/2026-10-22-v43-rf01-contracts-and-bootstrap.md)。

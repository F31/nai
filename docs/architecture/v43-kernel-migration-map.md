# V4.3 kernel业务迁出清单

2026-10-22，RF01源码盘点；本表是迁移责任，不表示旧业务已迁完。新生产组合为workbench-desktop → workbench-host → workbench-engine → runtime-core → contracts，不链接旧nai-kernel、nai-runtime或Provider/媒体实现。旧文件保留追溯，不能作为新接口兼容层。

| 现有源码 | 新职责/首责卡 | 迁移与删除条件 |
|---|---|---|
| crates/nai-kernel/src/registry.rs、registry/、assemble.rs | 稳定能力端口/组合根，RF01/02/10 | RF01仅新inspect/prepare；后续按manifest绑定和撤权重写，不复制旧ABI/Ready语义 |
| session.rs、duty.rs、scheduler.rs | 生命周期/epoch/有界调度留稳定核；目标许可与值守策略归policy，RF02/05/06 | 新Session与DeliveryGate测试完成；不恢复旧批准/租约 |
| evaluate/、review.rs | policy/agent评估与私有建议，RF02/04 | 使用不可变事实、LiveContext和独立预算；不能由kernel引用Provider |
| demo.rs、bridge.rs | demo/collaboration领域与外部接续adapter，RF09 | 显式同意、OutcomeUnknown和真实接收回执重新取证 |
| platform.rs、multiplex.rs | platform/channel端口和adapter，RF08 | 官方事件、账号/房间/代次/权限证据齐备；不能把未知权限当可用 |
| realistic.rs、device.rs | 模型准入/输出设备adapter与独立worker宿主，RF05/10 | 不在稳定核加载SDK/GPU；目标Windows设备和模型独立认证 |
| metrics.rs、monitoring.rs | audit/telemetry与观测端口，RF02/12 | 有界、敏感信息过滤；观测失败不得阻塞P0 |
| golden.rs | tests中的确定性夹具，RF02–12按消费者 | 演练证据保持simulated，不能加载进真实生产装配 |

apps/engine/src/operator.rs、program.rs、pipeline.rs的编排按content/live/agent/runtime划分；旧壳provider_db.rs的写owner迁入新storage（RF02/04），UI仅调用命令。迁出按实际消费者逐卡进行，不先批量搬目录或创建空crate。受许可约束的素材/现有用户数据不随代码删除。

当前门槛：cargo xtask check-deps检查normal/build/dev/target与传递依赖；新core只可到contracts/core或明确声明的port/algorithm，拒绝旧shared存储/Provider/宿主/媒体及SDK/HTTP依赖。adapter禁止互相依赖。RF01尚无业务adapter，负例用真实metadata及内存图验证，不声称生产插件认证。

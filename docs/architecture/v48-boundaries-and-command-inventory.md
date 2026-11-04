# V4.8物理边界、现有映射与命令清单

2026-10-24，V48-01。默认目标遵循V4.8§37.2–37.4，工程命名nai-*。本表不是领域已经迁出的声明。

| 默认领域/宿主 | 现有真实位置 | 后续迁出/实现任务 |
|---|---|---|
| nai-contracts | crates/nai-contracts、schemas、packages/contracts-ts | 保持公共类型无执行；媒体契约独立职责按实际消费者拆出 |
| nai-kernel | crates/nai-kernel（主要旧线）；新线nai-runtime-core中有机制和业务混合 | 仅保留注册/生命周期/有界调度/epoch/权限执行；按06/11迁出业务规则，不视旧kernel已符合全新版 |
| nai-content | nai-runtime-core/preparation/publication；storage-sqlite/publication等 | 04/06迁为领域核心与端口，SQL留adapter，core无数据库连接 |
| nai-live | 新workbench状态及私有preview/assistance/llm编排 | 06/07/08迁入program/scenes/session/assist/review/operations，不建review独立服务 |
| nai-policy / nai-identity | nai-runtime-core/foundation与owner角色门控 | 02/06拆清角色/撤销/permit端口；policy不引平台SDK |
| nai-cost / nai-storage | storage-sqlite预算/owner/outbox；runtime-core/workspace_budget | 02/03保留唯一writer，调用成本与商务限额分账；不复活旧批准 |
| nai-commerce | 尚无新线完整实现 | 10/13独立binding/presentation/action/limits/reconcile，不能扩binding代替actions |
| nai-performance | 尚无新线完整实现；旧2D实现可审查复用 | 12内部command/intent/planning/tracks/compatibility，不因职责名再造同义crate |
| nai-agent / nai-knowledge | runtime-core/llm及engine lanes；旧nai-knowledge-core可评估 | 06共用Harness/LiveContext/取消树，领域端口不依赖adapter |
| nai-presentation / nai-collaboration | 新线未开始 | 14独立受控演示与咨询/认领/交接，不合并GPU设备或直接改事实表 |
| engine组合根 | apps/workbench-engine；apps/engine为旧线 | engine可装配adapter；runtime领域调度不能反向绑定具体adapter，main不承载新增领域业务 |
| desktop | apps/workbench-desktop（ui为唯一在用前端，旧apps/desktop已退役） | 沿已有用户入口增量重构，不重复建第三套前端，不将目录重命名计为功能 |
| keeper / media-worker | 新线public-output-keeper已有独立待机窗口，audition-worker仅私有音频；正式media-worker/公共音轨未实现，旧输出不计完成 | 05建立正式协议/输出/P0/租约，按profile单个媒体worker；控制与媒体数据面分离 |
| voice-input / platform-bridge / native-host | 新线未完整实现 | 10/12/14仅在ASR/SDK/不可信native条件成立时启动，未选包无守护进程 |

依赖规则：公共contracts被核心/领域/adapter引用；领域通过端口协作；adapter实现端口；engine注册并装配。content不反向依赖live，review通过草稿端口提交；commerce不直接调用LLM或循环引用runtime；第三方native/媒体/keeper保留故障边界。共享模型只共享配置，不共享私有知识与正文。

不因目录表批量创建空crate。本轮没有改变crate依赖或增加进程；后续任何合并/拆分均须在对应任务ADR提供实际依赖或故障证据。现有check-deps只证明当前层级检查范围，不证明所有未来公开端口已迁出。

## 结构单源裁决

Protobuf为跨语言消息唯一源；JSON Schema为清单、配置、业务工件结构源；HTTP使用OpenAPI。附录M的动作业务对象采用JSON Schema生成数据类型和文档例子，Protobuf信封承载引用/版本或由生成器派生映射；严禁将同一对象在JSON Schema和proto手写两遍。对应业务对象尚未实现前不标Frozen。当前schemas/catalog/source.json仍索引实际capabilities.json，后续多源索引由13卡实现，不能声明附录M清单已经运行。

## 工程命令现状

| 实际入口 | 当前作用 |
|---|---|
| cargo xtask check-deps | 当前workspace层级依赖检查 |
| cargo xtask check --registry | Node真实注册表/证据检查 |
| cargo xtask schema [--check] | Rust/TS/C++及配置/HTTP生成和漂移 |
| cargo xtask test --suite contracts | 现有三语言契约及fixture互通，非全V4.8认证 |
| cargo xtask check --profile workbench | 当前备播基础工程闭包，非所有164ACC |
| cargo test -p 指定组件 / node --test 指定文件 | 按修改闭包的定向验证 |
| pnpm exec openspec validate/archive | 规格格式/归档，不替代实际测试 |

N.3中的schema-check/arch-check/test-contract/test-e2e/assembly-check/bench-topology/certify-profile/collect-evidence/package是目标语义，当前未提供这些同名完整实现；不得添加空成功别名。schema-check和arch-check部分职责当前分别由schema --check/check-deps覆盖，不能把局部等价扩大成完整支持。未实现命令是工程待实施，不是平台Blocked。新增工具分别归属11/12/15并接真实消费者/CI。

CapabilityStatus的安装/认证/外部权限/运行健康、RunPlan条件闭包和ReleaseManifest能力承诺仍待11等任务实现；本轮注册表枚举与排除条件校验不等同这些能力已完成。



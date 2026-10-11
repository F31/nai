# 平台化审视：实现重心、组件边界与多维能力差距（V3.7 对照）

- 日期：2026-10-11
- 基线：[直播数字人 + 智能体系统技术方案 V3.7.md](直播数字人%20%2B%20智能体系统技术方案%20V3.7.md)（ADR 0042）
- 依据：`tests/registry.yaml`（66 条验收）、`Cargo.toml`（18 包）、`schemas/catalog/capabilities.json`（27 能力）、`docs/development-plan.md`
- 性质：**审视与建议**，不是变更。本文不改任何 `status`、不新增验收、不建 OpenSpec 变更。

## 0. 结论摘要

1. **实现重心已经定形为「口径先于实现」**：仓库最强的地方是**判定机制**而非功能面——kernel 里 14 个模块几乎都是「把 V3.7 的一句验收写成纯函数 + 稳定标记 + 三态 readiness」，真值后补。这使 66 条验收里 **tested 10 / implemented 27**，但 `implemented` 的 27 条中**绝大多数缺的不是代码，是真值**。
2. **智能体与数字人的边界必须定成「数字人是被调用的能力，不是系统主体」**。系统主体是 L0 内核 + L4 智能体；形象/音色/出画都是 L3/L2 的**可替换能力**。守住这条，组件边界与可组合性才成立。
3. **四条接缝（契约 / 宿主 / Provider / 知识）已经存在且可用**，但**平台接缝缺失**：`platform-bridge` 未建，A03「新增平台连接器不改内核」**目前没有落点**。这是「开放兼容」的第一道关，也是「第一期 Windows、后续 macOS」之外更紧迫的一条。
4. **发现两个口径缺口（本次新证据）**：
   - **开放平台六契约在能力目录里不存在**：`tests/registry.yaml` 引用了 `data.query@1`、`device.control@1`、`market.catalog@1` 共 **8 处**，而 `schemas/catalog/capabilities.json`（§4.1 的**唯一事实源**）**一条都没有**——`check --registry` 也**不校验 `contract` 字段**（该词在 `scripts/contracts/registry.mjs` 里一次都没出现）。
   - **macOS 与移动端监控没有验收条目**：AGENTS.md 明写「验收唯一来源为 `tests/registry.yaml`」，而 66 条里**没有任何一条**覆盖「macOS 部署」；移动端只有 O03（`blocked`，且其口径是「路线 A/B 定案 + 最小形态通过 S09」）。
5. **优先级建议**：**先补平台接缝与契约冻结（仓库内可控）→ 再补「最后一公里」（安装器/桌面壳）→ 最后才是外部真值**。理由见 §7：外部真值类（B01/B02/O04/P01、写实 Gate、平台准入）**不能靠排期解决**，而平台接缝与安装器**能**，且是「极简部署/平滑升级/开放兼容」三个维度的共同前置。

## 1. 现状：实现重心在哪

### 1.1 一张表看清 66 条验收

| 状态 | 条数 | ID |
|---|---|---|
| `tested` | 10 | S03 S04 S05 S09 S11 R02 R04 A04 A05 A07 |
| `implemented` | 27 | S01 S02 S06 S07 S08 S10 R01 R03 R05–R11 A01 A02 A03 A06 A08–A11 B03 B04 B05 B06 |
| `pending` | 21 | O01 O02 O05 **M01–M18** |
| `blocked` | 5 | B01 B02 O03 O04 P01 |
| `not_applicable` | 3 | S12 X01 X02 |

读法：**`tested` 集中在「可在本机/CI 复现的工程与安全纪律」**（依赖边界、IPC、过载、关键提交、契约兼容）；**`implemented` 集中在「机制完整但真值未接」**（Provider 真实链路、升级器、接管、运营口径）；**`blocked` 全部是外部真值**（题集+真实知识库、模型 Gate、手机路线、干净 Windows 安装、官方 SDK）。

### 1.2 实现重心的三个「形状」

**形状一：判定骨架密集（kernel 14 模块）**

```
session  scheduler  registry  duty  metrics  review  golden  realistic
monitoring  device  bridge  demo  multiplex  evaluate
```

这 14 个模块里，`realistic/monitoring/device/bridge/demo` 五个是 W32–W35/W49 的**同一手法**：§ 条款 → 稳定标记集合 → `readiness()` 三态（`Blocked`/`Defective`/`Ready`，**`Defective` 优先**）→ 用「构造描述的审计并集**恰为** N 条」的断言双向钉住。`metrics/review` 是运营口径（分位、复盘字段），`golden/evaluate` 是知识与评估判据，`multiplex` 是多会话隔离，`registry` 是能力登记与绑定解析（`prepare/freeze/recheck/release` + `CapabilityView` + `BindingSnapshot`），`scheduler/duty` 是队列原语与值守。

**这是本仓库最大的资产，也是最大的解释成本**：外部看会以为「功能都做完了」，实际是「**怎么算达标**已经写死了，真值还没接」。

**形状二：垂直链路已经打通一遍（Slice-0 已闭环）**

```
手工事件 → 归一化/检索 → 决策 → 守门 → 生成 → TTS → 2D 帧 → 双水位提交 → 播放完成 ACK
                                    ↘ 决策日志（哈希链、只追加、可回放）
engine（真实装配：控制面 + supervisor + broker/keeper 单帧投递 + 强杀撤权 + 预热期 halt 不提交）
```

配套的真值证据：真实文本/语音两条链路经本栈走通并产出真值音频（`docs/reports/evidence/`）。**R01/R03/R07/A01 的机制面已实现**，缺的是硬件 ACK、真实设备与长播。

**形状三：两处「最后一公里」尚未开建**

| 缺什么 | 证据 | 影响的多维目标 |
|---|---|---|
| **安装器 / 干净 Windows 安装** | O04 `blocked`；`docs/deployment.md` 只到「包能不能装」的判定 | 极简部署、平滑升级 |
| **桌面壳** | `apps/desktop/package.json` 依赖只有 `react`/`react-dom`/`vite`/`typescript`——**是纯 Web 应用，不是 Tauri 应用** | 极简部署、高效运维 |
| **平台桥接** | `native/contracts` 只有契约；无 `platform-bridge` 实现 | 开放兼容、第一期可用性 |

> 第三行最要紧：A03（R1 验收）是「**新增平台连接器，不修改 kernel 与 agent 代码**」。**没有桥接实现，这条验收没有落点**——而 §1.4 把 R1 定义为 Lite 的退出条件。也就是说，**Lite 目前缺一条能真正证明「开放兼容」的验收**。

## 2. 定位审视：智能体与数字人的边界

### 2.1 分层的正确读法

V3.7 §3.1 的 L0–L5 不是「技术栈分层」，是**控制流分层**——「上层只依赖下层的能力契约；跨层调用经内核做权限检查并记入决策日志」。

| 层 | 内容 | 在 nai 里的落点 | 不该出现什么 |
|---|---|---|---|
| L5 体验 | 桌面控制台、手机监控包、团队控制台 | `apps/desktop`（TS）、`nai-control-api` | **不持有媒体时钟与密钥**；不直连数据库 |
| L4 智能体 | 决策、守门、记忆、技能、节目编排 | `agent.decide`/`guard.check`/`memory.access`/`skill.invoke`/`show.plan` 契约 | 策略**不进内核** |
| L3 能力 | LLM、TTS、知识、渲染 | `nai-provider-core`、`nai-knowledge-core`、`plugins/avatar-2d` | 不绕过守门发声 |
| L2 媒体与输出 | keeper、worker、输出适配 | `apps/output-keeper`、`apps/render-worker`、`nai-media-core` | **不直接调用 LLM/TTS 公网接口** |
| L1 接入 | 手工/回放连接器、平台连接器 | `platform.events` 契约 + `native/contracts` | 不伪造官方 SDK |
| L0 内核 | 八项机制 | `nai-kernel`（只依赖 `nai-contracts`） | **不含业务策略**（§3.2） |

### 2.2 三条推论（决定了组件边界）

**推论一：数字人是「被调用的能力」，不是系统主体。**
`avatar.render` / `tts.synthesize` / `output.deliver` / `avatar.profile` 都是**契约**，谁实现都行——2D 插件是本仓库的实现，写实是 R2 的另一实现，远程渲染是 R3 的第三种。**因此「换形象/换声音」的正确做法是换 Pack（资产包）或换 Provider 绑定，不是改内核。** 这是「可组合性」的第一块地基。

**推论二：智能体是可替换插件，不是内核的一部分。**
§3.2 明写「策略不在内核：内容策略、风险分级表、知识排序、LLM 选择、节目脚本、**播报调度策略**都是可替换模块或配置。内核只提供队列与令牌原语」。仓库符合度：**A04 已 `tested`**（plugins 之间零依赖、kernel 不依赖 plugins），由 CI 强制。**这条守住了，「一个内核、多档位」才不是口号。**

**推论三：安全不变量优先于任何档位与扩展声明。**
§33.3 的 INV-22–25（扩展不得绕过守门/授权/审批/租约/日志；实际权限不得超出声明；返回一律是数据且带来源与时效；撤销立即生效）。**它们的实现位置不是插件，是宿主**——「出站网络、数据范围、凭据、设备能力由宿主强制执行，不靠自觉」。仓库符合度：`nai-plugin-hosts` 有 `acl.rs`/`boundary.rs`（有落点），但 M02（出站白名单）/M06（撤销生效）仍是 `pending`。

## 3. 组件边界：四条接缝 + 一处缺失

「可扩展性 / 可组合性 / 松耦合」不是形容词，在 nai 里它们是**四个具体的可替换点**：

| 接缝 | 位置 | 现在的能力 | 缺口 |
|---|---|---|---|
| **契约接缝** | `schemas/catalog/capabilities.json`（27 能力）+ `Draft→Frozen(v1)→Deprecated` 冻结规则 | 14 个插件契约 + 4 宿主原语 + 9 个扩展能力（`platform.capabilities`/`lifecycle`、`media.preview`、`embedding.generate`、`rerank.rank`、`moderation.check`、`classification.classify`、`skill.list`、`gateway.invoke`）已在册 | **开放平台六契约不在册**（见 §3.2） |
| **宿主接缝** | `nai-plugin-hosts`（inproc/subprocess/remote/wasm） | **inproc + subprocess** 已实现，信任域由签名派生、inproc 仅 official/enterprise | remote/wasm 只有契约（**这是对的**，§1.4 明写不预建） |
| **Provider 接缝** | `nai-provider-core`（`ProviderProfile`→`ProviderClient`→适配器） | **A02 已 implemented**：仅改配置换 LLM/TTS 供应商、下一句边界生效、不拼接音频；两家协议（OpenAI 兼容 / Anthropic Messages） | 真实账号/配额是外部条件；`/v1/models` 清单是权威 |
| **知识接缝** | `nai-knowledge-core` | 本地检索 + 第三方知识 API 统一契约 | B01 门禁需 100 题集 + 真实知识库（外部）；W50 已登记外部语料**素材**来源 |

**松耦合的可检验形式**（不是口号，是可断言的）：
- 依赖方向由 `check-deps`（18 包）+ CI 强制（**A04 tested**）；
- 降级由**数据**驱动而非代码硬编码（§4.2「降级表是数据，不写在插件代码里」）→ 现在 `CapabilityView` 由 `registry.prepare/freeze` 产出，**已有落点**；
- 契约兼容由 A07（内核与 N-1 次版本互通，**tested**）与「同一主版本内只增可选字段」保证。

### 3.1 一处缺失：平台接缝

`platform.events` 契约在册，但**没有桥接实现**。后果：

- A03 无落点 → **R1/Lite 少一条能证明「新增平台连接器不改内核」的验收**；
- §9.2 抖音接入、§18.1「exe 与小程序产物类型选错且不可变更」（**高危**）都没有可验证的桥接边界；
- 「第一期先支持 Windows」在**输出侧**（直播伴侣/虚拟音频设备）恰恰是最 Windows 绑定的部分。

### 3.2 缺口一：开放平台六契约「有名无册」

§37.4 明写：

> `data.query@1`、`data.subscribe@1`、`device.sense@1`、`device.control@1` **先以 Draft 加入注册表**；`asr.transcribe@1` 为预留；`market.catalog@1` 为客户端契约。

实测（本次）：

```text
schemas/catalog/capabilities.json   中 data.query / data.subscribe / device.sense /
                                    device.control / market.catalog / asr.transcribe
                                    出现次数 = 0        ← 六条一条都没有

tests/registry.yaml 引用：
  skill.invoke@1        × 8   ← 在册 ✓
  market.catalog@1      × 6   ← 不在册 ✗
  control.manage@1      × 2   ← 在册 ✓
  guard.check@1         × 3   ← 在册 ✓
  knowledge.retrieve@1  × 1   ← 在册 ✓
  output.deliver@1      × 2   ← 在册 ✓
  storage.persist@1     × 1   ← 在册 ✓
  data.query@1          × 1   ← 不在册 ✗
  device.control@1      × 1   ← 不在册 ✗
```

**而 `check --registry` 抓不到**：`scripts/contracts/registry.mjs` 只校验 `id` 格式/唯一性、`release_line`、`status` 枚举与 mapping 目标；**`contract` 字段完全没有校验**（该词在文件中一次都不出现）。

**这意味着**：「开放兼容」这个维度目前的地基是**可空的**——注册表可以说某条验收属 `market.catalog@1`，而该契约既无 schema 也无目录条目，门控不会报错。

> 处置建议（本审视为**建议**，不擅自改）：两条路选一——① 按 §37.4 把六条作为 **Draft** 补进能力目录（**推荐**：设计文档已经承诺了，补上即「有名有册」，且 Draft 不生成消费者代码、不违反契约冻结规则）；② 若坚持「不预建」，则应在 `registry.mjs` 加一条**显式校验**：`contract` 非空时必须在目录中可解析，且 registry 里引用未登记契约时**具名失败**——否则这个洞会一直在。**③ 两条都做最稳**（先补 Draft，再让门控保证不再出现「有名无册」）。

## 4. 可扩展 / 可组合 / 松耦合：逻辑分析

### 4.1 可扩展性：三个「加东西不改内核」的路径

| 想加什么 | 正确路径 | 该改哪里 | 不该改哪里 |
|---|---|---|---|
| 换 LLM/TTS | 改配置（Provider 绑定） | 配置 + 适配器 | 内核、智能体 |
| 加数据源/业务系统 | 加 Connector（声明式 HTTP/Webhook） | 清单 + 契约 | 内核 |
| 加知识 | 加文档 → 入审核发布流程 | 知识库 + 事实版本 | 内核、守门 |
| 加形象/音色 | 加 Pack（资产包） | 清单 + 素材哈希 | 内核、渲染插件代码 |
| 加直播平台 | 加平台连接器 | `platform-bridge` + 契约 | **内核与 agent（A03 的全部意义）** |

**当前可走通的路径**：换 LLM/TTS（A02 implemented）、加知识（机制在）、加形象（`avatar-2d` 是一个实现）。
**当前走不通的路径**：加直播平台（**无桥接**）、加 Connector/Skill（M01–M07 `pending`，`skill.invoke@1` 在册但无实现）、加设备（M09，`device.control@1` 不在册）。

### 4.2 可组合性：三件「能装在一起」的前提

1. **同一套清单/签名/信任/权限/生命周期/撤销**——§33.2 明写「五类扩展共用同一套……不为市场另建一套」。仓库里这套机制**已存在**（`nai-plugin-sdk` 清单 + `nai-plugin-hosts` 信任域 + `nai-security` 签名/撤销），M05/M06（重新授权/撤销生效）是把它接上扩展。**这是「可组合」而不是「可堆叠」的关键分别。**
2. **档位是预设不是分支**——§14.1 末句「矩阵是默认预设，不是禁止加装」+「同一升级路径」。仓库里 `nai-upgrade-core` 的迁移图**不分档**（A08 已 implemented），**这条做对了**。
3. **能力视图是唯一显示源**——§4.2「界面用同一视图显示已降级项」。`CapabilityView` 契约与产出点已在。

### 4.3 松耦合的三个反例（需要在扩展开放前立规矩）

| 反例 | 为什么会发生 | 现在的护栏 | 建议 |
|---|---|---|---|
| **把业务逻辑写进 `nai-control-api`** | 它是桌面/手机的唯一通道，最方便 | 目前它**仍是薄投影**（`ApprovalItem = ResourceItem` 等），未腐化 | 在 Panel 扩展（R3）前明确：控制面**只做投影与命令转发** |
| **把降级判断散落在各模块** | 每个 kernel 模块各写各的 `readiness()` | 各自正确，但**缺一个统一导出** | 未来做「开播体检」时，需要一个**聚合视图**把 14 个模块的 readiness 汇总（§15 的「开播体检」列了 10 项） |
| **把「档位」编码进代码** | 「Lite 少点东西」最省事 | A08/A09（驻留与回收）已 implemented，走配置 | 保持；新功能默认走能力登记，不走 `if lite` |

## 5. 多维能力平台差距矩阵

用户提的七个维度，逐条落到「现状 / 缺口 / 归属验收」——**含本次新发现的空白**：

| 维度 | 设计落点 | 现状 | 缺口 | 归属验收 | 可否仓库内解决 |
|---|---|---|---|---|---|
| **统一平台**（同内核多档位） | §1.2 固定约束、§14.1 | A04 tested、A08/A09 implemented；迁移图不分档 | 无重大缺口 | A04 A08 A09 | ✅ 已基本达成 |
| **平滑升级** | §14.3 九步事务式流程 | `nai-upgrade-core`：`plan`/`gates`/`journal`/`preflight`/`verify`/`revocation` 已 implemented | **真实安装器**（O04 干净安装）与 **update-helper**（Windows 文件锁，未建）；S08 的供应链审计（advisory 数据源）与真实签名链仍缺 | R05 R09 R10 R11 S08 O04 | ⚠️ 一半（流程可，安装器与真机不可） |
| **极简部署** | §14.2 Local 零自建云、§15 10 分钟首句 | Local 架构成立；演示包判定机制已建（W49） | **安装器**（O04 blocked）、**桌面壳**（`apps/desktop` 是 Web 不是 Tauri）、一键连通真实接线 | O01 O04 | ⚠️ 桌面壳可，干净安装不可 |
| **灵活配置** | §14.1 预设可加装、§4.2 降级表是数据 | 配置 schema 在册；`CapabilityView`+`BindingSnapshot` 有落点；A02 可换供应商 | **降级表「数据化」**尚未统一导出 | A02 A08 | ✅ 可 |
| **开放兼容** | §33–§38、§4.5 | `skill.invoke@1` 在册；`nai-plugin-sdk`/`hosts` 在 | **平台桥接未建**（A03 无落点）；**六个开放契约不在能力目录**（§3.2）；M01–M18 全 `pending` | A03 M01–M18 | ⚠️ 契约与技能可，平台桥接不可（需官方 SDK） |
| **轻量敏捷** | §14.2 默认不依赖 Redis/NATS/K8s；§14.4 未启用能力不占资源 | **18 包**、依赖方向不变、每轮新增能力均**零新增外部依赖**、`Local` 零自建云、portable 全覆盖、`unsafe_code = "forbid"` | 桌面壳若引入 Tauri 会新增依赖（需按 §14.2「新增常驻依赖须记录成本」走 ADR） | A08 R11 | ✅ 可 |
| **高效运维** | §5.9 复盘、§12.5 指标、§23.5 保留配额 | `metrics`/`review`/`monitoring`/`decision log` 口径已 implemented；手机 `MobileView` 十字段白名单已建 | **真实运维数据**（长播、真实分位）；手机**应用**未建（O03 blocked） | B04 R06 B06 R01 O03 | ⚠️ 口径可，真值与应用不可 |

**一句话**：七个维度里，**能在仓库内单独完成的只有「统一平台」「灵活配置」「轻量敏捷」三个**；「平滑升级」「极简部署」「开放兼容」「高效运维」各自都有一半卡在**安装器 / 桌面壳 / 平台桥接 / 真机**上。

## 6. 平台面：Windows → macOS → 移动端

### 6.1 现状：平台相关代码的分布（实测 87 处 `cfg` 分支 / 15 个文件）

| 集中处 | 数量 | 说明 |
|---|---|---|
| `nai-media-core`（frame_slots: broker/shared_ram/consumer） | 34 | **共享内存帧传输**是最平台绑定的部分 |
| `apps/engine`（lib 10 + tests 8） | 18 | 装配与测试夹具 |
| `nai-security` | 6 | **凭据存储** |
| `nai-storage/src/paths.rs` | 4 | 数据目录 |
| `apps/render-worker` | 3 | 推理侧 |
| `apps/output-keeper/tests/faults.rs` | 3 | 故障演练 |
| 其余（avatar-2d、runtime、storage tests） | 4 | — |

**工作区依赖里 `keyring = { ... features = ["windows-native"] }`**——**凭据存储当前是 Windows 原生的**，macOS 需要 `apple-native`（或等价的系统钥匙串实现）。

### 6.2 macOS 的抽象缺口清单（面向「后续支持 macOS」）

按「改造成本从低到高」排：

1. **凭据存储**：`keyring` feature 从 `windows-native` 扩为按平台选择（`apple-native`）。**低**——依赖本身跨平台，只是 feature 选择。
2. **数据目录**：`directories` crate 已跨平台，`nai-storage/src/paths.rs` 的 4 处分支需核对。**低**。
3. **共享内存帧槽**：`shared_ram.rs` 现在按 Windows/Linux 分支；macOS 需 POSIX `shm_open` 或 `mach` 等价物。**中**——有明确对应物，但要新写实现并过 R03 故障隔离。
4. **IPC**：Windows 命名管道 ↔ POSIX 本地套接字。**中**。
5. **音频输出与设备枚举**：Windows 音频（虚拟音频设备、回环、设备变更）是**最重的一块**，且与「不重复收音」「回声」等 B05 验收直接绑定。**高**。
6. **桌面壳**：无论 Windows 还是 macOS 都还没有（`apps/desktop` 是 Web）。若用 Tauri，**其 macOS 打包、签名与公证是与 Windows 完全不同的一条流水线**。**高**。
7. **平台桥接**：官方 SDK 若只有 Windows 版，macOS 端**结构性不可用**——这不是工程量问题，是**能不能做**的问题。

> **诚实边界**：macOS 支持在 `tests/registry.yaml` 里**没有任何验收条目**，而 AGENTS.md 规定「验收唯一来源为 `tests/registry.yaml`」。⇒ **macOS 目前是「未在册的需求」**。要真正推进它，第一步不是写代码，是**先立验收**（建议一条 A 系列，例如「A12 macOS 上完成演练闭环，CapabilityView 正确显示降级项」，与 A01 同形）。§6.2 的第 5–7 条很可能决定它是「可做」还是「只做 Lite/纯逻辑」——**建议先做这个判断，再投入**。

### 6.3 移动端第一期：监控与管理

**好消息**：判据已经建好了——O03 的**判定机制**（W33）把 §10.2 的约束落成了可断言的东西：

- `MobileView` **结构上只含十个白名单字段**（mode / current_utterance / pending_seconds / event_source / api_status / budget_status / device_status / pending_approvals / alerts / enable_status）；
- `LiveState` 的 `media_clock` / `secret` / `raw_audio` / `raw_video` / `broadcast_control` **在投影之上没有落脚处**（否决「事后剔除」：给 `LiveState` 加字段就会默认流出）；
- `EXPOSURE` 白名单与 `FORBIDDEN` 集合互补且不相交，**加字段漏登记即测试失败**；
- 最小形态是**四项清单**（看板、审批、接管、紧急静音），预览与推送**可后置**。

**坏消息**：O03 是 `blocked`，卡在**M0b 的路线 A/B 定案**（A：中继+PWA，需有效 HTTPS 域名与证书，局域网明文/自签名不满足 Service Worker 安全上下文；B：原生壳+局域网直连+证书固定，需要应用分发，Tauri 2 移动端成熟度待验证）。§17.4 给了止损点：**两条路线都无法低成本交付时，最小形态只做局域网看板与紧急静音，原生壳后置**。

**建议**：移动端第一期的**技术风险不在"监控能做多少"，而在"路线选哪条"**。而 §10.2 的功能集（看板/审批/接管/静音）**全部已经在控制面契约里**（`control.manage@1`，A07 tested 的契约兼容）；`MobileView` 白名单也已建。⇒ **路线一旦定案，第一期工作量主要落在「壳 + 配对/设备身份 + 命令防重放与 ACK（S09 已 tested）的移动端接线」上，不是重写后端。**

## 7. 优先级重排建议

### 7.1 排序原则（三条）

1. **能靠排期解决的先做**——外部真值（题集、模型、官方 SDK、干净机）**不能靠排期解决**，别把主线押在上面；
2. **复用的前置先做**——「平台接缝 / 契约冻结 / 安装器 / 桌面壳」各自是多个维度的**共同前置**；
3. **不越证据升级**——按 `admission-registry` 的诚实回填，逻辑/界面出现**不等于**验收达标。

### 7.2 三批建议

**第 1 批（仓库内可控，解锁多个维度）**
- **P1-a 平台接缝的「可验证替身」**：为 A03 建一条**可执行的**落点——用一个**显式 Unavailable / 演练用的本地连接器**证明「新增连接器不改内核与 agent」（§17.2 明写「尚未取得官方接口的连接器只保存契约、准入说明与显式 Unavailable 实现，不放伪造 SDK」）。**不假装是抖音**。
- **P1-b 开放契约「有名有册」**：按 §37.4 把 `data.query@1`/`data.subscribe@1`/`device.sense@1`/`device.control@1`/`asr.transcribe@1`/`market.catalog@1` 以 **Draft** 补进能力目录；**并**给 `check --registry` 加「`contract` 必须在目录中可解析」的校验（否则洞会重现）。
- **P1-c M01–M07**（`skill.invoke@1` 已在册，纯仓库内）→ 兑现「开放兼容」的第一批可验证能力。
- **P1-d 桌面壳与安装器的技术决策**：Tauri？自研？还是「先 Web + 本机服务，安装包后置」？**这是极简部署与平滑升级的共同前置，且是一个 ADR 级的决策。**

**第 2 批（需要决策或外部条件）**
- 写实 Gate（B02）与题集/知识库（B01）——**外部投入**，只做「外部接入清单」；
- 干净 Windows 安装（O04）与真实签名链（S08）——需要**受控机器 + 证书**；
- 手机路线 A/B 定案（O03 的前置）——需要 **HTTPS 域名或应用分发**；
- 长播与真实分位（R06/B04）——需要**真实负载与时长**。

**第 3 批（新需求，先立验收再接工程）**
- **macOS**：先判断 §6.2 的 5–7（音频设备 / 桌面壳流水线 / 官方 SDK）是否可做；再立一条与 A01 同形的验收；**再**动代码。当前它在 registry 里**不存在**。
- **移动端第一期**：与 O03 合流，不做第二套后端。

### 7.3 与既有计划的差异（说明为什么调整）

`docs/development-plan.md` 的「下一主线」把顺序表述为「portable 可落地项已全部完成 → 剩余都需外部条件」。**本次审视认为这句话需要一处修正**：`M01–M07`、平台接缝的可验证替身、开放契约补册、桌面壳技术决策——**这四件仍是「仓库内可控」**，却因为没有被登记为「未完成的能力」而在计划里隐形了（M 系列在 W46 才登记，且 notes 写「自动化是规划意图」）。

## 8. 需要拍板的前置决策

1. **桌面壳形态**：Tauri / 自研 / Web+本机服务？决定极简部署与两条打包流水线。
2. **开放契约是否补 Draft 进目录**（§3.2 的 ①/②/③ 选择）。
3. **平台接缝的可验证替身**是否按 §17.2 用「显式 Unavailable 连接器」落地（不伪造 SDK）。
4. **macOS 的目标档位**：只 Lite（2D + 智能体 + 纯逻辑），还是要 Standard（写实 + 音频设备）？后者受官方 SDK 与音频抽象约束。
5. **移动端路线 A/B**（与 O03 的 M0b 定案合流）。

## 附：本次新增的两个口径缺口（登记建议）

| 缺口 | 证据 | 建议处置 |
|---|---|---|
| 开放平台六契约「有名无册」，且 `contract` 字段未被任何门控校验 | `schemas/catalog/capabilities.json` 命中 0；registry 引用 8 处；`registry.mjs` 无该词 | 补 Draft + 加校验（§3.2 / P1-b） |
| macOS 与移动端监控应用无验收条目 | registry 66 条中无 macOS；O03 仅覆盖「路线定案 + 最小形态通过 S09」 | 立 A 系列条目（与 A01 同形）；移动端与 O03 合流 |

> 本文只做审视与建议：**未改任何 `status`、未新增验收、未建 OpenSpec 变更、未改代码**。若采纳其中某项，应走常规流程（proposal → 实施 → 归档）。

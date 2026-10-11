# 演示包与可用性目标的就绪判定（O01）

代码：`crates/nai-kernel/src/demo.rs`；测试：`crates/nai-kernel/tests/demo.rs`；
决策：[ADR 0044](adr/0044-demo-package-readiness-boundary.md)。

本模块把 §615「演示包」的**构成要求**与 §612「可用性目标」的**时间目标**落成在 portable 下可执行、可测试的
判定骨架。它**不联网、不读文件、不测量耗时**——真实的一键连通、真实的人工演练与「**授权的**演示形象内容」
由外部条件提供；它负责的是「给定一份**声明的演示包**与**演练记录**，它是否齐备、诚实、自洽且达标」。

因此 `tests/registry.yaml` 的 **O01 保持 `pending`**：§661 的验收原文是「可用性：约 10 分钟首句演练语音；
约 30 分钟完成真实输出验证」，这些是**人工与真实端点**的真值。本文档描述的是**判定机制**，不是交付结论。

> **已另有落点**：O01 依赖的真实链路 W36/W37 已打通并落了真值证据（`docs/reports/evidence/`：
  `live-provider-bound-stack.json`、`live-provider-facts.json`、`voice-FunAudioLLM-CosyVoice2-0-5B.mp3`）。
  本模块补的是此前**没有任何落点**的那一半——演示包的构成判定（`preset`/`演示包` 两个词在
  `crates/`、`apps/`、`schemas/` 里此前一次都没出现过）与演练记录的判定。

## 未声明即条件化

| 描述符状态 | 审计 | `DemoReadiness` |
| --- | --- | --- |
| 五要素与演练记录**全缺** | 空 | `Blocked` |
| 声明了任一部分（其余成为**欠项**） | 缺项逐条具名 | `Defective` |

尚未交付**不是违规**，是**未开始**——与 O03「未声明某职责时不得按缺陷计」、O04「空描述符审计为空」同纪律。
反过来，一旦声明了任一部分，其余部分就是**欠项**：半份演示包是缺陷，不是「未开始」。

## 授权的演示形象：「授权的」是要求

§615 的字面是「**授权的**演示形象」。判据分两条，各自可判：

| 情况 | 报出 |
| --- | --- |
| 无形象 | `demo_avatar_missing` |
| 有形象但无授权引用 | `demo_avatar_without_license`（指名形象） |

## 五要素齐备性

| 缺失 | 报出 |
| --- | --- |
| 示例知识为空 | `demo_knowledge_missing` |
| 示例场景缺失 | `demo_scenario_missing` |
| Provider 预设为空 | `demo_provider_presets_missing` |
| 成本预估缺失 | `demo_cost_estimate_missing` |

## Provider 预设只携带凭据引用

与 `schemas/config/provider.schema.json` 的 `credential_ref` 同纪律（S07：明文密钥）：

| 情况 | 报出 |
| --- | --- |
| 预设内嵌明文密钥 | `provider_preset_embeds_secret`（指名预设） |
| 预设既无引用又无密钥 | `provider_preset_without_credential`（指名预设） |

内嵌了密钥的预设**不**再报「缺凭据」——它确实带了凭据，只是带错了方式。

## 一键连通测试：连通不等于达标

| 情况 | 报出 |
| --- | --- |
| 连通测试缺失 | `demo_connectivity_missing` |
| 探针是常量桩（没真的测） | `connectivity_probe_is_stub` |
| 把冒烟结果声明为验收达标 | `smoke_claims_acceptance` |

前一条判「测没测」，后一条判「有没有把冒烟当验收」——与 W36/W37 已固化的纪律一致
（**主机的连通成功不是端点的验收结果**）。

## 成本预估：未知须说明原因

与 `upgrade-transaction` 已固化的口径一致（未知 ≠ 0）：

| 情况 | 报出 |
| --- | --- |
| 某分项声明未知却未给原因 | `cost_unknown_without_reason`（指名分项） |

## 可用性目标由演练记录判定

§612 的两条目标是**设计值**，判据是「记录的耗时 ≤ 目标」；本模块**不**测量时钟。

| 情况 | 报出 |
| --- | --- |
| 记录缺项（首句或真实输出验证缺失） | `drill_record_incomplete` |
| 时间戳早于安装就绪 | `drill_timeline_inconsistent`（指名里程碑） |
| 首句耗时 > 10 分钟 | `first_utterance_beyond_target`（附实测毫秒） |
| 真实输出验证 > 30 分钟 | `real_output_beyond_target`（附实测毫秒） |
| **无**演练记录 | **审计为空**，读作 `Blocked` |

时间戳早于安装就绪是**不自洽**，不再报「太快」——一条不可能的记录不该被读成「跑得很快」。

## readiness 三态

| 条件（按序） | `DemoReadiness` |
| --- | --- |
| 审计非空（任一违规） | `Defective` |
| 未声明，或已声明但无演练记录 | `Blocked` |
| 齐备、诚实且已在目标内演练 | `Ready` |

**先取审计**——未演练不掩盖越界：一份违规的声明不得因子项未演练而被读成 `Blocked`。

> `DemoReadiness` 是**描述符**的词汇；`tests/registry.yaml` 的 `status` 是**轮次交付**的词汇。O01 的
> registry 状态保持 `pending`（库内判定已交付，真值与人工演练仍外部），二者不矛盾。

## 审计是描述符的纯函数

`audit_demo_package(&DemoPackage)` 对描述符字段做纯函数，报出**全部**命中项：

| token | 含义 |
| --- | --- |
| `demo_avatar_missing` | 无演示形象 |
| `demo_avatar_without_license` | 有形象但无授权引用 |
| `demo_knowledge_missing` | 示例知识为空 |
| `demo_scenario_missing` | 示例场景缺失 |
| `demo_provider_presets_missing` | Provider 预设为空 |
| `provider_preset_without_credential` | 预设既无凭据引用又无密钥 |
| `provider_preset_embeds_secret` | 预设内嵌明文密钥 |
| `demo_connectivity_missing` | 一键连通测试缺失 |
| `connectivity_probe_is_stub` | 探针是常量桩 |
| `smoke_claims_acceptance` | 冒烟结果被当验收达标 |
| `demo_cost_estimate_missing` | 成本预估缺失 |
| `cost_unknown_without_reason` | 未知分项未说明原因 |
| `drill_record_incomplete` | 演练记录缺项 |
| `drill_timeline_inconsistent` | 时间线不自洽 |
| `first_utterance_beyond_target` | 首句耗时超目标 |
| `real_output_beyond_target` | 真实输出验证超目标 |

「缺失」与「存在但违规」对同一部分互斥，故 `every_finding_token_is_constructible` 断言的是
**一组构造描述**的审计并集恰为这**十六条**——既保证每条可构造，也保证没有未被登记的额外标记。

## 未做边界

- **不含真实交付**：授权的演示形象内容、示例知识/场景正文、真实一键连通、约 10 分钟与约 30 分钟的人工演练
  都由外部提供。测试里的描述符是**构造出来的**，只为证明判定口径与 readiness 派生可执行。
- **不做连通**：本模块判「探针是不是真的」，**不**发起任何调用；真实连通属外部与后续里程碑。
- **不重复升级成本的权威口径**：`nai-upgrade-core::cost` 是升级成本（cloud/download/storage/runtime_delta）的
  权威；本模块只判演示包的成本预估**有没有把未知说成未知**。
- **不新增 xtask 命令与证据目录**：§30.1 为 UI / RAG / 升级 / 媒体 / 故障 / 契约 / 集成 / 性能各指定了目录；
  **演示包没有证据目录约定**，其真值来自**真实演练与真实端点**，不是仓库载荷。**臆造** `tests/demo` 属过度绑定。
- 注册表 **O01 保持 `pending`**。相关：[UI 层](ui.md)（O05 成本显示）、[升级器](upgrade.md)、
  [本轮报告](reports/2026-10-11-w49-o01-o05-in-repo-closeout.md)。

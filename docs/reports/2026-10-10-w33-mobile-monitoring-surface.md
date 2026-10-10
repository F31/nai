# W33 实测报告：手机监控面判定骨架（O03 机制侧前置解耦）

- 日期：2026-10-10
- 变更：`openspec/changes/w33-mobile-monitoring-surface/`
- 决策：[ADR 0035](../adr/0035-mobile-monitoring-surface-boundary.md)；指南：[docs/mobile-monitoring.md](../mobile-monitoring.md)
- 判据：`tests/registry.yaml` O03（回指 S09）

## 结论

O03 的**真值**（M0b PoC 定路线 A/B + 真机最小形态通过 S09）需要外部条件，本轮不可达，O03 **保持 `blocked`**——并且其 registry notes 此前只有
一句「Blocked 原因待 registry 标注」，本轮**补齐了这个原因本身**。本轮做的是 **O03 的判定骨架**：§3.1 L5 与 §10.2 给手机监控面钉了一组口径，
此前**全库没有落点**（`nai-control-api` 是桌面 loopback 传输面，其文件头明说不做设备认证/预检；`nai-kernel::platform` 管的是平台事件门），
于是「手机监控包不得越界」在执行层等于一句**无从判定**的要求。本轮把 §3.1、§10.2、§37.3、§636/§86、§742 的判定口径落进
`crates/nai-kernel/src/monitoring.rs`，四处最容易含糊的地方被钉死：

1. **「不持有媒体时钟与密钥」是投影的结构性质，不是事后剔除**——`MobileView` 结构上只含十个白名单字段，禁止类别在它之上**没有落脚处**；
   两条一致性断言把「加字段而漏登记」变成测试失败。
2. **「它不是手机开播」是一条可判的暴露类别**——`BroadcastControl` 与媒体时钟、密钥同族。
3. **「未定案」是诚实状态，不是缺陷，也不得掩盖缺陷**——`readiness` 三态且 `Defective` 优先。
4. **「如实披露」与「虚假声明」被分开**——中继终止 TLS 时如实声明不算违规，宣称端到端才算（与 W32「不以 CUDA EP 注册宣称全 GPU」同形）。

**本轮不产生任何真实交付**：移动端应用（PWA 或原生壳）、证书、分发、中继与真机 S09 集成都来自外部，不在此仓库生成。
**不新增 xtask 命令与证据目录**：§30.1 为 UI 层指定 `tests/ui`、为 RAG 黄金集指定 `tests/rag-golden`；手机监控面的真值来自**真机与 PoC**
而非仓库载荷，臆造 `tests/mobile-monitoring` 属过度绑定。

## 目标行为

| 判据（出处） | 实现 | 关键设计 |
| --- | --- | --- |
| L5 只看状态、不持有媒体时钟与密钥（§3.1） | `MobileView` / `LiveState` / `Exposure` | 白名单投影；字段集合一致性断言；**否决**黑名单剔除 |
| 最小形态四项（§10.2、§663） | `MobileCapability::MINIMUM_FORM` | 看板/审批/接管/紧急静音；缺项逐条指名 |
| 预览与推送可后置（§10.2） | 最小形态判定只看四项 | 后置**不是**缺陷 |
| 它不是手机开播（§10.2、§742） | `Exposure::BroadcastControl` | 与媒体时钟/密钥/原始音视频同族的禁止类别 |
| 预览四条件（§10.2） | `PreviewPolicy` | 按需 / 真正编码的低码率 / 关闭立即释放 / 不得挤占正式出画 |
| 预览未启用不构成缺陷（§10.2） | 仅 `enabled` 时受检 | **否决**把「不适用」读成「不合规」 |
| 中继终止 TLS 不宣称 E2E（§10.2） | `EncryptionClaim` | 两条独立规则；**如实披露无缺陷** |
| 路线 A 前置（§10.2） | `ConnectionRoute::RelayPwa` | 有效 HTTPS 域名 + 证书（Service Worker 需安全上下文） |
| 路线 B 前置（§10.2） | `ConnectionRoute::NativeShellLan` | 应用分发 + Tauri 2 成熟度已验证 |
| 未定案是诚实状态（§10.2、§663） | `SurfaceReadiness` | `AwaitingRouteDecision` / `Defective` / `Ready`，后者**优先于**第二个 |
| 远程命令契约（§86 INV-14、§636） | `RemoteCommand` | 身份/会话与 epoch/序号/TTL/实际 ACK，缺项指名 |
| 观察者不得执行高危（§636） | `CommandRisk` / `Role` | `high_risk_command_open_to_observer` 指名命令 |
| 紧急静音可达（§10.2、§37.3） | `emergency_mute_reachable` | 能力已声明却不可达即报出 |
| 启用状态与紧急停用（§37.3） | `enable_status_provided` | 缺失即报出 |
| 知识速补走审核流（§10.2） | `knowledge_patch_routes_to_review` | **仅**在声明该能力时判 |
| 审计纯函数（O03） | `audit_surface` | 十九条稳定标记，逐条可构造，并以「恰为这十九条」断言 |

## 测试

`cargo test -p nai-kernel --test monitoring`：**34 passed / 0 failed**。

| 组 | 覆盖 |
| --- | --- |
| 投影与词汇（4） | `Exposure::ALL` 恰为 `ALLOWED ∪ FORBIDDEN` 且互不相交；`MobileView::field_names()` 与 `Exposure::ALLOWED` 标签集合相等；`LiveState::sensitive_field_names()` 与投影字段集合不相交；带密钥/媒体时钟/原始音视频/开播句柄的状态投影后**禁止类别与密钥值均不出现** |
| 最小形态（4） | 最小形态恰为四项且 ⊆ 九项；缺紧急静音被指名；**能力缺失时不重复报「不可达」**；预览/推送后置 → 最小形态完整且审计为空 |
| 暴露类别（3） | 五类禁止暴露逐条命中对应标记；同名重复声明只报一次；白名单暴露不产生 finding |
| 运营项（4） | 紧急静音不可达；未提供启用状态；知识速补绕审；**未声明知识速补时不报该项** |
| 预览（2） | 启用时四条件逐条命中；**未启用时四字段一律不产生缺陷**（含 `Default` 为"已后置"） |
| 路线（4） | 路线 A 缺域名或缺证书均命中；路线 A 齐备 ⇒ 审计空且 `Ready`；路线 B 缺分发与成熟度各命中；路线 B 仅成熟度未验证时**不**误报缺分发 |
| 加密（4） | 中继终止 TLS 且宣称 E2E 命中；**如实披露（不宣称 E2E）审计为空**；E2E 而协议未审核命中且不误报另一条；经审核的 E2E 审计为空 |
| 命令（5） | 角色序 `Operator ≥ Observer`；五项契约字段各缺一次逐条命中并指名；高危开放给观察者命中；高危要求操作员审计为空；低危可开放给观察者 |
| readiness（2） | 未定案且干净 ⇒ `AwaitingRouteDecision`；未定案但暴露媒体时钟 ⇒ `Defective`（优先） |
| 审计完备（2） | 三份构造描述符的审计并集**恰为十九条**；干净描述符审计为空 |

## 实跑

```
$ cargo xtask check --profile portable
... 63 test binaries + 17 doc-test groups, 592 passed / 0 failed
portable: engineering foundation checks passed; device/SDK/GPU/install certification not performed
EXIT=0

$ cargo xtask check-deps
dependency boundaries passed: 18 workspace packages (normal/build/dev/target declarations)

$ cargo xtask check --registry
admission registry check passed (V3.6 acceptance + D01-D14 mapping)
```

门控目标数由 W32 的 **79** 升至 **80**（63 Running + 17 Doc-tests）：新增一个集成测试目标 `tests/monitoring.rs`；通过项由 558 升至 **592**。
本轮**先跑预链**（`cargo test --workspace --no-run` + 按报错路径定向清理被锁 exe）**3 轮收敛**，随后门控一次通过、`LNK=0`——
宿主对新生成 exe 的瞬时锁，与代码无关。

## 依赖边界

- 落 `nai-kernel`（只依赖 contracts），复用 `golden::Finding`（同一报告词汇）。
- `check-deps` 仍 **18 包**、方向不变；**外部依赖零新增**（仅 `std`）。
- 不新增 crate；不改 workspace 组成。

## 未覆盖边界

- **不含任何真实交付**：移动端应用、证书、分发渠道、中继、真机与 S09 真机集成都由外部提供。测试中的描述符是**构造的**，只证明越界口径与
  readiness 派生可执行。
- **不替 M0b 做路线决策**：路线 A/B 二选一是外部 PoC 的结论，本模块只校验**已声明**路线的前置条件。
- **不引入传输层**：命令契约的字段齐备性可判；真实防重放、去重、ACK 与吊销即时生效属 S09 的集成验收。
- **不改 `platform` 模块**：它管官方事件门（事件来源、房间、权限快照、连接代次），本模块管监控面的越界面。
- **不加载体**：待 M0b 定案并立项真机交付后再补 xtask 命令，**无需改本模块判定**。
- 注册表 **O03 保持 `blocked`**：判定机制（W33）在 notes 中保留并**补齐了原先缺失的 blocked 原因**；**S09 保持 `tested`** 并在 notes 回指
  本变更引用了其命令契约。

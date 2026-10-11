# W53 显式 Unavailable 平台连接器（A03 落点）

- 日期：2026-10-11；环境：Windows 11 工作站（Rust 1.96.0 / MSVC，Node 22.22.2），本机门控。
- 基线：V3.7（ADR 0042）。对应验收：**A03「新增连接器不改内核」**（`release_line: r1`，`automation: integration`）。
- 结论：接缝与实物均已交付、实测通过；**A03 保持 `implemented`、P01 保持 `blocked`——真实官方接口仍是外部真值。**

## 1 本轮做了两件事

### 1.1 补上 A03 的**主语**

A03 的命题「新增连接器不改内核」在本轮之前**没有主语**：

| 命题 | 本轮之前 |
| --- | --- |
| 「无官方 SDK 时不用 fake connector 冒充」 | `crates/nai-kernel/src/bridge.rs:537` **已实现**（`fake_connector_claimed_as_real_capability`） |
| 「内核不依赖平台连接器」 | `xtask/src/deps.rs:71` **已实现**，且其单测夹具就叫 `plugins/platform-manual` |
| 「存在一个平台连接器」 | **不存在**——`plugins/` 下只有 `avatar-2d`，`apps/` 下没有 `platform-bridge` |

⇒ **缺口不是「缺判定」，而是「缺被判定的实物」。** 本轮交付实物。

### 1.2 把「不放伪造 SDK」从承诺变成**结构事实**

§17.2 要求「尚未取得官方接口的连接器只保存契约、准入说明与显式 Unavailable 实现，**不放伪造 SDK**」。
在此之前这句话在仓库里**无处校验**（既无该包，也无该检查）。本轮它由一条**读该包自身 `Cargo.toml`** 的测试钉住，
且该测试**自证非空洞**。

## 2 交付物

| 位置 | 内容 |
| --- | --- |
| `crates/nai-plugin-sdk/src/connector.rs` | 连接器接缝：`ConnectorIdentity`、`UnavailableReason`（**封闭五值**）、`ConnectorAvailability`、`AdmissionNotes`、`PlatformConnector`、`require_available`、`unverified_external_dependency`；模块内 4 项单测 |
| `plugins/platform-unavailable/{Cargo.toml,src/lib.rs}` | **快手**与**视频号**两个显式 Unavailable 实例（层 `plugin`，只依赖 `nai-contracts` + `nai-plugin-sdk`） |
| `plugins/platform-unavailable/tests/unavailable.rs` | 9 项集成测试（含非空洞守卫与「不伪造 SDK」的结构断言） |
| `xtask/src/deps.rs` | 新增规则：`plugins/platform-*` **不得依赖 `kernel` 层**；+1 项单测 |
| `Cargo.toml` / `Cargo.lock` | workspace 成员 **18 → 19** |
| `docs/platform.md`、`docs/platform-lifecycle.md`、`docs/engineering.md` | 就地订正 |
| `tests/registry.yaml` | A03 notes 指向落点（**status 未改**） |
| `openspec/specs/platform-connector/spec.md` | 新能力（归档时吸收；5 条需求、12 个场景） |

## 3 关键口径（逐条钉死）

1. **可用性是声明的一部分，且是一个有依据的主张。**
   `Verified { interface_revision }` 的空/空白修订号在**构造时即拒**（稳定标记
   `VERIFIED_WITHOUT_INTERFACE_REVISION`）。`Unavailable { reason }` 的 `reason` 取自**封闭五值**
   （`NoOfficialInterface` / `PlatformAdmissionPending` / `NotPortedToTargetOs` / `CredentialAbsent` /
   `DebugBuildOnly`）——开放字符串会让「为什么不可用」退化成无法被判定的自由文本。
   本仓两个实例都是 `NoOfficialInterface`（§9.4 的原话是「在核实官方接口前」）。

2. **不可用时「返回 Ok 但什么都没做」在接缝里没有表示。**
   §27.1 的四项职责（SDK 初始化 / 事件与权限转换 / 认证 IPC / 退出）是**提供的公开方法**：
   每个先过 `require_available()`，再交给 `*_impl` 钩子。因此不可用连接器**不可能**从任何一项返回成功。
   失败统一为 `CAPABILITY_NOT_READY` + `details["reason"] = "UNVERIFIED_EXTERNAL_DEPENDENCY"`
   + `details["duty"]` + `retryable = true`。
   `UNVERIFIED_EXTERNAL_DEPENDENCY` 取自 **§27.1 第 1076 行的原文术语**，与
   `bridge.rs::SdkAvailability::UnverifiedExternalDependency` 是同一个词——两处不得各起一个名字。

   为什么是 `CAPABILITY_NOT_READY` 而不是 `CAPABILITY_ABSENT`：能力本身在设计里是**存在**的（只是未就绪），
   且与既有用法一致（A10 的 `Unmeasured` 以 `CAPABILITY_NOT_READY` 拒绝、
   `xtask check --golden` 在题集未提供时返回 `CAPABILITY_NOT_READY`）。

3. **失败关闭。** `*_impl` 钩子默认返回具名 `NOT_IMPLEMENTED`。若有人声明了 `Verified` 却忘了实现钩子，
   结果是**具名失败**而非静默成功。

4. **准入说明缺项按名拒绝。** `AdmissionNotes` 覆盖 §13.1 的**五项**合规核对与 §9.3 的**四项**接入节奏；
   `AdmissionNotes::declared` 对缺项返回 `ADMISSION_NOTES_INCOMPLETE` 并**列出所缺项**。
   静默丢一项会让准入说明看起来「已满足」而闸门是开的。

5. **「不放伪造 SDK」的可判定形式。** 该包的依赖只允许 `nai-contracts` 与 `nai-plugin-sdk`
   （白名单断言 + 条目数断言，防止解析「什么都没看到」），并另有一条 **SDK 形态扫描**
   （`douyin` / `kuaishou` / `wechat` / `tencent` / `bytedance` / `-sys` / `bindings` / `ffi` / `native-`）。
   扫描刻意**不含**裸 `sdk`——接缝本身就叫 `nai-plugin-sdk`；`*-sdk` 这一形态由精确白名单兜住。

6. **A03 的两个方向都由门控强制。**
   - 内核 → 连接器：既有分层规则（`kernel` 只能依赖 `contracts`）。
   - 连接器 → 内核：**W53 新增**（`plugins/platform-*` 不得依赖 `kernel`）。
     理由：接缝充分则无需内核类型；若连接器需要内核，则「加连接器」必然先改内核，A03 名存实亡。
     规则**只针对 `plugins/platform-*`**，普通插件不受影响（单测钉住这一点，防止规则被无意扩大）。

7. **非空洞守卫。** 测试内有且仅有一个 `Verified` 替身在**测试内**实现四项钩子并走通成功路径。
   没有它，「不可用连接器的每一项职责都失败」这条断言会被一个**恒失败的接缝**同样满足。
   该替身住在 `tests/`、身份写明 `test-double`、从不冒充任何平台（§17.5/§721：mock 只属于测试与明确的演练）。

## 4 实测

| 检查 | 结果 |
| --- | --- |
| `cargo test -p nai-platform-unavailable` | **9 passed / 0 failed** |
| `cargo test -p nai-plugin-sdk --lib` | **18 passed**（新增 4 项；原 14） |
| `cargo test -p nai-xtask --lib` | **52 passed**（新增 1 项；原 51） |
| `cargo xtask check-deps` | **19 workspace packages**，依赖方向不变，**外部依赖零新增** |
| `cargo xtask check --profile full` | 见 §5 |
| `openspec validate w53-… --strict` | valid |

## 5 门控与闭环

- 门控、归档、提交与 CI 观测结果见本报告追加段（同轮完成，随每轮「一次 commit + push」闭环）。
- **计数口径变化（下一轮请注意）**：新增一个 workspace 成员 ⇒ 测试目标数与通过数都会变
  （新增 `Running unittests src\lib.rs (nai_platform_unavailable…)`、`Running tests\unavailable.rs` 与
  `Doc-tests nai_platform_unavailable` 三条目标行）。**不要把它读成「误改依赖」**——
  `check-deps` 从 18 包变 19 包是本轮**唯一**的图结构变化。

## 6 未做与已知边界

- **`nai-kernel` 一行未改**（改了即自否 A03 的命题）。BUS/注册表/生命周期均未触及。
- **不写任何真实 SDK、不发网络请求、不启动伴侣、不推流、不建 `apps/platform-bridge`**。
  §17.2 的 `apps/platform-bridge` 是**原生 C++ 桥接进程**（加载官方 SDK，属抖音**正式接入**，即 R2/P01）；
  §9.4 说的「同一平台插件目录」里的**连接器**才是本轮的对象。二者不是同一件东西。
- **不升级任何 `status`**：A03 保持 `implemented`（本轮把它的命题变成**可判定**，不是「真实平台已接入」）；
  P01 保持 `blocked`。
- **真实真值仍缺**：官方接口文档、调试版伴侣 PoC、平台用途/产物/权限与费用的准入决定（§9.3）——
  两者一律如实记 `Unavailable { NoOfficialInterface }`，**不宣称可上线**。
- **风险 1**：第二条依赖规则目前**只有这一个包触发**。它是为「所有未来的平台连接器」写的，
  但现在只有一个样本；若将来出现合法的「连接器需要内核」需求，应走 ADR 而不是放宽这条规则。
- **风险 2**：`Verified` 路径目前**只有测试替身**走过。真实 `Verified` 连接器的形状
  （接口修订号的来源与校验方式）要等抖音正式接入时才能定，本轮**不预设**。
- **风险 3**：`tools/model-export` 与 Python 侧无关，未触及；本轮也不涉及任何新证据目录
  （§30.1 未为平台连接器指定载荷目录；撮造目录属过度绑定）。

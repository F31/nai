# 设计（w53 显式 Unavailable 平台连接器）

## 1 为什么是「补实物」而不是「补判定」

本轮最容易做错的方向是再写一个判定模块——仓库里已有的判定模块（`realistic` / `monitoring` /
`device` / `demo` / `bridge`）会诱使人照搬一份「连接器审计」。**但缺的不是判定**：

| 命题 | 现状 |
| --- | --- |
| 「无官方 SDK 时不用 fake connector 冒充」 | `nai-kernel/src/bridge.rs:537` **已实现**（`fake_connector_claimed_as_real_capability`） |
| 「内核不依赖平台连接器」 | `xtask/src/deps.rs:71` **已实现**，且其单测夹具就是 `plugins/platform-manual` |
| 「存在一个平台连接器」 | **不存在**（`plugins/` 只有 `avatar-2d`） |

所以 A03「新增连接器不改内核」今天**无法被证伪也无法被证实**：它没有主语。
本轮的产出必须是**那个主语**。这也决定了本轮的验收方式——不是「又 N 条稳定标记」，
而是**一份可真跑的、显式不可用的连接器，加上把它的性质钉成结构事实的检查**。

## 2 接缝为什么放在 `nai-plugin-sdk`

连接器要用到的类型必须能被**插件层**引用，而内核**不能**依赖插件（§17.5）。
分层门控（`xtask/src/deps.rs:71`）规定：

- `contracts` → 不得依赖任何东西；
- `kernel` / `sdk` → 只能依赖 `contracts`；
- `plugin` → 不得依赖 `plugin` / `runtime` / `app`（依赖 `contracts`、`sdk`、`media`、`shared` 均合法）。

因此接缝放 `crates/nai-plugin-sdk`（层 `sdk`）：**内核够不着它，插件够得着它**，
且它自己只依赖 `contracts`——不引入任何新依赖。

**备选与否决**：

- *放内核*（与既有判定模块同处）⇒ **否决**：内核够得着的接缝，会让「连接器必须依赖内核才能实现」
  变得合理，正是 A03 要排除的形状。
- *放新 crate*（如 `nai-connector-sdk`）⇒ **否决**：为一个 trait 与四个数据类型的接缝再加一个包，
  在 18 包的依赖图里多一条边而无对应收益；`plugin-sdk` 的定位（`lib.rs` 首行：
  「Manifest conversion and plugin surface」）本就包含「插件面」。

## 3 为什么是 `plugins/platform-unavailable` 而不是 `apps/platform-bridge`

设计里这**两个是不同的东西**：

- `apps/platform-bridge`（§17.2 第 683 行的「其余应用」）是**原生 C++ 桥接进程**——
  加载官方 SDK、转换官方事件、与引擎做认证 IPC。它是**抖音正式接入**的载体，属 R2/P01，
  且需要引入 C++ 构建链（本机 Windows 上 `target/contracts/cpp` 的 CMake 产物来自 Linux 容器）。
- §9.4 说的「快手、视频号放入**同一平台插件目录**」与 §17.2 说的「尚未取得官方接口的**连接器**」——
  是**连接器**，位于插件层。

本轮交付的是**后者**：一个可编译、可测试的插件层成员，把「显式 Unavailable」从一句话变成一件实物。
命名取设计自己的术语（§9.4/§17.2 一律用 **Unavailable**），故为 `plugins/platform-unavailable`
（包名 `nai-platform-unavailable`）。

## 4 可用性是一个有依据的主张，不是一个形容词

```rust
pub enum ConnectorAvailability {
    Verified { interface_revision: String },
    Unavailable { reason: UnavailableReason },
}
```

- `Verified` **必须**带 `interface_revision`（所依据的官方接口修订号）。缺失即拒绝——
  这与仓库既有口径一致（B02 的 G0「许可引用存在不等于证据通过」、
  `established_right_without_basis`；台账里「有名字没依据」一律不算成立）。
- `UnavailableReason` 是**封闭五值**：`NoOfficialInterface` / `PendingApproval` /
  `NotPortedToTargetOs` / `CredentialAbsent` / `DebugBuildOnly`。
  封闭是刻意的：开放字符串会让「为什么不可用」退化成自由文本，无法被测试与门控区分对待。
  本轮两个实例都取 `NoOfficialInterface`（§9.4 的原话是「在核实官方接口前」）。

## 5 不可用时的操作必须**按名失败**

`trait PlatformConnector` 把 §27.1「桥接只做」的四项职责做成交接点，四项一律返回
`Result<_, ContractError>`：

| 职责 | 方法 |
| --- | --- |
| SDK 初始化 | `initialize` |
| 官方事件与权限转换 | `translate_event` |
| 与引擎的认证 IPC | `deliver_to_engine` |
| 退出 | `shutdown` |

- 返回的**成功类型**是各自的具体结果（`Initialized` / `TranslatedEvent` / `Delivery` / `Shutdown`），
  不是 `Option` 或「空结果」——**没有**「成功了但什么都没发生」这种表示。这是刻意的：
  §1344 禁的是「用 mock 通过真实验收」，而**最容易发生的退化**恰恰是「返回 Ok 空对象」，
  它在调用方看来与真接入无法区分。
- `Unavailable` 时，接缝提供的 `require_available()` **是通向成功类型的唯一入口**
  （四个方法都先过它），它返回 `ContractError { code: CAPABILITY_NOT_READY, category: "platform",
  details: {"reason": "UNVERIFIED_EXTERNAL_DEPENDENCY"} }`。
  `UNVERIFIED_EXTERNAL_DEPENDENCY` 取自 §27.1 第 1076 行的**原文术语**，
  与 `kernel::bridge::SdkAvailability::UnverifiedExternalDependency` 是同一个词——
  两处不得各起一个名字。
- 选 `CAPABILITY_NOT_READY` 而非 `CAPABILITY_ABSENT`：能力本身在设计里是**存在**的（只是未就绪），
  且这与仓库既有用法一致（A10 的 `Unmeasured` 以 `CAPABILITY_NOT_READY` 拒绝、
  `xtask check --golden` 在题集未提供时返回 `CAPABILITY_NOT_READY`）。`retryable = true`。

## 6 「不放伪造 SDK」做成结构事实

承诺式的做法是在文档里写「本项目不内置平台 SDK」——不可查。本轮的做法的分两层：

1. **包清单只依赖两样**：`nai-contracts`、`nai-plugin-sdk`。两者都是 workspace 内成员，没有外部依赖。
2. **一条读自身清单的测试**（`include_str!("../Cargo.toml")`）：
   解析 `[dependencies]` 段的依赖名，断言 (a) 全部落在允许集合内；
   (b) **没有任何依赖名命中 SDK 形态**（大小写无关地含 `sdk` / `douyin` / `kuaishou` /
   `wechat` / `tencent` / `-sys` / `bindings` / `native-`）。

这条测试**有真实失败模式**：往 `Cargo.toml` 加一行 `douyin-sdk = "1"` 会让它红。
它不是「跑一遍总是绿」的装饰。注意这里**不解析 TOML 库**（会新增依赖），
只做行级解析并对行数与依赖数做断言——**解析范围外的东西不装作已检查**。

## 7 A03：两条方向都由门控强制

A03 的命题「新增连接器不改内核」有两个方向，缺一不可：

- **内核 → 连接器**：`deps.rs` 既有规则已禁（`kernel` 只许依赖 `contracts`），
  其单测夹具 `plugins/platform-manual` 就是为此写的。
- **连接器 → 内核**：**此前没有规则**。如果连接器需要依赖内核才能实现，
  那「新增连接器」就必须先动内核——A03 即便在结构上「没违反」也已名存实亡。
  本轮新增：**路径以 `plugins/platform-` 开头的包，不得依赖 `kernel` 层**。

第二条是**可复用**的（快手/视频号/未来的其他平台连接器一并适用），且不是为本轮一个包量身的。
**代价如实登记**：它为「当下的一个包」新增了一条目前只有它触发的规则；
不做的代价是 A03 只有单向强制。

## 8 非空洞守卫

「不可用连接器的每一项操作都失败」这条断言**天然接近恒真**——如果接缝写错成恒失败，
测试照样全绿。故测试内构造一个 `Verified { interface_revision: "test-only" }` 的替身，
断言同一组四项操作**存在成功路径**。该替身**只在测试内**（§17.5/§721：「mock 只属于测试与明确的演练」），
不进入 `src/`，也不冒充任何平台能力——它的身份是 `test-double`，准入说明明确写「不是平台」。

## 9 未做与风险

- **不做**：`nai-kernel` 一行不改（改了即自否 A03）；不写真实 SDK；不发网络请求；不建 C++ 桥；
  不升级 A03/P01 的 `status`。
- **风险 1（登记）**：workspace 成员 18 → 19，`check-deps` 包数随之变化。这是**唯一**的图结构变化，
  报告与 AGENTS 里都写明，避免下次把它当成「误改依赖」。
- **风险 2（登记）**：本轮证明的是「**接缝足够**——只用契约与插件面就能写一个连接器」，
  **不是**「真实平台已接入」。真实官方接口、调试版伴侣 PoC、上线授权仍是外部真值。
- **风险 3（登记）**：`Verified` 路径目前只有测试替身走过一次。真实 `Verified` 连接器的形状
  （接口修订号的来源与校验）要等抖音正式接入时才能定，本轮**不预设**。

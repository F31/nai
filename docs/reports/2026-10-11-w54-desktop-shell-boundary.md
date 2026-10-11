# W54 实测报告：桌面壳形态与 Tauri 边界

- 日期：2026-10-11
- 变更：`openspec/changes/w54-desktop-shell-boundary/`
- 裁决：[ADR 0046](../adr/0046-desktop-shell-form-and-tauri-boundary.md)
- 指南：[docs/control.md](../control.md)、[docs/ui.md](../ui.md)
- 判据：`tests/registry.yaml` **A12「桌面壳宿主边界」**（新增，保持 `pending`）

## 结论

基线把宿主壳写了四处明文要求（§1248 技术栈、§1260 产物、§1151 最小权限、§1259 sidecar 与生命周期、§900 数据根、
§1101 受保护 IPC），而仓库里**一处落点都没有**，并且**没有任何 ADR 记录过这个决策**。
本轮把「形态与边界」变成**可查的裁决**与**可判定的声明**，并在过程中确认一件容易被重复做的事：
**INV-18「内核不依赖 Tauri」已经由门控强制**，本轮**不重复实现**。

**登记状态**：新增 A12，`status: pending`。本轮交付的是**形态裁决与边界**，不是壳体——
§1248/§1260 的真值是**可运行、可打包、可签名**的壳，本轮没有。

## 一、缺口性质：形状已定，缺的是「裁决」与「可查的落点」

| 事实 | 出处 |
| --- | --- |
| 基线明文规定技术栈 **Tauri 2 + React + TypeScript + Vite**；产物含 `desktop-shell` | V3.7 §1248 / §1260 |
| `docs/adr/` 的 **44 篇**里**没有一篇**裁决桌面壳形态 | `ls docs/adr/`（0001–0045） |
| `apps/desktop` 是**纯 Web**：依赖只有 `react`/`react-dom`/`vite`，无 `src-tauri/` | `package.json`、`find` |
| §1151／§1259／§900／§1101 四组条款在 `crates/`、`apps/`、`schemas/` **零命中** | 全仓检索 |
| `tests/registry.yaml` 的 **66 条里没有桌面壳条目**（A04 只管依赖方向） | `tests/registry.yaml` |
| **INV-18「内核不依赖 Tauri」已由门控强制**：`tauri` 前缀检查 + 单测夹具 | `xtask/src/deps.rs:108`、`:490` |

最后一行决定了本轮**不做**什么。直觉会想「既然是 Tauri，先加一条『内核不得依赖 Tauri』的检查」——
但它**已经在**（`dependency.starts_with("tauri")`，且单测用 `&["windows-sys", "tauri"]` 钉住）。
重复实现它只会制造第二处定义。故 ADR 0046 只**引用**它。

⇒ 真正的缺口是**边界**：壳做什么、不做什么、授了什么、走哪条通道、后台归谁管。
在边界缺席时，「壳做完了没」无法判真伪——任何一份配置文件都能自称壳。

## 二、决策（D1–D8，见 ADR 0046）

| # | 决策 | 要点 |
| --- | --- | --- |
| D1 | 形态 = Tauri 2；壳是**呈现层与宿主接缝** | 需要在 UI 关闭后继续运行的进程归 supervisor / 托盘（§1259） |
| D2 | 壳**只做四件事** | 窗口与托盘生命周期／受保护 IPC 宿主侧／数据根解析／sidecar 拉起与监督；业务语义 MUST NOT 进壳 |
| D3 | **不引入真实 Tauri 依赖** | WebView2 导入库与网络会经构建链把**主机事实**混进门控结论（ADR 0041 已裁过的禁忌） |
| D4 | 最小权限是**集合相等**，禁聚合权限集 | §1151 的「只开放必要」是上界断言；「包含」只证下界 |
| D5 | 受保护 IPC = 生产路径；loopback HTTP = 测试适配 | §1101；既有 `ControlClient` **不变**，只在声明里定性 |
| D6 | 生命周期与 sidecar 命名同源 | 都来自「壳是可关闭的呈现层，不是后台的监护者」 |
| D7 | **一次裁决三处** | 桌面／macOS／移动端路线 B 共用边界；差异只允许落在目标与分发 |
| D8 | 落点：ADR + 真实格式声明 + 校验 + A12 | 校验经既有 `test:ui` 自动进入 `full` 门控 |

## 三、改动

| 文件 | 改动 |
| --- | --- |
| `docs/adr/0046-desktop-shell-form-and-tauri-boundary.md`（新） | 形态与边界裁决：D1–D8、否决方案、已知后果 R1–R6 |
| `apps/desktop/src-tauri/tauri.conf.json`（新） | Tauri 2 真实格式：窗口 `label: main`、`frontendDist`、逐指令显式 CSP、`bundle.externalBin`（**不带三元组**的逻辑名） |
| `apps/desktop/src-tauri/capabilities/default.json`（新） | 最小权限面：**逐条**列举 5 个 `core:*:allow-*`；无聚合权限集、无 shell/fs/sql/process |
| `apps/desktop/src-tauri/shell-policy.json`（新） | 本仓库自有的**策略声明**（非 Tauri 字段）：把 Tauri schema 表达不了的边界变成可判定数据 |
| `apps/desktop/src-tauri/README.md`（新） | 说明「只有声明、没有 `Cargo.toml`/`main.rs`」是裁决结果，不是遗漏 |
| `tests/ui/desktop-shell.test.mjs`（新） | **15 项**校验，含**非空洞守卫** |
| `tests/registry.yaml` | 新增 **A12**（`status: pending`，notes 记落点与缺失前置） |
| `docs/control.md` | 就地订正首页句（形态已裁决／壳体未构建）＋ 新增「桌面壳宿主边界（W54）」节 |
| `docs/ui.md` | 新增「桌面壳宿主边界（W54）」节 |
| `apps/desktop/src/main.tsx` | 页脚从「Tauri/受保护 IPC 尚未接线」改为准确表述（形态已裁决、**壳体尚未构建**；基线 V3.6 → V3.7） |
| `AGENTS.md`、`docs/development-plan.md` | 同步 W54 段 |
| `.gitignore` | 预置 `src-tauri/target/` 与 `src-tauri/gen/`（引入真实依赖后才出现） |
| `openspec/specs/desktop-shell/` | 新能力，8 条需求 / 14 个场景 |

**不动**：`crates/` 全部、`apps/*/Cargo.toml`（本轮无 Rust 改动）、workspace 成员（仍 **19 包**）、
任何既有验收的 `status`、任何 Tauri 依赖。

## 四、校验的内容与「非空洞」

`desktop-shell.test.mjs` 把每条检查写成**纯函数**（输入数据 → 输出错误列表），因此可以拿**注入样本**证明它不是恒真的：

| 断言 | 注入样本（必须被抓到） |
| --- | --- |
| 权限集合与声明逐一相等 | `shell:allow-execute`（超界）、`core:default`（聚合）、删一项（缺声明） |
| 权限形状 | `fs:allow-read-text-file`（非 `core:` 前缀） |
| CSP | `'unsafe-eval'`、`connect-src *`、`connect-src http:`、缺 `ipc:` |
| sidecar | 未登记的 `binaries/nai-nonexistent`、带三元组的逻辑名、空清单 |
| 壳策略 | `backendsOutliveUi: false`、壳承担 `session-lifecycle`、形态集合漂移、`loopbackHttpRole: "production"` |
| 绝对路径 | `C:\Users\...`、`/home/...`（而回环 URL **不**误报） |
| Tauri 导入 | `@tauri-apps/plugin-fs`、`@tauri-apps/api/core`（相对导入不误报） |

**真值来源不是测试里编的常量**：sidecar 白名单从 `apps/*/Cargo.toml`（或 `src/main.rs`）**读出真实二进制包名**；
CSP 与 capabilities 读的是仓库里真实的声明文件。

## 五、已知后果与风险（如实登记，非缺陷）

| # | 后果 | 处置 |
| --- | --- | --- |
| R1 | 校验读的是配置**形状**，**不能**证明它被某个 Tauri 版本接受 | 不声称「配置已验证可构建」 |
| R2 | Tauri 大版本改动配置字段时，本仓库检查**不会自动发现** | 引入真实依赖时应改由**构建产出物**校验 |
| R3 | `tauri.conf.json` 的 `$schema` 指向官方 URL，**离线不可校验** | 校验**自建**，不依赖网络；schema 拉取不进任何门控 |
| R4 | 权限集被钉死为 5 条，加功能要同改两处 | 这是 §1151「只开放必要」的落地方式 |
| R5 | A12 为 `pending`，易被读成「没做事」 | notes 区分**本轮落点**与**缺失前置** |
| R6 | 前端页脚与若干文档此前称「Tauri 尚未接线」 | 就地订正为「形态与边界已裁决，**壳体尚未构建**」——两者是不同的陈述 |

另有一处**接线缺口**需明说：壳体一旦引入，同机控制通道 MUST 迁到受保护 IPC；
在那之前 CSP **不**开放 loopback HTTP 源（不是因为限制，而是不让「HTTP 是正式通道」在配置里被默认下来）。
**本轮不改** `ControlClient`——它现在是测试适配，且它确实是 §1101 允许的那个角色。

## 六、未做边界

- **壳体未构建**：无 `tauri` crate、无 Rust 壳主体、未打包、未签名 ⇒ **A12 保持 `pending`**。
- **未测真实 WebView2 构建**：安装器与 Windows 代码签名链属发布里程碑。
- **macOS 与移动端路线 B**：本轮只写**共用边界**，各自实现分别属第 4 项与第 5 项。
- **未升级任何既有 `status`**；**内核未动一行**；**workspace 仍 19 包**。

## 七、门控实测

- `cargo xtask check --profile full` 覆盖工程门控与前端门控；本轮**零 Rust 改动**，故 Rust 侧目标数与通过数
  与 W53 持平（**86 目标：68 `Running` + 18 `Doc-tests`；740 passed / 0 failed**）。
- 前端侧新增 `tests/ui/desktop-shell.test.mjs`：`tests/ui` 由 **37 → 52**（+15）。
  该文件经根 `package.json` 的 `test:ui` 自动进入 `full` 门控——**未改门控代码**
  （`scripts/frontend-gate.mjs` 按 `scripts` 驱动，ADR 0041 D3）。
- `cargo xtask check-deps` 仍为 **19 workspace packages**；`openspec validate --all --strict --archived` 见归档结果。


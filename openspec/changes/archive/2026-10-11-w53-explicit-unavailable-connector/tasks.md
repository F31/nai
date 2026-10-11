# 任务（w53 显式 Unavailable 平台连接器）

## 0 立项

- [x] 0.1 建立 `openspec/changes/w53-explicit-unavailable-connector/`（`.openspec.yaml` / `proposal.md` /
      `design.md` / `specs/platform-connector/spec.md` / `tasks.md`）
- [x] 0.2 `openspec validate w53-explicit-unavailable-connector --strict` 通过

## 1 接缝（`crates/nai-plugin-sdk/src/connector.rs`）

- [x] 1.1 `ConnectorIdentity`（连接器 id + 平台 id + 平台标签，三者非空且非空白）
- [x] 1.2 `UnavailableReason` 封闭五值 + `label()`；`ConnectorAvailability { Verified { interface_revision } |
      Unavailable { reason } }`，`Verified` 的空/空白修订号在构造时即拒
- [x] 1.3 `AdmissionNotes`：§13.1 五项合规核对 + §9.3 四项接入节奏，提供 `missing()` 具名查询
- [x] 1.4 `trait PlatformConnector`：`identity` / `availability` / `admission_notes` +
      §27.1 四项职责（`initialize` / `translate_event` / `deliver_to_engine` / `shutdown`）
- [x] 1.5 接缝纪律：`require_available()` 与 `unverified_external_dependency()`
      ——`Unavailable` 时返回 `CAPABILITY_NOT_READY` + `details["reason"] = "UNVERIFIED_EXTERNAL_DEPENDENCY"`
      + `retryable = true`；四项职责是**提供的公开方法**，先过闸再交 `*_impl` 钩子，
      钩子默认返回具名 `NOT_IMPLEMENTED`（失败关闭）⇒ **「Ok 空结果」没有表示**
- [x] 1.6 `lib.rs` 导出 `pub mod connector;`（并更新 crate 头注释）

## 2 连接器（`plugins/platform-unavailable/`）

- [x] 2.1 `Cargo.toml`：`[package.metadata.nai] layer = "plugin"`；依赖仅 `nai-contracts` + `nai-plugin-sdk`；
      加入根 `Cargo.toml` 的 workspace 成员（18 → 19）
- [x] 2.2 `src/lib.rs`：两个显式不可用实例——**快手**、**视频号**，均为
      `Unavailable { NoOfficialInterface }`；四项职责一律不实现（闸门先于钩子）
- [x] 2.3 准入说明按 §13.1（实名与类目、虚拟人标识、权限须 granted、话术多样、记录可导出）与
      §9.3（官方接口核实、调试版伴侣 PoC、平台准入确认、产物配置申请前定案）成 in-tree 数据；
      模块头写明「不宣称可上线」

## 3 测试（`plugins/platform-unavailable/tests/unavailable.rs`）

- [x] 3.1 两个平台的 id 互异且稳定；可用性恒为 `Unavailable { NoOfficialInterface }`
- [x] 3.2 四项职责逐一调用：全部返回 `CAPABILITY_NOT_READY` 且 `details["reason"]` 为
      `UNVERIFIED_EXTERNAL_DEPENDENCY`，**没有任何一项返回 Ok**
- [x] 3.3 准入说明覆盖 §13.1 全部五项 + §9.3 全部四项；缺任一项即失败（以夹具断言该检查有效）
- [x] 3.4 **非空洞守卫**：测试内 `Verified` 替身走通四项职责的成功路径
- [x] 3.5 **不伪造 SDK（结构断言）**：`include_str!("../Cargo.toml")` 解析依赖名，
      断言落在允许集合内且不命中 SDK 形态；并以「注入 `douyin-sdk` / `platform-bindings`」验证非空洞
- [x] 3.6 **A03**：连接器只经 sdk 接缝被消费（对象安全），依赖闭包不含 `nai-kernel`

## 4 依赖方向门控（`xtask/src/deps.rs`）

- [x] 4.1 新增规则：路径以 `plugins/platform-` 开头的包 MUST NOT 依赖 `kernel` 层，附理由注释
- [x] 4.2 单测：连接器 → 内核报错；普通插件 → 内核**不**报错（规则不被无意扩大）；
      合法连接器（只依赖 contracts/sdk）不报错

## 5 文档与登记

- [x] 5.1 `docs/platform.md` 增「显式 Unavailable 平台连接器（W53）」小节（就地订正）
- [x] 5.2 `docs/platform-lifecycle.md` 增「判定面与连接器实物的分工（W53）」（就地订正）
- [x] 5.3 `tests/registry.yaml` A03 notes 指向本落点（**status 未改**，保持 `implemented`）；P01 保持 `blocked`
- [x] 5.4 `AGENTS.md`、`docs/development-plan.md`、`docs/engineering.md` 同步
- [x] 5.5 `docs/reports/2026-10-11-w53-explicit-unavailable-connector.md`（含 18→19 包与未做边界/风险）

## 6 闭环

- [x] 6.1 预热两个测试目标后跑 `cargo xtask check --profile full`：**EXIT=0**、
      **86 目标（68 `Running` + 18 `Doc-tests`）、740 passed / 0 failed**、前端全部步骤通过
      （W52 为 83/726；+14 = plugin-sdk 4 + 连接器 9 + xtask 1）
- [x] 6.2 `cargo xtask check-deps` = **19 workspace packages**；`openspec validate --all --strict` 记录条数
- [x] 6.3 全部 tasks 勾选后 `openspec archive -y w53-explicit-unavailable-connector`
- [x] 6.4 `openspec validate --all --strict --archived` 记录条数
- [x] 6.5 一次 commit + push（提交信息用 Write 落 UTF-8 文件，避免 `\u` 字面转义）
- [x] 6.6 用**缓存破坏参数**匿名复核 CI：断言 `head_sha` 与推送一致、记录 conclusion 与失败步骤

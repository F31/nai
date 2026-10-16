# Proposal: W130 — S07 诊断面扫描（日志 / 诊断包 / 崩溃转储）

## Why
registry `S07`（`密钥：日志、诊断包、崩溃转储、仓库扫描均无明文密钥`）的判定口径含**四面**；
仓库侧面已由 W36 建成，其余三面（日志、诊断包、崩溃转储）在仓库里**一直没有落点**——
`tests/registry.yaml` 的 `S07` notes 末句逐字写着「日志/诊断包/崩溃转储的扫描仍未建，保持
implemented 不升级」。于是该条口径的四面里，只有一面真的被判据守着。

本轮把**诊断面**接上。仓库里唯一真实存在的诊断产物是 **CI 门控日志**：它由 CI 的前序步 `tee`
写出，并由 `scripts/ci-diagnose.mjs` 把其中的行发布成**匿名可读的 check-run annotation**。
它的泄漏面是真实的，而它的**失败那一次**恰恰信息量最大——libtest 会把失败测试的 stdout
一起转储进日志。因此「扫日志」这件判定 MUST 在那一次也活着。

## What Changes
1. `xtask/src/secrets.rs` 新增**产物域**（仓库域行为不变）：
   - 登记表 `ARTIFACTS`：`{dir, producer, source, marker}`，`source` 由 `include_str!` 编译期抓取
     生产者自己的文本 ⇒「登记了」与「核验了」是同一份字节。
   - 闭世界台账 `ArtifactLedger`：四个桶（文本 / 非文本 / 非文件 / 容器）**各自独立累加**，
     于是「条目 == 四桶之和」是一条**可失败**的判据。
   - `scan_tree(dir)`：缺目录是**值**（`present = false`）不是错误；非 UTF-8 **有损解码后照样扫**
     （ASCII 段——真实密钥所在处——完整保留）；按目录项自身的类型分类，**不跟随符号链接**。
   - `Hit` / `ArtifactHit` 都**不含匹配文本**：回显它就是把密钥从一份私有文件搬进构建日志。
2. CI（`.github/workflows/foundation.yml`）：门控步的 `tee` 目标改到仓库内约定目录
   `.nai-diagnostics/`（前置 `mkdir -p`）；新增 **`if: always()`** 的 `cargo xtask check --secrets`
   步——它扫的是前序步写的日志，**必须覆盖失败路径**。
3. 门控登记：W95 成因闭集 +`ci-produced-artifact` 并配两条凭据（指向**消费者**
   `scripts/ci-diagnose.mjs` 与扫描器自己的 `producer:` 字段，刻意不指 workflow 自己的 `tee` 行，
   以免两个在册条目互为证据）；W100 条件形状表 +`always`，并新增 **R6**（产物扫描的 `if:` MUST
   是 `always()`）。

## Modified Capabilities
- `live-provider-evidence`：`cargo xtask check --secrets` 的扫描域从「git 会发布的文件」扩展到
  **诊断产物**——产物必须住在扫描器看得见的目录里，且空集要报 `0`、缺目录要说「没有目录」，
  MUST NOT 用「跳过」这种把「没检查」说成「检查过了」的措辞。
  ⇒ 差量为 **MODIFIED**（既有的「凭据只经进程环境变量」一条，正文保持 W36 原样）
  **＋ ADDED**（`The registered diagnostic artifact domain is scanned by the same command`，
  4 条 Scenario）。**拆开而非合并**的理由：一条 Requirement 只声明一条行为，
  且既有一条的正文已 386 字符、再加 410 字符会越过 500 的上限。
- `engineering-foundation`：执行配置判据新增 `always` 条件形状，并新增一条规则——判定**由更早的
  CI 步产生的产物**的步 MUST 带 `always()`，否则门控一红、扫描就消失。
  ⇒ 差量为 **MODIFIED**（既有的「执行配置由解析出的平台集合兑付」一条，去掉 `always` 正文）
  **＋ ADDED**（`A step that judges an artifact produced by an earlier step declares always()`）。
  `always` 规则**只此一处**声明，`live-provider-evidence` 侧不再重复。

## Impact
- registry：`S07` 的收尾判定**由证据决定**（口径含四面，而诊断包与崩溃转储在仓库内**没有产生者**，
  见 design.md），MUST NOT 在本文件预先承诺。
- 生产代码：`xtask/src/secrets.rs` 新增产物域（仓库域一字未改，只加一处具名弱点注释）；
  CI 一处 `tee` 目标改写 + 新增一个 `if: always()` 步；`.gitignore` 增 `/.nai-diagnostics/`。
- 测试：`cargo test -p nai-xtask --lib` **60 passed**（本轮 +9）；node 契约 `step-config.test.mjs`
  新增 F10、合成世界扩为 8 步/4 种 kind/3 种条件形状。
- 具名登记的弱化（MUST NOT 顺手修）：仓库域 `check` 的 `else { continue; }`——非 UTF-8 与读失败
  被静默跳过。本轮**只加具名注释**，不改行为（那是另一件事）。

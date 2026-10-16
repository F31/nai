# ADR 0108：S07 诊断面扫描——产物域与仓库域分开，`always()` 是必须的

- 状态：Accepted｜日期：2026-10-17｜归属：W130（S07 口径收口，**保持 `implemented`**）
- 关联：ADR 0038（W36 仓库密钥面与「git 会发布的内容」口径）、ADR 0078（W100 执行配置面）、
  ADR 0073（W95 步集合闭世界）、ADR 0100（`tested` 的证据可核验）、
  `live-provider-evidence` / `engineering-foundation` spec、`tests/registry.yaml` 的 `S07`

## 背景
`tests/registry.yaml` 的 `S07`（「无明文密钥」）判定口径含**四面**：日志、诊断包、崩溃转储、仓库扫描。
W36 只建了**仓库面**，其 notes 末句逐字写着「日志/诊断包/崩溃转储的扫描仍未建，保持 implemented 不升级」。
于是这条口径的四面里，只有一面真的被判据守着。

开工前的现测（MUST NOT 由「同族」推断）：全仓**无** `tracing`/`log` 依赖；`REDACTED` 的唯一落点是
`Secret` 的手工 `Debug`——**已有**测试覆盖，不是缺口；`diagnostic_refs` 无消费者；仓库内**没有任何代码
写 `.log`**。唯一真实存在的诊断产物是 **CI 门控日志**：`.github/workflows/foundation.yml` 用 `tee`
写出它，`scripts/ci-diagnose.mjs` 把其中的行发布成**匿名可读的 check-run annotation**——泄漏面是真实的，
而失败那一次恰恰信息量最大（libtest 会把失败测试的 stdout 一起转储进日志）。

## 决策
1. **产物域与仓库域分成两条扫描路径，共用一套判据**。仓库域的扫描域由 git 定义
   （`--cached --others --exclude-standard`）；产物域的扫描域由**登记表** `ARTIFACTS` 定义。
   两者的**缺省语义不同**，这个差别是刻意的：
   - 仓库域对非 UTF-8 与读失败 `else { continue; }`——**静默跳过**。这是**具名登记的弱化**，
     本轮**不修**（见决策 5）。
   - 产物域**收下非 UTF-8**：崩溃转储就是二进制，`continue` 会把整条记录静默丢掉，报告上看与干净
     一模一样。有损解码把无效字节变成替换字符、**ASCII 段逐字保留**——高熵凭据恰在 ASCII 段。
   - 产物域**缺目录是值**（`present = false` =「没东西可扫」），不是错误，也 MUST NOT 被读成
     「存在但为空」；空集报 `0`，措辞上 MUST NOT 用「跳过」（那是把「没检查」说成「检查过了」）。
2. **`ARTIFACTS` 是产生者的名册，不是「想扫的目录」的清单**。登记一个**没有产生者**的目录，等于给
   「扫了个空目录」发一张通行证：它让 `present = true`、`scanned = 0`，报告上看与「干净」一模一样。
   因此本轮**只**登记 CI 门控日志；诊断包与崩溃转储在仓库内没有产生者这件事，写进报告、registry notes
   与本 ADR 具名——**而不是**用空目录把它敷衍过去。登记表的凭据由
   `every_registered_artifact_names_the_repository_text_that_produces_it` 双向钉住：`marker` MUST
   逐字存在于 `producer` 的文本里，且 MUST 点名 `dir`；`producer` MUST 是已跟踪文件，且 MUST NOT 是
   判据自己的实现文件（**不是自我满足**）。
3. **闭世界台账，四个桶各自独立累加**。`visited` 数每一个 `read_dir` 条目，四个桶（文本／非文本／
   非文件／容器）各自数自己那一类、**不是派生** ⇒「条目 == 四桶之和」是一条**可失败**的判据：
   新增一条分支却忘了落桶，它会红。命中结构（`Hit` / `ArtifactHit`）**不含匹配文本**——回显它就是
   把密钥从一份私有文件搬进构建日志。
4. **`always()` 是必须的，不是保险**。GitHub 的默认语义是「前序步失败 ⇒ 后续步被跳过」。产物扫描判的是
   **前序步写的产物**，而失败那一次最该扫 ⇒ 没写 `if:` 等于「门控一红，扫描就消失」，一个**在需要它的
   那一刻才消失**的判定。这条落成 W100 的 **R6**；`always()` 也因此成为条件形状表里的独立一格
   （它与「没写条件」在**平台集合**上等价，差别**只在失败语义**）。
   该规则**单独成条 Requirement**（ADDED `A step that judges an artifact produced by an earlier step
   declares always()`，正文 160 字符），而不是塞进既有的执行配置 Requirement——后者只声明「执行配置由
   解析出的平台集合兑付」这一件事（正文 440 字符）。同理，诊断产物域也单独成条 ADDED。**理由**：一条
   Requirement 只声明一条行为；且归档后 `validate --all --strict` 会按 500 字符上限检查**主规格**的
   Requirement 正文，把两条行为叠进一条正文会红（本轮实测：604 / 838 字符各报一次，见报告 §5.1）。
5. **两处具名登记，都不顺手改**：
   - 仓库域 `check` 的 `else { continue; }`（非 UTF-8/读失败静默跳过）——只加具名注释，不改行为。
   - **规则改动与数据改动分开**：`always` 形状与 R6 是**规则**，写进规格；成因闭集新增取值
     `ci-produced-artifact`、兑付表新增一格、`ARTIFACTS` 新增一行都是**数据**。W95 与 W108 的
     Requirement 是**生成的**，新增取值是对它们的实例化 ⇒ MUST NOT 在规格里再写一遍（同一件事两处定义）。
6. **S07 保持 `implemented`，不升 `tested`**。现测：四面里**两面**（仓库、日志）已有真实产生者与判据；
   **两面**（诊断包、崩溃转储）在仓库内**没有产生者** ⇒ 在本仓库**建不了**（登记空目录不是覆盖，
   是发通行证）。按 ADR 0100 §6 的门槛与 `admission-registry` 的 Honest backfill 要求，
   剩下的缺口仍在 ⇒ MUST NOT 升 `tested`。

## 后果
- 正向：S07 的四面从「一面有落点」变成「两面有落点，且日志面有**真实产生者**与**失败路径覆盖**」；
  「登记了却没人生产」这条退化路径被**结构性**堵住（凭据双向核验）；「二进制产物被静默丢弃」
  这条退化路径被闭世界台账与 `non_text` 具名堵住。
- **S07 保持 `implemented`**（不升 `tested`）：诊断包与崩溃转储在仓库内无产生者，缺口具名在
  registry notes、报告与 ADR。
- 生产代码：`xtask/src/secrets.rs` 新增产物域（**仓库域一字未改**，只加一处具名弱点注释）；
  CI 一处 `tee` 目标改写 + 新增一个 `if: always()` 步；`.gitignore` 增 `/.nai-diagnostics/`。
- 测试：`cargo test -p nai-xtask --lib` **52 → 60**（Windows 本机；新增 9 条中 1 条由 `cfg(unix)` 门控
  ——建符号链接在 Windows 上要特权）；node 契约新增 F10，合成世界扩为 8 步 / 4 种 kind / 3 种条件形状。
- **注入验证 11 条**真跑（A1–A6 / B1、B1b、B2–B4），逐条确认「对应判据变红且可归因」，随后**逐字节还原**
  （四文件 `sha256sum` 全部等于基线）；B1 只触发 **R6**（R1/R2 未误报），B1b 同时触发 **R6 + R3**
  ⇒ 两条规则各自独立触发。
- 如实披露：`a_symbolic_link_is_named_and_its_target_is_not_read` 依赖宿主能建符号链接，按本仓惯例用
  `cfg(unix)` 门控，**不让宿主能力决定结论**；这条用例在 Windows 本机不编入。

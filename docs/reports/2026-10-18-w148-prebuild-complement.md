# W148 报告：预构建处方的补集 —— 块外不得有第二份

立项依据：W147 报告 §八第 2、3 条（本轮**不修**别的，只把这两条补齐）。范围与本轮改动见
`openspec/changes/archive/2026-10-18-w148-prebuild-complement/`，裁定见
[ADR 0122](../adr/0122-prebuild-complement-closed-world.md)。

## 一、勘定与现测

### 1.1 §八第 3 条：`testCommandsOf` 有没有「不许被过滤」的守卫

对 `scripts/contracts/gate-coverage.mjs` 的 `testCommandsOf` 注入两种过滤形状，每次跑
`node --test tests/contracts/gate-coverage.test.mjs`，跑完按 sha256 逐字节还原
（基线指纹 `41dcd8ba99cc7199487fccdb4b2c0eb61dd55f59b45aa329ce347d342c8e566b`，三次核对一致）：

| 探针 | 注入 | 结果 |
| --- | --- | --- |
| P1 | `if (values.includes('--exclude')) continue;`（黑名单） | **25 pass / 6 fail**：`#1 #11 #23 #25 #27 #31` |
| P2 | `if (!values.includes('--locked')) continue;`（白名单） | **31 pass / 0 fail** |

P2 是报告原文点名的形态，它**零信号**。成因不是判据缺失，而是**夹具是一致性样本**：那份测试里
每一条 `["test", …]` 夹具（`WORKSPACE_RUN` 与另外十余处）都带 `--locked`，白名单过滤在它们身上
是**恒等变换**。P1 之所以红，是因为它恰好命中真命令里唯一那条 `--exclude`。

⇒ W147 §八第 3 条的措辞必须改写：**判据对「只保留含某旗标」这一形态零信号，成因在夹具**。
落点不是「再加一条判据」，而是**给一个判别性样本**：四种旗标剖面各一条 `["test", …]`，断言
「N 进 N 出且逐 token 等于原文」。

### 1.2 §八第 2 条：块外的域有多大

对**全部已跟踪** `.md`（971 份）按围栏切分后统计：

| 组 | 文件数 | 围栏内处方 |
| --- | --- | --- |
| 活文档（域内：仓库根 + `docs/` 下，减去两段历史） | **50** | **2**（全在 `engineering.md` 标记块里） |
| `docs/adr/` | 121 | **2**（`0121-prebuild-recipe-unit-set.md` L42–43） |
| `docs/reports/` | 153 | **3**（`2026-10-10-*` L111 一条、`2026-10-18-w147-*` L52–53 两条） |
| `openspec/` | 637 | 0 |
| `.opencode/` / `apps/` / `tests/` | 10 | 0 |

围栏配平：324 份 `docs/` 与根目录 md、421 个围栏、**0 个不配对** ⇒ 解析器可以在真实语料上
要求配平，不会一上来就把基线打红。

两条结论：域内今天为绿（可以立判据）；**历史里确有 5 条副本**（所以历史必须排除，且这条排除
必须由断言付代价，见 §三）。

## 二、判据形态

三条决定（理由全文见 ADR 0122）：

1. **只判围栏内的行，不判散文**。围栏里的是可复制执行的，散文里的是用来说明的——
   `engineering.md` L34–36 那条 `cargo build --workspace --all-targets` 反例正是后者。
   判散文就得为它开豁免名单，而豁免名单是判据退化的入口。
2. **处方只认带 `--no-run` 的 `cargo` 命令**。不带的是门控那一步本身，活文档里到处都有。
3. **匹配用多重集，配额先由承载块的文档消费**。块那两行天然命中，第三处出现必然落空 ⇒ 具名；
   先扫承载块的文档，所以被点名的是副本而不是块本身。

实现落点（`scripts/contracts/gate-coverage.mjs`）：

| 名称 | 作用 |
| --- | --- |
| `HISTORY_DOC_SEGMENTS` / `isLiveDocument` | **域的唯一出处**：`.md` + 根目录或 `docs/` 下 − 两段历史 |
| `fencedLines` | 围栏解析（长度/字符/信息串三条件判闭合），不配对即抛 |
| `prebuildPrescription` | 一行围栏内容 → `{text, signature}`，只认 `cargo … --no-run` |
| `strayPrebuildErrors` | 多重集补集，返回 `{errors, documentsScanned, fencedLinesScanned, prescriptionsScanned}` |
| `prebuildRecipeErrors` | 对账入口，**末尾组合**补集（不新增第二处调用点） |

`strayPrebuildErrors` 的四条具名失败：副本（带 `路径:行号` 与整条命令）、域为空、
围栏不配对、块内处方未落在任何围栏内。最后一条是本轮**新引入**的不变量：一条不在围栏里的处方
没有判别力（`fencedLines` 看不见它），把它当「已检查」就是短路。

## 三、域与排除的代价

域只有 `isLiveDocument` 一处定义。测试从 `.git/index` 取路径集合（W75 的 `readTrackedPaths`），
再用它划分；测试**不**另写正则——否则域有两处边界，改一处不会让另一处跟上。

历史排除不是注释里的理由，而是**有断言付代价**的：

- 正向：测试读 `docs/adr/0121-prebuild-recipe-unit-set.md`，断言它的围栏里**确实**有一条处方
  ⇒ 若那边其实没有副本，这条排除就是装饰，断言会先红。
- 反向：把 `HISTORY_DOC_SEGMENTS` 清空（把历史纳入域）⇒ 实测 **30 pass / 4 fail**
  （`#25 #31 #32 #33`）⇒ 排除是承重的。

## 四、判据与注入

用例 31 → **34**（`tests/contracts/gate-coverage.test.mjs`，无新增测试文件 ⇒ 站点与逐字清单不变）。

新增三条：

- `the prebuild recipe has no second copy anywhere in the live documents` —— 基线零错误 + 非空洞三件
  （`documentsScanned == 50`、`fencedLinesScanned == 2096 > 0`、`prescriptionsScanned == 2 ==` 处方条数）
  + 空域具名 + 历史确实有副本 + 历史确实在域外。
- `a prebuild copied into a fence outside the block is named, prose is not` —— 同文档块外副本、
  他文档副本（带行号）、**散文 MUST NOT 红**、围栏不配对、补集接到对账入口上。
- `a flag profile no fixture carries is still carried through, one command per array` ——
  四种旗标剖面 + 一条非 `test` 数组，断言 N 进 N 出且逐条逐 token 相等。

注入验证（每次都 `sha256sum -c` 核对还原，基线
`gate-coverage.mjs = 319191f6…d29c`、`docs/runtime.md = 58e0bbb1…6fbe`、
`gate-coverage.test.mjs = 174547e0…af81`）：

| 编号 | 注入 | 期望 | 实测 |
| --- | --- | --- | --- |
| W1 | 白名单过滤 `--locked` | 新样本必红 | **33 pass / 1 fail**，红的正是 `#34`（W147 时同一注入是 31/31 全绿） |
| W2 | 按 `--lib` 过滤 | 多条红 | **26 / 8**（含 `#34`） |
| S1 | `docs/runtime.md` 追加围栏副本 | 具名 | 红：`docs/runtime.md:343 carries "cargo test … --no-run" — a second copy of a prebuild step` |
| S2 | 同一条命令写成散文 | 不许红 | **34 / 0**（与 S1 成对：区别只在围栏内外） |
| S3 | `docs/runtime.md` 围栏不配对 | 具名 | 红：`the fenced block opened at line 342 is never closed` |
| W4 | 历史目录纳入域 | 红 | **30 / 4**（`#25 #31 #32 #33`） |

S1 与 S2 用的是**同一条命令字符串**，只差围栏内外——这是本轮最关键的一对：
它证明判据判的是「可复制执行」这条界线，不是字符串匹配。

## 五、诚实边界

- **散文里的处方不在域内**。有人在散文里写一条错的预构建命令，判据看不见。这是决定一的代价，
  换来的是不必维护豁免名单。
- **未跟踪的活文档不在域内**。路径集来自 `.git/index`；游离在索引外的文件对判据不可见。
  登记可见性由 W135 的 `pending-registration` 管，两者**不互相替代**（一个管「有没有登记」，
  一个管「登记过的文档里有没有第二份处方」）。
- **`openspec/`（637 份 md）不在域内**。今天那里围栏内处方为 0（实测），故未纳入；纳入会让
  每一份变更文档的围栏都成为被判对象，代价与收益不成比例——如实登记为边界。
- **围栏解析不追求 CommonMark 完备**。四空格缩进的围栏可解析（按 `trim` 判）；四反引号包住三反引号
  时按长度判、不会误闭合；行内出现孤立 ```` ``` ```` 的段（如本报告 §一的表头）不会被当围栏——
  真正不支持的是「围栏里的围栏用同长度同字符」这种非法写法。
- **历史里的副本会继续增加**。每一次引用处方的报告都会多一份，这是记录的正常形态；判据永远不会
  去看那边，这是有意为之。
- **`--no-run` 是处方的唯一标记**。若将来出现别的预构建形态（例如 `cargo build --tests` 恰好等价），
  判据看不见它——但那需要先推翻 ADR 0121 的结论。

## 六、门控

`cargo xtask check --profile full`（`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1`）→ **第 1 次 `EXIT=0`**，
用时 2 分 43 秒。

| 项 | 值 |
| --- | --- |
| 门控内 `Compiling` | **0**（W147 的预构建仍然有效，本轮没有引入需要重编的 Rust 改动） |
| 门控内 `LNK1104` | **0** |
| Rust | **1147 / 0**（102 条 `test result` 行，与 W147 完全一致 ⇒ 本轮 Rust 零改动） |
| Node | **384 / 0**（15 组；W147 是 381 ⇒ **+3**，正是本轮新加的三条用例） |
| 依赖边界 | 19 个 workspace 包 |
| 准入登记 | `V3.11 acceptance + D01-D14 mapping` |
| `validate --all --strict` | 归档前 **44 / 0**；归档后 **43 / 0**（变更移出活跃集，−1） |
| `validate --archived` | 归档后 **145 / 0**（W147 是 144，+1 即本轮的归档变更） |

**「新用例真的在门控里跑了」有直接证据**，不是从总数倒推：门控日志里三条新用例各自是一条 TAP
子测试并被逐条判过——

```
# Subtest: the prebuild recipe has no second copy anywhere in the live documents
ok 32 - …
# Subtest: a prebuild copied into a fence outside the block is named, prose is not
ok 33 - …
# Subtest: a flag profile no fixture carries is still carried through, one command per array
ok 34 - …
1..34
# tests 34 / # pass 34 / # fail 0
```

这 34 条与本机单独跑 `node --test tests/contracts/gate-coverage.test.mjs` 的 34/0 是**同一组**，
所以「本机绿」在这里不是替代品——门控里跑的就是它。

**门控没有把本轮文档改动判红**：`docs/engineering.md` 新增那一段里的反例命令 `--no-run` 只出现在
**散文**里（L34–36 那条 `build --all-targets` 亦同），围栏内仍是块里那两条 ⇒ 补集判据在门控内为绿，
与 §四 S2 注入（散文不红）一致。这是「判据不判散文」这个决定在真实语料上的第一次检验。

## 七、CI 观测

提交 **`1d1af86`**（11 文件 / +767 −2）→ run **`39128537142`**（`Engineering foundation`，第 243 次），
`completed / success`，`head_sha` 完整 40 位一致（`1d1af8627dd4bab3bc52e4bdf8e885f6729499e4`）。
判结论三件齐（唯一查询参数 `cb=<纳秒>` + `Cache-Control: no-cache` + 回读 `head_sha`）；完成态由
90–120 s 间隔的有界轮询在第 5 次看到（`created 12:44:17Z` → `updated 12:53:12Z`，约 9 分钟），
随后又用 `/jobs` 与 `/runs/{id}` 各查一次做确认（**完成态共 3 次查询**，全同）。

| 平台 | 状态 | 步骤记录数 | 编号 | 被跳过 |
| --- | --- | --- | --- | --- |
| `foundation (ubuntu-24.04)` | **success** | 20 | `1–16` 后跳 `30–33` | `11`（仅失败时触发） |
| `foundation (windows-latest)` | **success** | 20 | `1–16` 后跳 `30–33` | `8` `14`（Linux-only）、`11` |

两平台各 20 条记录、编号 `1–16` 后跳 `30–33`（`if` 未触发的步不生成记录 ⇒ 条数不与 workflow 的
`- name:` 相减，与 W147 同形）。关键步全部 success：9 生成物无漂移、**10 `check --profile full`**、
12 凭据扫描、13 schema 与 API 契约、15 `pnpm spec:validate`、**16 `openspec validate --archived`**。

**本轮的重点是第 16 步**：W147 正是红在这里（归档的 `tasks.md` 还差两条）。本轮推送前本机
`--archived` 已是 **145 / 0**、归档的 26 条任务全 `[x]`，故这一步两平台全绿——W147 那条教训
（「`--archived` MUST 在**推送前**绿」）这次是被执行了，而不是被解释了。

第 10 步在**干净 checkout** 上全绿，说明本轮判据不依赖任何本机状态：活文档集来自 `.git/index`、
围栏内容来自仓库内文件本身，没有一条输入取自工作区残留（这正是 W147 判据「非空洞前提 MUST NOT
用工作区状态当输入」的那条要求在本轮的落点）。

## 八、具名登记（本轮发现但不做）

1. **W147 §八第 1 条仍未判**：本机 `LNK1104` 的宿主成因（可疑面收窄到「注册的杀软里只有 McAfee 在跑」）
   需关 WebAdvisor 或换 `CARGO_TARGET_DIR` 做对照实验，会动宿主配置。本轮没碰。
2. **`openspec/` 域外**：637 份 md、围栏内处方 0 份。纳入与否是成本问题不是正确性问题，
   今天选择不纳入（§五第 3 条）。
3. **散文域外**：不打算判。若将来真出现「散文处方的读者照着执行」，先把那条命令搬进围栏，
   判据会自动接管——这比给判据开豁免名单便宜。
4. **`docs/contracts/README.md` 在域内但没有围栏处方**：纳入是结构决定的（`docs/` 下任意深度），
   不是特意挑选；它今天为绿，没有额外成本。

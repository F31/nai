# Tasks: W148 — 预构建处方的补集：块外不得有第二份

## 1. 勘定（先把 W147 §八的两条量成数字）

- [x] 量 `testCommandsOf` 的两种过滤形状（注入 → 跑 → sha256 还原，基线
      `41dcd8ba99cc7199487fccdb4b2c0eb61dd55f59b45aa329ce347d342c8e566b`）：
      黑名单 `--exclude` ⇒ 25 pass / 6 fail；白名单 `--locked` ⇒ **31 pass / 0 fail**
- [x] 定位零信号的成因：夹具里每一条 `["test", …]` 都带 `--locked` ⇒ 白名单是恒等变换
- [x] 量补集的域：`docs/` 下 50 份活文档 / 2096 行围栏内容；围栏内处方 2 条（全在标记块里）
- [x] 量域外的副本：`docs/adr/0121`（2 条）与 `docs/reports/2026-10-10-*`（1 条）、
      `docs/reports/2026-10-18-w147-*`（2 条）共 5 条 ⇒ 历史必须按路径段排除
- [x] 量围栏是否配平：324 份 md / 421 个围栏 / 0 个不配对 ⇒ 解析器可以在真实语料上要求配平

## 2. 判据

- [x] `gate-coverage.mjs` 新增 `HISTORY_DOC_SEGMENTS` + `isLiveDocument`（域的唯一出处）
- [x] 新增 `fencedLines`：围栏长度/字符/信息串三条件判闭合，不配对即抛
- [x] 新增 `prebuildPrescription`：只认带 `--no-run` 的 `cargo` 命令（不带的是门控那一步本身）
- [x] 新增 `strayPrebuildErrors`：多重集补集 + 空域、不平衡围栏、处方未落围栏三条具名失败
- [x] 把补集**接进** `prebuildRecipeErrors`（对账入口一处，不新增第二处调用点）

## 3. 判别性样本

- [x] `load()` 从 `.git/index` 派生活文档集（不遍历工作区），`documents` 只装兄弟文档
      —— 同一份事实给两处会让夹具改一处、判据看另一处
- [x] +3 条用例（31 → 34）：补集基线 + 非空洞三件（域大小 / 围栏行数 / 处方计数）
- [x] 四种旗标剖面（裸 / `-p` / `--lib` / `--workspace --exclude`）的「N 进 N 出」样本，
      使任何按旗标挑命令的过滤必红
- [x] 围栏副本 / 散文 / 不平衡围栏三种形态各一条断言（散文那条 MUST NOT 红）

## 4. 注入验证（逐次 sha256 还原）

- [x] W1 白名单过滤 `--locked` ⇒ **34 条里只红 #34**（W147 时是 31/31 全绿）
- [x] W2 按 `--lib` 过滤 ⇒ 8 条红（含 #34）
- [x] S1 `docs/runtime.md` 追加围栏副本 ⇒ 红，点名 `docs/runtime.md:343`
- [x] S2 同一条命令写成散文 ⇒ 34/34 全绿（与 S1 成对：区别只在围栏内外）
- [x] S3 `docs/runtime.md` 围栏不配对 ⇒ 红，点名 opened at line 342 is never closed
- [x] W4 把历史目录纳入域 ⇒ 4 条红 ⇒ 排除是承重的，不是装饰

## 5. 文档与收口

- [x] `docs/engineering.md` L53–57 补一句「块外不得再出现处方」（含为什么不扫散文）
- [x] ADR 0122：域为何是围栏内行、为何只认 `--no-run`、为何按路径段排除两段历史
- [x] 轮次报告 `docs/reports/2026-10-18-w148-prebuild-complement.md`（八节）
- [x] `docs/development-plan.md` W148 段
- [x] 门控 `cargo xtask check --profile full`；两条 validate（含 `--archived`）在推送前绿
- [x] 一次 commit + push；CI 双平台观测；回填 §六/§七

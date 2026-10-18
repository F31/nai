# Tasks: W151 — 文档内部指针的可达性

## 1. 勘定（先量，再判）

- [x] 量 ADR 0124 残余第 1 条（归档 `tasks.md` 的段结构）：147/147 齐备，但 **39 份没有 `## ` 段**、
      段标题 **375 种**（311 种只出现一次）、**52 个段内 0 条任务** ⇒ 「结构」没有统一形状，
      判据只剩「非空 + 全勾选」，而后者已被 `--archived` 覆盖 ⇒ **不立项**
- [x] 转向同族真缺口：`docs/` 的 831 条链接里 **36 处走不到**（32 断链 + 4 通配），跨 **25** 份文件
- [x] 量域边界：外部链接 **37**、纯锚点 **0**、引用式链接定义行 **0**、图片链接 **5**、角括号 **0**
- [x] 证实现有门控零信号：`--strict` / `--archived` / registry / 各契约判据都不解析 markdown 链接
      （仓内无任何链接解析器，`grep -rn '\]\(' scripts/` 只有判据自身的正则）

## 2. 判据

- [x] `scripts/contracts/doc-links.mjs`：`linkTargetsOf` / `isExternalTarget` / `resolveTargetOf` /
      `docLinkErrors`（纯函数，域与索引由调用方注入）
- [x] 规则一：相对链接解析后 MUST 在 `.git/index` 里（域与 W75 同源，未跟踪文件域外）
- [x] 规则二：**目录算存在**——前缀下有已跟踪文件即通过；不带尾随斜杠同样算
- [x] 规则三：**通配直接失败**（`*` / `?`），MUST NOT 走「文件不存在」那条（否则它在「能解析」名义下逃逸）
- [x] 规则四：外部 / 协议相对 / 纯锚点在域外；不可解析（越界、非法 `%`）MUST 分类失败，MUST NOT 静默跳过
- [x] 两个空集守卫：文档集为空、在域内链接为 0 ⇒ 各自具名失败

## 3. 用例与接线

- [x] `tests/contracts/doc-links.test.mjs`：**9** 条用例
- [x] 真实语料四项非空洞：域非空、`documentsChecked == 索引里 docs/*.md 的份数`（全量非抽样）、
      `linksChecked > 0`、文档路径集合不塌缩
- [x] 合成样本：断链（带文件:行号）/ 目录（含尾随斜杠、不含、空目录）/ 通配 `*` 与 `?` /
      外部与纯锚点 / 越界 / 非法 `%` / 正确 `%` 编码 / 三种空集
- [x] 接线：`engineering_checks` 加一步、`TEST_REGISTRATION_SITES` 增加 `xtask-doc-links`、
      逐字清单「十九个」→「二十个」
- [x] `tests/registry.test.mjs` 57 / 0

## 4. 修复 36 处

- [x] 缺 `../` 的 **15** 处（报告在 `docs/reports/` 里按 `docs/` 写相对路径：`adr/…` → `../adr/…`、
      基线文件名同理）
- [x] 多一层 `docs/` 的 **4** 处、目录深度错 **2** 处（`../changes/archive/…` →
      `../../openspec/changes/archive/…`）、兄弟文档写成子目录 **2** 处（`kernel.md` → `../kernel.md`）
- [x] 日期笔误 **2** 处（`2026-10-10-engine-manual-event-pipeline.md` → `2026-10-09-…`，同目录唯一候选）
- [x] ADR 文件名笔误 **1** 处（`0001-workspace-layout-and-naming.md` → `0001-project-name-and-design-baseline.md`，
      编号唯一确定）
- [x] 通配 **4** 处 → 按同一行已写明的编号补全真实文件名（ADR 0051 / 0052）
- [x] 指向**从未存在的文件**的 **8** 处：`0027-trace-id-single-definition.md`（6 处）降级为纯文本
      ——`git log --follow` 证明 ADR 0027 自创建起就是「部署生命周期的边界」，编号不可复原，MUST NOT 猜；
      `0015-decision-log-criticality.md`（2 处）就地订正为 **ADR 0019 决策 5**（唯一正文出处）
- [x] 6 份 ADR 各追加 `## 勘误（W151，指针）` 段；正文一字未动

## 5. 注入验证（逐次 `sha256sum -c` 还原）

- [x] I1 关掉目录豁免 ⇒ **2 红**（`#1` 真实语料 6 处目录链接、`#3`）
- [x] I2 放行通配 ⇒ **只红 `#4`**（真实语料已无通配 ⇒ 判别性样本）
- [x] I3 少看一份文档（抽样）⇒ **只红 `#1`**（非空洞断言在承重）
- [x] I4 把外部链接纳入域 ⇒ **3 红**（`#1` `#5` `#7`）
- [x] I5 去掉「零相对链接」守卫 ⇒ **只红 `#7`**
- [x] I6 越界不判为不可解析 ⇒ **2 红**（`#6` `#9`）

## 6. 文档与闭环

- [x] ADR 0125：指针订正的边界（URL 就地订正 ≠ 改写历史；不可复原 ⇒ 降级 + 勘误段）
- [x] 报告 `docs/reports/2026-10-18-w151-doc-links.md`（含域边界与四处零样本的如实披露）
- [x] `docs/development-plan.md` 新增 W151 段（并补 W150 回填提交的 CI 观测）
- [x] 全量门控 `cargo xtask check --profile full`
- [x] 归档 + 两条 validate（`--all --strict` / `--archived`）
- [x] 一次 commit + push + CI 双平台观测 + §六/§七 回填

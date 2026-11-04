# V4.8 技术白皮书

以 `docs/digital-host-agent-design-V4.8.md` 为唯一依据整理的技术白皮书，含六张由 `archify` 生成的图。

## 交付物

| 文件 | 内容 |
|---|---|
| [nai-v4.8-technical-whitepaper.html](nai-v4.8-technical-whitepaper.html) | 白皮书正文（单页、深色、可打印），内嵌全部六张图 |
| [diagrams/01-process-boundaries.html](diagrams/01-process-boundaries.html) | 图 1 进程边界与双门控（architecture） |
| [diagrams/02-live-answer-chain.html](diagrams/02-live-answer-chain.html) | 图 2 一条评论到一次上屏（workflow） |
| [diagrams/03-utterance-lifecycle.html](diagrams/03-utterance-lifecycle.html) | 图 3 一次话语的交付生命周期（lifecycle） |
| [diagrams/04-performance-tracks.html](diagrams/04-performance-tracks.html) | 图 4 表演子系统（architecture） |
| [diagrams/05-publication-impact.html](diagrams/05-publication-impact.html) | 图 5 事实变更的传播路径（dataflow） |
| [diagrams/06-latency-budget.html](diagrams/06-latency-budget.html) | 图 6 动态短答的三秒预算（waterfall） |

另有一份 Word 版：`nai-v4.8-technical-whitepaper.docx`（正文 41 页，六张图各自一页横排）。
它由上面这份 HTML 转换而来，不引入第二份内容源——两者冲突时以 HTML 正文为准。

`diagrams/*.delivery.json` 是各产物的交付回执（provenance），`check --require-provenance` 依赖它。

## 图的可编辑源

图的候选 JSON 保存在仓库根目录的 `.archify/<type>-v48-<slug>-20261023-180435>/candidate.json`（未纳入版本控制）。
改动后按「validate → deliver → check → browser-check」重跑即可，例如：

```bash
node <archify>/bin/archify.mjs deliver architecture .archify/architecture-v48-boundaries-20261023-180435/candidate.json docs/whitepaper/diagrams/01-process-boundaries.html --json
node <archify>/bin/archify.mjs check docs/whitepaper/diagrams/01-process-boundaries.html --require-provenance --json
```

## 证据状态

- 六张图均通过 `validate`、`deliver`、严格 provenance `check` 与真实 Chrome `browser-check`（containment / readability / viewerChrome / themeStates）。
- 未做人工视觉评审（`visualReview: not-requested`）。
- 本机 Windows 上 `finalize` 的证据发布步骤会因同卷硬链接无法删除而失败；改为分步执行并把 `--out-dir` 放在系统盘，是同一组门控的等价路径，不改变任何判定口径。
- 白皮书中的所有性能、资源与费用数字均为设计预算或目标，未实测；能力状态默认「待实施 / 待验证」。

## Word 版是怎么来的

转换脚本在 `../../.archify/whitepaper-docx/`（本地工具，未纳入版本控制）。它不做 HTML→DOCX 的通用转换，
而是按本文件已知的语义块（`h2/h3`、`p.lede`、`div.card`、`div.note`、`div.tbl`、`div.stat-row`、`figure`）
逐类映射到 Word 样式；遇到未识别的块会直接报错退出，不静默丢内容。

三处需要留意的取舍：

- **图按浅色重绘**。`diagrams/*.html` 是深色交互页，直接截图贴进白纸会是一块深色砖。脚本抽出内联 `<svg>`
  与页面样式，以 `data-theme="light"` 重建最小页面再截图（2400px 宽），因此 Word 里的图与交互页**同源同几何**，
  只是配色不同。正文里的交互页链接仍指向深色原版。
- **六张图各占一页横排**。图 3/图 6 的宽高比接近 4:1，缩进 A4 竖排正文宽只有 16.4cm，图内文字会掉到 4–5pt。
  横排页用满 26.4cm，图内正文约 7.5–8pt。图较矮时竖向居中。
- **目录是静态的**。目录域交给 Word 生成，行距会跟内建 `TOC 1` 样式走、且要手动更新域。这里改成两遍排版：
  第一遍导出 PDF 量出每个标题的印刷页码，写入目录后再导出核对（漂移应为 0）。代价是增删正文后页码不会自动更新，
  页面上已标注这一点。

重新生成的顺序：`build_docx.py`（第一遍）→ Word 导出 PDF → `extract_pages.py`（量页码）→ `build_docx.py`（第二遍）
→ 导出 → `verify_pages.py` 核对；`rebuild.sh` 把这几步串起来。依赖：python-docx、Pillow、PyMuPDF，以及本机 Word
（仅用于导出 PDF 复核版式，生成 docx 本身不需要 Word）。


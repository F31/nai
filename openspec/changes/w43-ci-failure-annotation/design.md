## Context

观测链路的三段（全部实测，见 W42 报告勘误段）：

| 通道 | 匿名可读？ | 内容 |
| --- | --- | --- |
| `GET /actions/runs?per_page=N` | ✅ | run 级 `status`/`conclusion` |
| `GET /actions/runs/{id}/jobs` | ✅ | job 与**每一步**的 `conclusion`（步级） |
| `GET /check-runs/{id}/annotations` | ✅ | 目前只有 `Process completed with exit code 1.` |
| `GET /actions/jobs/{id}/logs` | ❌ 403 | 真正的日志（测试名、断言、编译器输出） |

缺口只在最后两行之间：**注释位可读、内容空**。本轮补的正是这一段。

## Goals / Non-Goals

**Goals**

- CI 门控失败时，**无凭据**的读者能读到失败原因（测试名 / 断言 / 具名拒绝信息）。
- 采集输出这件事本身**绝不改变**门控的判定结论。
- 诊断通道的失败**绝不掩盖或替换**原始失败。
- 平台无关：Linux 与 Windows 走**同一条**命令，不出现两份会漂移的 shell 逻辑。

**Non-Goals**

- 不修那个已知的 ubuntu 间歇失败（本轮只是把「查不到原因」变成「查得到」）。
- 不引入本机复现手段（`act`/Docker/WSL 均不可用，已实测）。
- 不把 GitHub 的 workflow command 语法写进 `xtask` 或前端门控——那是 CI 关注点，不是工具关注点。

## Decisions

### D1：用 `::error::` workflow command，而不是 job summary 或 artifact

三条候选通道里只有 annotation **匿名可读**：job summary 存在 check-run 之外、API 读不到；
artifact 下载需要登录；`::error::` 直接落进 `GET /check-runs/{id}/annotations`。
选 `error` 而非 `warning` 级：它的语义就是「失败摘要」，且不影响 job 结论（结论由步骤退出码决定）。

### D2：门控步改用 `shell: bash` + `set -o pipefail` + `tee`

- **为什么统一 bash**：Windows job 默认是 pwsh，若为「重定向到文件」写两套 shell 逻辑，就是两处会漂移的代码。
  runner 自带 Git for Windows，`shell: bash` 在两个平台都可用，于是**同一条命令**覆盖两个平台。
- **`set -o pipefail` 是必需的，不是风格问题**：`cmd | tee file` 的退出码是 `tee` 的（恒 0）。
  本机实测（Git Bash，与 CI 同构）：
  - 有 `set -o pipefail`：`cargo run ... check --profile bogus | tee …` ⇒ **EXIT=1**；
  - 无 `set -o pipefail`：同一条命令 ⇒ **EXIT=0**。
  也就是说少了这一行，**CI 会永远绿**，且看上去一切正常。
- **为什么 `tee` 而不是重定向**：重定向会同时丢掉 stdout 与 stderr 之外的信息、且不便于在失败时回看完整输出；
  `tee` 让日志既落盘又照常打印。

### D3：诊断逻辑放在仓库脚本 `scripts/ci-diagnose.mjs`，不放 workflow 内联 YAML

内联 YAML 里的 shell 无法本机测试、无法逐条断言、也逃不掉 shell 方言问题。仓库已有
`scripts/frontend-gate.mjs` 这一类「CI 相关逻辑写在脚本里」的先例，且 CI 已装 Node ⇒ 零新依赖。

### D4：诊断步骤 `continue-on-error: true`，且脚本恒以 0 退出（双重保险）

诊断只读取日志并打印注释，**永不判定成败**。脚本对「日志缺失/为空/读不了」都返回 0；
参数错误这类**脚本自身缺陷**返回非零，由 `continue-on-error: true` 兜住——
即使脚本写坏了，job 的失败结论仍由门控步骤决定，绝不会被「诊断失败」替换。

### D5：逐行输出 + 失败信号优先 + 长度截断

- **逐行**：GitHub 会对超长 annotation 做截断；一整块日志塞进一条注释，关键行会被切掉。
- **信号优先**：只取尾部会漏掉「错误在前、大段 cargo 输出在后」；只取信号行则在纯编译失败或措辞特殊时全空。
  故先取匹配 `error|failed|failures?|panicked|assertion|unsupported|not implemented|unimplemented|失败|未通过|不通过`
  的行，不足再用尾部普通行补齐，并按原顺序输出。
- **截断**：单行上限（默认 400 字符）与条数上限（默认 25）都是显式参数，可按需调整；
  省略时另发一条说明「另有多少行未展示」，不让读者误以为看到了全部。
- **剥离 ANSI**：cargo 的颜色码写进注释只会变成乱码。

### D6：端到端验证用**临时分支**制造一次真实失败

「本机跑脚本输出正确」不等于「GitHub 会把 `::error::` 解析成可读的 annotation」。
要证明后者，必须让 CI 真的红一次。做法：
1. `main` 正常推送 ⇒ 观测正常路径两个 job 仍全绿（验证新写法没有破坏正常流程）；
2. 推临时分支 `ci-annotation-probe`，**只改一行**——把门控命令换成 `cargo xtask check --profile bogus`
   （xtask 会以具名信息 `unsupported or not implemented task: …` 非零退出，失败形态确定且可预期）；
3. 观测两个平台的 `annotations` 能否匿名读到该信息；
4. 删除该分支。

代价：仓库永久留下一个失败 run 记录（GitHub 不支持删除 run 历史）。已在 proposal 的 Impact 中披露。

### D7：不动 `xtask`，不把 CI 语法渗进工具

`::error::` 是 CI 关注点。把它写进 Rust 工具会让工具在没有 CI 的环境里产生无意义输出，
也会让「工具的失败语义」与「CI 的呈现」耦合。诊断因此全部落在 `scripts/` 下的独立脚本。

## Risks / Trade-offs

- **Windows runner 上 Git Bash 能否看到 `cargo`**：`bash` 会继承 Actions 设置的 PATH（含 rustup 的 shim 目录），
  应当可见；但这是本机无法完全证实的假设。D6 的 probe 在**两个平台**上跑，因此该假设**会被实测覆盖**——
  若不可见，诊断步骤也会随之失败（而 `continue-on-error` 保证不影响原始结论）。
- **`$RUNNER_TEMP` 在 Windows 是 `D:\a\_temp`**：Git Bash 展开后作为参数传给 Node，Node 接受该路径。
- **注释条数**：GitHub UI 每个 step 只折叠显示有限条注释，API 可读全部。本脚本默认 25 条，是显示与信噪比的折中。
- **日志体积**：`tee` 只保留单次 job 输出，不上传 artifact，不构成新的存储依赖。

## Rollback

删除诊断步骤、把门控步改回 `run: cargo xtask check --profile full`、删除 `scripts/ci-diagnose.mjs` 即可，
三者互不依赖。规格差量随之作废。
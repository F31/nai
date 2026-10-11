# W43：门控失败以匿名可读的 annotation 发布摘要

- 变更：`openspec/changes/w43-ci-failure-annotation`（归档后为 `openspec/changes/archive/2026-10-11-w43-ci-failure-annotation/`）
- 前置：W42（`docs/reports/2026-10-11-w42-ci-complete-profile.md` 及其勘误段）
- 性质：**观测能力**建设。零 Rust 源码改动、零新依赖、不新增 workspace 包、**不改任何验收 `status`**。

## 1. 要补的是哪个缺口

W42 的勘误段实测了四段观测链路，缺口只在最后两段之间：

| 通道 | 匿名可读 | 内容 |
| --- | --- | --- |
| `GET /actions/runs?per_page=N` | ✅ | run 级 `status` / `conclusion` |
| `GET /actions/runs/{id}/jobs` | ✅ | job 与**每一步**的 `conclusion`（步级） |
| `GET /check-runs/{id}/annotations` | ✅ | **只有** `Process completed with exit code 1.` |
| `GET /actions/jobs/{id}/logs` | ❌ 403 `Must have admin rights to Repository.` | 真正的日志（测试名、断言、编译器输出） |

**注释位可读、内容是空的。** 后果很具体：那 13 次 ubuntu `Portable foundation` 间歇失败只能定位到**步级**，
定位不到**测试级**——本机非 Linux（`wsl.exe` 被安全策略禁止、无 Docker）、无 `gh` CLI、无仓库凭据。

GitHub 会把 workflow command（`::error::…`）解析成 check-run 的 annotation，而 annotations API **匿名可读**。
把门控日志的失败摘要转成 annotation，就用**唯一匿名可读的那条通道**传递失败原因——不需要新权限、不需要 runner、不需要凭据。

## 2. 改动

### `.github/workflows/foundation.yml`

门控步骤改为采集输出，并新增一个只在失败时运行的诊断步骤：

```yaml
      - name: Complete engineering foundation (portable steps + frontend workspace)
        shell: bash
        run: |
          set -o pipefail
          cargo xtask check --profile full 2>&1 | tee "$RUNNER_TEMP/nai-gate.log"
      - name: Publish the gate failure summary as annotations
        if: failure()
        shell: bash
        run: node scripts/ci-diagnose.mjs "$RUNNER_TEMP/nai-gate.log"
        continue-on-error: true
```

- **`set -o pipefail` 是必需的，不是风格问题**：`cmd | tee file` 的退出码是 `tee` 的（恒 0）。本机实测（Git Bash，与 CI 同构）：
  同一条 `cargo run … check --profile bogus | tee …`，**有** `pipefail` ⇒ `EXIT=1`；**无** `pipefail` ⇒ `EXIT=0`。
  少了这一行，**CI 会永远绿且看上去一切正常**。
- **统一 `shell: bash`** 而不为 Windows 另写一份 pwsh 逻辑：两份 shell 就是两处会漂移的定义；runner 自带 Git Bash。
- **`continue-on-error: true`**：诊断永不判定成败，绝不掩盖或替换原始结论。

### `scripts/ci-diagnose.mjs`（新）

读门控日志 → 挑行 → 逐行输出 `::error title=gate-failure-summary::…`。

- **失败信号行优先 + 尾部兜底**：只取尾部会漏掉「错误在前、大段 cargo 输出在后」；只取信号行则在纯编译失败
  或措辞特殊时全空。信号覆盖 `error`、`failed`、`failures?`、`panicked`、`assertion`、`unsupported`、
  `not implemented`、`unimplemented`、`失败`、`未通过`、`不通过`。
- **逐行**输出：整块塞进一条注释会被服务端截断，关键行会被切掉。
- **双层截断**：单行默认 400 字符、条数默认 25（均为显式参数），省略时另发一条说明「另有多少行未展示」。
- **剥离 ANSI**（CSI 与 OSC 两类）：cargo 的颜色码写进注释只会变成乱码。
- **转义** `%`、`\r`、`\n`，property 值另转义 `:`、`,`。
- **日志缺失/为空/读失败 ⇒ 退出 0 且不输出任何 annotation**：诊断 MUST NOT 凭空制造噪音或伪失败。

### 规格

`engineering-foundation` 新增三条需求：失败摘要 MUST 以**无需凭据**即可读的 annotation 发布；
采集输出 MUST NOT 改变门控判定；日志缺失或为空时 MUST NOT 产生任何 annotation。
（这条通道若不写进规格，会被后人当冗余删掉——它的唯一价值就是「可观测」。）

## 3. 验证

### 3.1 本机

| 项 | 结果 |
| --- | --- |
| 脚本：日志缺失 | `EXIT=0`，输出「日志不存在；不输出 annotation」，**无 annotation** |
| 脚本：日志为空 | `EXIT=0`，**无 annotation** |
| 脚本：无参数 | `EXIT=2` + usage（脚本自身缺陷应当被发现） |
| 脚本：合成样本（ANSI + `%`/`::`/`,` + 600 字符超长行 + CRLF） | 14 条 annotation；ANSI 已剥离、`%`→`%25`、超长行截断为 400 字符含 `…`、**保持原顺序** |
| 脚本：`--max-lines 5 --max-chars 45` | 5 条 + 「另有 1 行未展示」⇒ 信号优先与省略提示均生效 |
| 脚本：**真实** xtask 具名拒绝日志 | 1 条：`xtask: unsupported or not implemented task: check --profile bogus (use cargo xtask help)` |
| **`pipefail` 反向自检** | 有 ⇒ `EXIT=1`；无 ⇒ `EXIT=0` |
| workflow 结构断言（Node 解析 YAML，临时脚本） | **21/21 通过**（含 `shell: bash`、`set -o pipefail`、`tee` 路径、诊断步存在/靠后/`if: failure()`/`continue-on-error`） |
| `check --secrets` | **720 文件零命中** |
| `openspec validate --all --strict` | 通过（含 `change/w43-ci-failure-annotation`） |

> **一处如实记录的自身失误**：结构断言第一版误报 6 项失败。原因是**断言脚本自身有 bug**——
> 用只收录含 `run` 字段的 `runs` 数组索引去取 `steps`，被仅有 `uses` 的步骤（`checkout`/`setup-node`）错位。
> workflow 本身当时是正确的。这条记进报告，因为它正是「结构断言比肉眼可靠、但断言脚本自身也必须自证」的例证。

### 3.2 端到端（两个方向都实测）

「本机跑脚本输出正确」**不等于**「GitHub 会把 `::error::` 解析成可读的 annotation」。要证明后者必须让 CI 真的红一次。

**正常路径**（`main`，`9e6ddfe`，run [`38108824691`](https://github.com/F31/nai/actions/runs/38108824691)）：

| job | 结论 | 门控步 | 诊断步 |
| --- | --- | --- | --- |
| `foundation (ubuntu-24.04)` | `success` | `success` | **`skipped`** |
| `foundation (windows-latest)` | `success` | `success` | **`skipped`** |

⇒ 新的 `shell`/`pipefail`/`tee` 写法**没有破坏正常流程**；成功时诊断步被 `if: failure()` 跳过，**不产生任何注释**。

**失败路径**（临时分支 `ci-annotation-probe`，`70e9c36`，run [`38108847930`](https://github.com/F31/nai/actions/runs/38108847930)，
只把门控命令换成 `check --profile bogus`）：

| job | 结论 | 门控步 | 诊断步 |
| --- | --- | --- | --- |
| `foundation (ubuntu-24.04)` | `failure` | **`failure`** | `success` |
| `foundation (windows-latest)` | `failure` | **`failure`** | `success` |

**匿名读取 `GET /check-runs/{id}/annotations` 的实测内容**（无任何凭据）：

```
# check-run 114379989218（ubuntu-24.04）—— 5 条
[warning]  Node.js 20 is deprecated. …
[failure] gate-failure-summary :: xtask: unsupported or not implemented task: check --profile bogus (use cargo xtask help)
[failure] gate-failure-summary ::      Running `target/debug/nai-xtask check --profile bogus`
[failure] gate-failure-summary ::     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
[failure]  Process completed with exit code 1.

# check-run 114379989179（windows-latest）—— 6 条
[failure] gate-failure-summary :: error: process didn't exit successfully: `target\debug\nai-xtask.exe check --profile bogus` (exit code: 1)
[failure] gate-failure-summary :: xtask: unsupported or not implemented task: check --profile bogus (use cargo xtask help)
[failure] gate-failure-summary ::      Running `target\debug\nai-xtask.exe check --profile bogus`
[failure] gate-failure-summary ::     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.16s
[failure]  Process completed with exit code 1.
```

⇒ **通道成立**：失败原因现在可以在**无凭据**条件下读到，且**两个平台都成立**。
两个平台的门控步都是 `failure`（不是被 `tee` 吞成 0），**反过来证明 `set -o pipefail` 在 Windows 的 Git Bash 下同样生效**。
Windows 上多出的那条 `error: process didn't exit successfully …` 正是**信号优先**策略抓到的最有价值的一行
（Linux 版 cargo 不输出该格式）。

probe 分支已在验证后删除（本地与远端）。

## 4. 代价与边界（如实披露）

- **仓库永久留下一个失败 run 记录**（run `38108847930`）。GitHub 不支持删除 run 历史，这是本轮已知的、可接受的代价，
  已在 proposal 的 Impact 中事先声明。
- 本轮**只解决「查不到原因」，没有解决那个间歇失败本身**。原因仍未知；变化是：下一次它发生时，
  失败测试名与断言会**自动出现在匿名可读的注释里**，不再需要 admin 权限的日志。
- 注释条数：GitHub UI 每个 step 只折叠显示有限条，**API 可读全部**。本脚本默认 25 条，是显示与信噪比的折中。
- 诊断步骤用 `if: failure()`，因此**任何**前置步骤（checkout、setup-node…）失败时它也会运行；
  此时日志不存在，脚本按契约**不输出注释并成功退出**，`continue-on-error` 再兜一层。

## 5. 未做（有意）

- 不修那个 ubuntu 间歇失败（原因未知；本轮只是把「查不到」变成「查得到」）。
- 不引入 `act`/Docker/WSL 等本机复现手段（均已实测不可用）。
- 不把 `::error::` 这类 CI 语法写进 `xtask` 或前端门控——那是 CI 关注点，写进工具会让工具在没有 CI 的环境里
  产生无意义输出，也会把「工具的失败语义」与「CI 的呈现」耦合。
- 不动 `scripts/frontend-gate.mjs` 一行；不新增依赖；不新增 workspace 包。

## 勘误（2026-10-11 同日复核）：通道的第一次真实使用暴露了选取缺陷

**触发事件**：run [`38109458805`](https://github.com/F31/nai/actions/runs/38109458805)（提交 `1aee4d6`，
**只含文档与归档改动**，与此前全绿的 `9e6ddfe` **代码完全相同**）在 `foundation (ubuntu-24.04)` 上
`Complete[failure]` + `Publish[success]` ⇒ §1 登记的那个间歇失败**再次发生**，
且再次发生在**只改文档**的提交上（再次印证非确定性）。

诊断步骤如约产出了注释——**通道本身成立**。但**内容被噪音淹没**：匿名读到的 12 条里

```
[failure] gate-failure-summary :: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; …   ← ×8
[failure] gate-failure-summary ::     Checking thiserror v2.0.21
[failure] gate-failure-summary ::    Compiling thiserror-impl v2.0.21
[failure]  Process completed with exit code 1.
```

**真正的错误行一条都没有。** 缺陷在**选取逻辑**，不在通道。

### 缺陷一：信号正则把小写 `failed` 当成失败信号

cargo 对**每个成功**的测试目标都印 `test result: ok. N passed; 0 failed; …`，`/failed/i` 把它们全部判成失败行，
瞬间占满配额。修复：拆成**强信号**（`error`、`FAILED`、`panicked`、`unsupported`、`not implemented`、
`unimplemented`、`assertion`、`失败`、`未通过`、`不通过`，其中 `FAILED` **区分大小写**——cargo 的失败总结行
`test result: FAILED.` 是大写）与**弱信号**（`failures?:`）；并新增 **NOISE** 规则：整行以 `test result: ok.`
开头即**永不入选**。

### 缺陷二：按行内容去重，让重复行被整体捞回

第一版用 `Set` 存**行内容**再 `lines.filter(chosen.has)`。日志里大量存在重复行（每个成功目标各印一遍），
按值匹配会把「同内容的所有行」一起选中——实测把上限 25 冲成 **803 条**。
同处还有 `rest.slice(-0)` 返回**整个数组**的陷阱（`-0 === 0`）。
修复：改为**按索引**选取（索引天然去重且保持原序），尾部起点写成 `Math.max(0, len - budget)`。

同时，强信号改为在**全量日志**里找，而不是只看尾部——cargo 会先印上千行 `Compiling`，只看尾部会漏掉根因。

### 修复后的复验（本机，真实形态样本）

2307 行样本：1500 行 `Compiling` + 中部 `error[E0599]` 及其 4 行上下文 + **800 行 `test result: ok.`** +
尾部 `test result: FAILED.` 与 `error: could not compile …`。结果：**输出 25 条**（守住上限），
三条真错误全部命中且带上下文，`test result: ok.` 全部排除。

### 仍未复验的部分（如实标注）

- **本次选取修复尚未在真实 CI 失败上复验**——它要等下一次 ubuntu 间歇失败（届时自动验证，无需干预）。
- GitHub 对 `::error::` 的解析与匿名可读性，已由 §3.2 的 probe **实测证明**，不受本次修复影响。
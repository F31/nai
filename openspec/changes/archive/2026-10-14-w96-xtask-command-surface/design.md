# w96 设计

## 一、问题形状

`xtask` 是本机门控的**入口面**：`cargo xtask <...>` 的每一条命令都从
`xtask/src/lib.rs` 的 `execute()` 这个 `match` 里分发出去。这个 match 有 **24 条臂**，
而它**不在任何判据的受判范围内**：

```
[ 1] ["check-deps"]                          => deps::check(root)
[ 2] ["check", "--registry"]                 => registry::check(root)
[ 3] ["check", "--golden"]                   => golden::check(root, None)
[ 4] ["check", "--golden", "--dir", dir]     => golden::check(root, Some(dir))
[ 5] ["check", "--secrets"]                  => secrets::check(root)
[ 6] ["sbom"]                                => sbom::generate(root, None)
[ 7] ["sbom", "--out", path]                 => sbom::generate(root, Some(...))
[ 8] ["schema"]                              => schema::generate(root, false)
[ 9] ["schema", "--check"]                   => schema::generate(root, true)
[10] ["test", "--suite", "contracts"]        => schema::contracts(root)
[11..18] ["test", "--suite", <8 个套件>]      => run(root, &["test", …])
[19] ["check", "--profile", profile @ ("portable" | "windows-local")] => …
[20] ["check", "--profile", "full"]          => …
[21] ["help"] | ["--help"] | []              => println!("Usage: …")
[22] _                                       => Err(unsupported)
```

同一文件里另有一处**同一件事的第二次书写**：`println!` 里的
`Usage: cargo xtask check-deps | check --registry | …` 字符串。两处**没有任何机制**要求一致。

同族的既有先例：W84（**测试文件**有没有被接上电）、W86（**OpenAPI 文档集**）、
W95（**CI workflow 的 step**）。本轮给「**`execute()` 的命令面**」补——同一手法，**新的一层对象**。

## 二、证据（三个探针，都真跑）

| 探针 | 改动 | 结果 |
| --- | --- | --- |
| P1 加法 | 往 `execute()` 插入一条臂 `["check", "--audit"] => Ok(())` | registry **52**、schema **30**、platform **22**、workflow **13** —— **全绿、零信号** |
| P2 减法 | 删掉 `["check", "--secrets"] => secrets::check(root)` 这条臂 | 同样 **52 / 30 / 22 / 13** —— **一条判定命令静默消失** |
| P3 漂移 | usage 字符串追加 `\| check --audit`（声明一条不存在的命令） | 同样全绿 —— usage 与 dispatch **分家** |

每次注入后 `sha256` 改前记录、`git checkout --` 还原、逐字校验
（`xtask/src/lib.rs` 还原后 `sha256 = bfdd6d2e0a92f29ab4939df9e8c6f32704b6758b5f9f9e3adbcdfcdcedaf7bec`，
三次复现一致）。

增补核实：`grep -rn "Usage:" tests/` **为空**、`grep -rn "\-\-registry\"\] =>\|as_slice" tests/` 无命中
⇒ 没有别的守卫在读命令面或 usage。**判定的消失没有信号**：`cargo xtask check --profile full`
照旧 `EXIT=0`、目标数不变、用例数不变。

### 派生实测（解析两侧，确认设计可行）

一个纯 JS 探针（`.w96-probe.mjs`，**未跟踪，不入库**）用括号计数法定位 match 的体，
再用正则取臂模式，把 `profile @ ("portable" | "windows-local")` 这类**标识符锚**抹掉后
展开字面量笛卡尔积：

```
dispatch (24)   ← execute() 的臂
usage    (21)   ← Usage: 字符串展开（可选段 [--dir <path>] / 多值 portable|windows-local|full）
usage \ dispatch  = []
dispatch \ usage  = ['help', '--help', '']
```

⇒ **双向对账成立且当前是干净的**：usage 不多说一条命令，dispatch 也不漏写一条
（三个 help 类别本就不该出现在 usage 里，因为它们就是「打印 usage」本身）。

## 三、设计决定

### 决定 1：判定单元是**命令签名**，不是命令名（W91）

签名 = **空格连接的顶层字面量序列**。理由：「同名」≠「同一个」——`check --golden` 与
`check --golden --dir` 是两条命令，只按名字看会合成一条。同样，`test --suite X` 的 8 条
共享前缀 `test --suite`，签名把它们区分开。匹配锚是**整条签名**（`["check","--secrets"]`），
不是 `"--secrets"`——后者会在别的对象上命中（本仓库有 `check --secrets` 与
`secrets::check` 两个不同对象，W91 的 `nai-plugin-sdk` 同名事故的重演风险）。

### 决定 2：usage 与 dispatch **对账**，而不是**消除第二处**

W93 的「同一件事 MUST NOT 两处定义 ⇒ 调用第一处」在这里**做不到**：usage 是给人看的自然语言，
不可能由 match 臂渲染（那会把 usage 变成机器产物，失去它的作用）。⇒ 采取**第三态**：
承认两处，**断言一致**——`usage ⊆ dispatch` 与 `dispatch \ usage ⊆ HELP_SIGNATURES`。

`HELP_SIGNATURES = ['help', '--help', '']` 是**具名例外**（不是「其余随便」）：
少写三条 help 是有理由的（它们不执行任何命令），而这个理由 MUST 被**逐字点名**，
否则「少写一条判定命令」会混进同一个豁免里。

### 决定 3：`gate` 必须引出一处**真的存在**的**门控落点**

「本机门控真的跑它」不能是空口的话（W75「已声明 ≠ 已执行」）。`gate` 的每条带
`{file, marker}`，marker MUST 在**门控区域**内逐字存在。门控区域由解析得出，共三块：

| 区域 | 起点 | 覆盖的命令 |
| --- | --- | --- |
| `engineering_checks` 的函数体 | `fn engineering_checks(` | `check-deps`、`check --registry` |
| `check --profile portable\|windows-local` 的 arm 体 | 该 arm 的 `=>` 后的 `{}` | `check --profile portable`、`check --profile windows-local` |
| `check --profile full` 的 arm 体 | 同上 | `check --profile full` |

实测（`.w96-probe2.mjs`）五条 `gate` 的落点全部命中：

```
OK  "check-deps"                 <- "deps::check(root)?"                      @ [engineering_checks]
OK  "check --registry"           <- "registry::check(root)?"                  @ [engineering_checks]
OK  "check --profile portable"   <- "engineering_checks(root)?"               @ [portable-arm, full-arm]
OK  "check --profile windows-local" <- "\"windows-local requires a native …\"" @ [portable-arm]
OK  "check --profile full"       <- "frontend::check(root)?"                  @ [full-arm]
```

**为什么必须有「区域」这一层**：只要求「marker 在文件里逐字存在」，`marker` 就可以填
**arm 自己**的文本（`["check", "--secrets"]`），从而把「本机门控真的跑它」偷换成
「这个命令自己存在」——**自我满足**的典型形态。区域把判定域收紧到**门控闭包**内，
堵住这条假阳性路径。

**如实披露**（写进报告）：`check --profile portable` 的 marker `engineering_checks(root)?`
同时出现在 `full` 的 arm 里 ⇒ 判据验的是「该调用点在门控区域内逐字存在」，**不**验
「它专属于 portable」。这是**假阴性方向**的宽松（不会把问题掩盖成绿的），与 W92 的
「假阳性与假阴性不对称 ⇒ 永远选安全方向」一致。`windows-local` 取了它那条 arm **独有**的
落点（`"windows-local requires a native Windows environment"`），所以五条里只有一条是共享的。

### 决定 4：`standalone` 的成因取自**闭集**，域外 ⇒ 具名失败

| 成因 | 含义 | 实例 | 依据 |
| --- | --- | --- | --- |
| `host-subprocess` | 需要派生子进程，宿主对该步 `EBUSY` | `schema --check` | W95 同族成因；CI 里跑得到、本机 `EBUSY` |
| `requires-compiled-artifact` | 需要 protoc／编译产物 | `test --suite contracts` | `descriptor.test.mjs` 要 protoc |
| `superseded-by-workspace-run` | 门控跑的是**全 workspace** 测试，覆盖同一批包，但**不走这个入口** | `test --suite ×8` | `engineering_checks` 里是 `run(root, &["test","--workspace","--locked","--exclude",HARNESS])` |
| `deliberate-on-demand` | 有意按需，**不并入**常规门控 | `check --golden`、`check --secrets` | ADR 0043：「`cargo xtask sbom` 是生成命令，与 `check --secrets` 同类，**暂不并入 `engineering_checks`**：portable 门控须保持「只需 Rust 工具链」不变」 |

最后一个成因有**文档依据**（ADR 0043 第 116 行、`docs/provider-live.md`），不是凭空的标签。
`superseded-by-workspace-run` 是本轮**新增**的成因——它记录一个真实且容易误判的事实：
这 8 条命令不是「跑不了」，而是「**门控跑的是它们的超集**」。

**如实披露**：判据只验成因**落在闭集里**，**不验成因是否成立**（那是语义，仍由人读）。

### 决定 5：四类闭集，触发条件只由「字段存在」决定（W83）

- `gate` ⇒ MUST 带 `evidence`；`standalone` ⇒ MUST 带 `reason`、MUST NOT 带 `evidence`；
- `artifact` / `help` ⇒ MUST NOT 带 `evidence` 与 `reason`。

若允许 `standalone` 也带 `evidence`，「它是不是在本机门控里跑」就没有单一回答（W88 的
「谁在管它」）。

### 决定 6：解析不可得 ⇒ **具名失败**，不静默降级

`dispatchSignatures` 找不到 `pub fn execute(` 或括号不平衡 ⇒ 抛；`usageSignatures` 找不到
`Usage: cargo xtask ` ⇒ 抛。判据把它转成**一条具名错误**并返回，MUST NOT 静默返回空错误集
——那会把「读不懂」伪装成「没问题」。同理：dispatch 为空、usage 为空、某一类为空，
都必须**显式报错**（空态 MUST 被显式断言，W92 同族）。

## 四、与 W95 及 `test-reachability` 的边界（同一件事 MUST NOT 两处定义）

| 模块 | 断言的对象 | 独有覆盖 |
| --- | --- | --- |
| `test-reachability.mjs` (W84) | **测试文件**（`.test.mjs`）有没有被某个站点接上电 | 「仓库里的测试文件集 == 覆盖 ∪ 未覆盖台账」；**不管** CLI 命令 |
| `workflow-steps.mjs` (W95) | **CI workflow 的 step** 有没有被分类在册 | 「workflow 的 step 集 == 在册集」；**不管** CLI 命令 |
| `xtask-commands.mjs`（本轮） | **`execute()` 的 CLI 命令**有没有被分类在册，且 usage 与之对账 | 「dispatch 集 == 在册集」+「usage 与 dispatch 一致」；**不管** step 与测试文件 |

⇒ 删掉 W95 的 step 登记，本轮不会红；反之亦然。三者**不互相顶替**（报告 §五 逐条核对）。

## 五、注入清单

| 编号 | 注入 | 期望 |
| --- | --- | --- |
| F1 | 往 `execute()` 插入一条未登记的臂 | 红（具名该签名） |
| F2 | 从 `execute()` 删掉一条已登记的臂 | 红（具名该命令消失） |
| F3 | 把某条臂的字面量改掉（改名） | 红（新旧两侧都被点名） |
| F4 | usage 字符串里加一条不存在的命令 | 红（usage 声称了不存在的命令） |
| F5 | usage 字符串里删掉一条判定命令 | 红（该命令不在 usage 里） |
| F6 | `gate` 的 `evidence.marker` 改成不存在的串 | 红（假契约） |
| F7 | `gate` 的 `evidence.file` 改成没被注入的文件 | 红（缺输入必须具名） |
| F8 | `gate` 删掉 `evidence` | 红（空声明） |
| F9 | `standalone` 的 `reason` 改成域外值 | 红（域外成因） |
| F10 | `standalone` 挂上 `evidence` | 红（双向） |
| F11 | 清空整张登记表 | 红（闭世界两个方向都报） |
| F12 | 反向：合法登记原样 | **不**红 |
| P1 | 复现加法探针 | 红（改前绿） |
| P2 | 复现减法探针 | 红（改前绿） |
| P3 | 复现 usage 漂移探针 | 红（改前绿） |

每次注入后 `sha256` 逐字还原（`python` 写文件用 `newline=''`，W92 的 CRLF 假差异教训）。

## 六、明确不做

- **不验「CI 真的跑了某条命令」**：那要读 CI 的运行结果，不是静态判据的事。
- **不验 `standalone` 的成因是否成立**：语义，仍由人读（与 ADR 0054 决策 2 同族的边界）。
- **不验 `check --profile portable` 的证据是否专属于它**：见决定 3 的如实披露（假阴性方向）。
- **不改 `execute()` 与 usage 的任何一个字符**：本轮只补判据与登记，不动被审对象。
- **不把「本机能否跑」写进判据的输入**：分类是静态声明，MUST NOT 运行时探测（**铁律二**）
  ——`EBUSY` 是宿主事实，换一台机器会对同一个仓库得出不同分类。

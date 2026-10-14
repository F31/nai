# w101 设计

## 一、问题形状：**一条声明与它实际跑的对象的配对**没人读

W95/W97/W100 三轮补的都是**集合**：有哪些步、每个 suite 覆盖哪些包、CI 调用哪些命令、
每个步在哪些平台跑。**集合层对「配对」是瞎的**（W74 的那句话在这里第二次出现）：

```
在册条目   {host, identity, kind}          ← identity 是**名字**，不是目标
workflow    - name: API Provider transport and retry …
              run: cargo xtask test --suite provider
```

「这个名字的步」与「这条命令」之间**没有任何断言**。于是：

- 把两个 suite 步的目标**互换**（W1）⇒ 名字开始说谎，但覆盖集不变 ⇒ 没人看得见；
- 把某个 suite 步的目标**改成另一个 suite**（W2/W3）⇒ 那个 suite 的判定面**从 CI 静默消失**，
  同时另一个 suite 被跑两遍 ⇒ 没人看得见；
- 把一个判定步整个换成**另一条合法的判定命令**（W4）⇒ 性质全变 ⇒ 没人看得见。

这不是「漏了一个字段」，而是**从没被建立过的关系**。

## 二、证据（探针，改真文件 + 真跑 + 逐字节还原 + `sha256` 逐字一致）

9 个 portable 套件 = registry / schema / platform / workflow / xtask-commands / gate-coverage /
npm-scripts / tier-payment / step-config。基线 `229 passed / 0 failed`。

| 探针 | 改动（真改 `.github/workflows/foundation.yml`） | 失败向量 | 读它的人 |
| --- | --- | --- | --- |
| W0 | 基线 | `[0,0,0,0,0,0,0,0,0]` | — |
| W1 | `Plugin hosts and IPC` 与 `Session leases…` 的 `--suite` 互换 | `[0,…]` **零信号** | 无 |
| W2 | `Scheduling and shared budget ledger` 改跑 `--suite provider` | `[0,…]` **零信号** | 无 |
| W3 | `Knowledge algorithms…` 改跑 `--suite storage` | `[0,…]` **零信号** | 无 |
| W4 | `API Provider transport…` 改跑 `cargo xtask check --secrets` | `[0,…]` **零信号** | 无 |
| W5 | 门控步 `check --profile full` → `check --profile portable` | `gate-coverage=1` | W97 的**越界**断言（钉具体串） |
| W6 | `cargo xtask schema --check` → `cargo xtask schema` | `gate-coverage=1` | 同上 |

W1–W4 是**目标绑定**这一层真正的零信号向量；W5/W6 的红来自
`tests/contracts/gate-coverage.test.mjs` 里

```js
assert.ok(commands.includes('check --profile full') && commands.includes('schema --check'), …)
```

——一条**钉具体串**的断言。它同时是**假阳性源**（把 `full` 合法改成别的 profile 就红）
与**假阴性源**（W4 那种同形状替换它看不见）。它落在 W97 是因为 W97 的测试文件在那里，
但**声称的内容不属于 W97**：W97 自己的边界声明是「**不验签名的取值**」（W97 §八 2）。
按对象性质，这条声称属于 W101 ⇒ 本轮**就地订正**（活文档 = `tests/`，未发布历史记录才追加勘误段）。

### 三条探针方法论教训（本轮实测踩到）

1. **第一轮探针的红大多不是判据在响，是夹具锚点被扰动**（铁律一的反面）：
   `tests/contracts/workflow.test.mjs` 的「步消失」用例锚在
   `- name: Kernel registration and binding\n        run: cargo xtask test --suite kernel\n`
   这一整段上 —— 探针一改这条命令，`.replace()` 就**匹配不到** ⇒ 注入变 no-op ⇒
   用例因**前提被扰动**而红，看起来像「判据发现了目标被改」。**处置**：第二轮的干净探针
   只改**没有任何夹具锚定**的那些步（`hosts`/`session`/`scheduler`/`provider`/`knowledge`），
   于是得到真正的零信号向量 W1–W4。
2. **别把「身份被顺带改掉」当成「目标被改」**：无 `name` 的步（`- run: pnpm spec:validate`）
   其 `identity` 就是 `run` 首行 ⇒ 改它的 `run` 会**同时**触发 W95 的「未在册 + 已消失」两条，
   看上去像是目标绑定被守住了。**判据 MUST 分开测**：身份层（W95）与目标层（W101）。
3. **`nodes --test` 的失败向量要按套件读，不能只看总数**：同一处注入会让多个套件红，
   其中只有一部分是本轮要证的。**逐条读 `not ok` 与它的 assertion message**，
   才知道红的是「判据」「夹具前提」还是「上游级联」。

## 三、设计决定

### 决定 1：`target` 与 `kind` 住**同一个条目**，MUST NOT 另开一张表

一个 step 是**一个对象**：它的身份、类别、目标、凭据都是它的属性。W95 的
`WORKFLOW_STEPS` 已经是「步身份 ↔ 类别」的唯一在册处，闭世界（未在册 / 已消失）也长在它上面。
再开一张 `step → target` 的表就是**同一件事的第二处定义**：两张表会各自漏掉对方的新增。
⇒ `target` 是本条目上的**新字段**，不是新表。（对照 W88：「加一个必填字段」而不是「加一张表」。）

### 决定 2：`target` 钉**取值**，这与 W96「签名不含值」不冲突——两者的**对象不同**

W96 说的是「`dispatchSignatures` 描述**入口的形状**」：`check --profile <P>` 是一条签名，
`<P>` 取值不进去，因为那张表管的是「命令面有哪些入口」。
W101 问的是**另一个问题**：「**这一步实际调用的是哪一个入口实例**」——它的答案里
**必须**含 `full`，否则无法把「跑 `full`」与「跑 `portable`」分开。
⇒ 同一条命令在两个不同的判定域里被问了两个不同的问题，两个答案都必要，**不是重复定义**。

### 决定 3：xtask 那一支**复用** W97 的 `ciXtaskCommands`，MUST NOT 写第二份剥壳逻辑

「一段文本里有哪些 `cargo xtask …` 命令」这件事已经有第一处定义（W97），
它已经处理了 `2>&1`、`| tee`、`&&`、`;`、行续符。第二份实现一定会漂移
（`set -o pipefail` 那一行的 `| tee` 就在门控步里）。⇒ `targetsOf` 对 xtask 形状
**调用** `ciXtaskCommands`（逐段 `run` 传入），只负责**分类**与**其余形状**。

### 决定 4：形状闭集 `TARGET_SHAPES = ['xtask', 'node-test', 'pnpm']`，MUST NOT 认「其他」

闭集让「未知形状」**具名失败**（W90）。三个取值由**实测**得出，每个都有活实例：

| 形状 | 识别 | 活实例 |
| --- | --- | --- |
| `xtask` | `cargo xtask <签名>` → 签名原样 | 10（门控 1 + suite 7 + 契约套件 1 + `schema --check` 1） |
| `node-test` | `node --test <文件…>` → 规范化文件列表 | 1 |
| `pnpm` | `pnpm <脚本>`，**`pnpm install` 除外** | 2 |

`pnpm install --frozen-lockfile` **不算目标**：它是环境准备（`Install frontend and OpenSpec
dependencies` 是 `infra`）。照 W95 的先例，这条排除与 `JUDGE_COMMAND_SHAPES` 的
`/^pnpm\s+(?!install\b)/` **同一条纪律**，但对象不同（W95 判「是不是判定命令」，本轮判「目标是什么」）。

### 决定 5：`infra` / `diagnostic` **双向**约束——不声明目标，且**解析出零个目标**

单向（「infra 不许写 target」）是**装饰**：真正的后门是「infra 的 `run` 里塞一条判定」。
W95 已经有一条这样的检查，但它**只看首行**、只认 **5 个正则**（`cargo xtask` / `node --test` /
`node --experimental-strip-types` / `pnpm exec` / `pnpm <非 install>`）。
⇒ 本轮从**解析**侧补：整段 `run` 解析出的目标集合 MUST 为空。
这条比 W95 那条**严格**（整段 vs 首行），两者**对象不同**：W95 问「这条命令像不像判定」，
W101 问「它解析出的目标集合是不是空的」。

### 决定 6：对账用**集合双向相等**，MUST NOT 用「声明 ⊆ 解析」单向

- 解析出而没声明（W1/W4）⇒ 目标被**悄悄换掉**；
- 声明了而没解析出（把 `run` 清空）⇒ 凭据是**空话**。
两个方向都具名。集合元素用 `shape:value` 的**规范串**，排序后比较——顺序不是声明的一部分。

### 决定 7：判分由**在册条目**驱动，MUST NOT 由 `run` 文本里的名字驱动

把某条声明的 `target` 改掉时，判据 MUST 红——而不是去猜「这条命令看起来像什么」。
`target` 的**取值域**由实测决定（W93），不是由「什么看起来合理」决定。

### 决定 8：缺输入 / 不可解析 / 空注册表 ⇒ **具名失败**，MUST NOT 静默降级

`workflowText` 非字符串或为空 ⇒ 返回一条具名错误（不是 `[]`）。
注册表为空 ⇒ 报「空注册表让闭世界恒真」。不可解析由 W95 的解析层抛 ⇒ 本轮**不重复**解析，
只在 `workflowStepsOf` 抛时把它转成一条具名错误。

## 四、边界（与相邻判据互不顶替；实测数字见报告 §五）

| 相邻判据 | 它问的问题 | W101 问的问题 | 是否顶替 |
| --- | --- | --- | --- |
| W95 步层闭世界 | 这一步**在不在册**、`kind` 是什么、`local` 有没有落点 | 这一步**跑哪个目标** | 否（W1–W4 对 W95 全绿） |
| W96 命令面 | 命令面**有哪些入口**（签名，不含值） | 这一步调用**哪一个入口实例** | 否（取值 vs 形状，见决定 2） |
| W97 包集合 | 某个 suite **覆盖哪些包** | 某条 CI 命令**是不是** `--suite X` | 否（W4 对 W97 全绿） |
| W99 站点层级 | 某站点**最低在哪一层跑**（兑付） | 哪条命令**是**这一步的目标 | 否 |
| W100 执行配置 | 跑不跑 / 在哪跑 / 失败算不算数 | 跑的是**什么** | 否（正交面） |

**与 W97 的重叠点是本轮要治理的对象**：W97 那条 `includes(...)` 断言与本轮 R1 在
「门控步跑 `check --profile full`」这一件事上重叠 ⇒ 按「同一件事 MUST NOT 两处定义」，
把 W97 的那条**退回形状级**（「至少有一条 `check --profile …`」），取值绑定只留 W101 一处。

## 五、非空洞

- 闭集 `TARGET_SHAPES` 非空，且**每个值都必须有活实例**（实测 10/1/2）——本轮**钉实例数**吗？
  **不钉**（W92 教训：一次合法的收敛会让判据变红）。只要求**闭集非空** + 每个**出现的**
  `shape` ∈ 闭集 + 真实仓库里三个形状**当前都有实例**这一事实**如实记录**在报告里，
  由测试用**合成世界**分别覆盖三条解析规则（合成世界可以任意构造，不受真实仓库约束）。
- 每个分支（R1 两个方向 / R2 两个方向 / R3 域外 / 缺输入 / 空注册表）都有注入用例。
- 反向：一条**合法**的注册（换成一个同样真的存在、且被声明了的目标）MUST NOT 红。

## 六、明确不做 / 如实披露

1. **不验目标能跑通**：那要执行（`cargo`/`pnpm` 在宿主上的可用性），且铁律二说宿主瞬态
   MUST NOT 改变结论。
2. **不实现 shell 语义**：变量展开（`${{ matrix.os }}`）、条件分支、函数体一概不解析。
   ⇒ `run` 里 `if`/`for` 包裹的命令会被当成**无条件目标**（假阳性方向，安全）。
3. **不剥 shell 注释**：`run` 块里的 `# cargo xtask …` 会被当成一个目标（W97 的既有边界，
   本轮沿用）。假阳性方向，安全。记入 §八。
4. **不判目标在矩阵各腿上的等价性**：那是 W100 的执行配置面。
5. **不判 `run` 里的第二条／第三条命令是否也该在册**：`targetsOf` 会**全部**解析出来，
   因此多出来的会被 R1 的「解析出而没声明」抓住——这是**有意的**（要求人显式归类）。

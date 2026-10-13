# w98 设计

## 一、问题形状

「npm 脚本」是一层**双面**对象：一面是**声明**（`package.json` 的 `scripts` 表，可枚举、可读 JSON），
另一面是**执行面**（谁调用它——CI 的 `run:` 行、驱动表的一项、文档里给人抄的一行）。当前只有第一面
有形状，第二面**完全不在任何判据里**。

```
声明面（可枚举）                        执行面（谁调用它？—— 无人回答）
root/package.json
  ├─ typecheck        = pnpm --recursive typecheck        ← 被谁调用？ 无人调用；它自己是一个"递归委托"
  ├─ build            = pnpm --recursive build            ← 同上
  ├─ test:ui          = node --test tests/ui/*.test.mjs   ← frontend-gate.mjs 的 STEPS 里有一项('.', 'test:ui')
  ├─ check:frontend   = node scripts/frontend-gate.mjs    ← W41 报告说"便于人工单独调用"——散文，不是判据
  ├─ spec:validate    = openspec validate --all --strict  ← CI 的 run: 行
  ├─ test:control-smoke = cargo build … && node …         ← 只在 docs/control.md 的代码块里
  └─ test:engine-smoke  = cargo build … && node …         ← 只在 docs/engine.md 的代码块里
apps/desktop/package.json
  ├─ dev              = vite --host 127.0.0.1 …           ← 只在文档里（人工）
  ├─ typecheck / build / test                             ← STEPS 里各有 1 项
packages/contracts-ts/package.json
  └─ typecheck / build                                    ← STEPS 里各有 1 项
```

三个陷阱在这一层同时出现：

1. **同名≠同一个**（W91）：`typecheck` 在三个包里同名，`build` 在两个包里同名
   ⇒ 判定单元 MUST 是 `(包目录, 脚本名)`，MUST NOT 是裸脚本名。
2. **注释/散文不是引用**（W97 的 `stripRustComments` 教训，换一层）：`workflow-steps.mjs` 的文档注释里
   写着 `pnpm spec:validate`、`tests/registry.yaml` 的 `notes` 里写着 `pnpm typecheck/build`——
   它们是**解释**，不是**调用**。实测：原始正则命中 4 行，剥注释与识别散文后**只有 1 行**是真引用。
3. **文档不是调用**：`.md` 代码块里的 `pnpm test:control-smoke` 是给**人**抄的，它不**执行**任何东西。
   把文档当消费者，等于给一个「从不运行」的脚本发一张「有人在跑它」的证书。

## 二、证据（三个 Mutation，改真文件 + 真跑 + 备份拷回 + `sha256` 逐字还原）

**.baseline（13 条脚本 = 3 包）**：`invoked` **7** 条、`orphan` **6** 条，全部有据可查（见 §三）。

| Mutation | 改动 | 现有 6 个 portable 套件的结果 |
| --- | --- | --- |
| **M2** | 根 `package.json` 加 `"typecheck:fast": "node scripts/nowhere.mjs"` | **158 passed / 0 failed——零信号** |
| **M3** | `frontend-gate.mjs` 的 `STEPS` 里 `['apps/desktop', 'test']` → `'tests'` | **158 passed / 0 failed——零信号** |
| **M5** | 根 `package.json` 加 `"test:loop": "pnpm test:loop"`（自引用） | **158 passed / 0 failed——零信号** |
| M1（反例） | CI 里 `pnpm spec:validate` → `spec:validat` | **2 failed**——W95 的 `localEvidence` marker 逐字比对抓到了 |

M1 是**如实记录的反例**：W95 抓的是「**那一行文本变了**」，不是「**那个脚本没有消费者了**」。
同一处文本、两种语义。⇒ 本轮的判据不与它重叠（§四边界实测）。

## 三、设计决定

### 决定 1：判定单元是 `(包目录, 脚本名)` 的配对

`packageFiles` 的键是**包目录**（`.` / `packages/contracts-ts` / `apps/desktop`），台账的键是
`dir\0script`。裸脚本名不作键——`typecheck` 三条互相独立，删掉其中一条 MUST 只点名那一条。

### 决定 2：三态处置 + 四格成因闭集

```
disposition = 'invoked'  → invocation: {file, marker}
disposition = 'orphan'   → cause: <闭集之一> + 该格自己的代价
```

| 成因 | 含义 | 代价（MUST 逐字/逐项成立） |
| --- | --- | --- |
| `covered-by-driver` | 被**驱动表**的等价步骤覆盖（根包的递归委托是多余的） | `coveredBy: [{dir, script}]`，每项 MUST 是**一条真的 STEPS 项**；且脚本体 MUST 是 `pnpm --recursive <X>`，每项的 `script` MUST 同为 `<X>` |
| `manual-smoke` | 人手冒烟，门控不跑 | `docEvidence: {file, marker}`，marker 在文档里逐字存在 |
| `manual-alias` | 人手别名，指向同一个入口 | `equivalentTo: {file, marker}`，marker 在**源码**里逐字存在 |
| `interactive` | 交互式（开发服务器），门控**不能**跑它 | `docEvidence: {file, marker}`——**门控执行不了的东西，其可用性只能由文档背书** |

`covered-by-driver` 的「脚本名一致」这条不是装饰：根包的 `typecheck` 体是 `pnpm --recursive typecheck`，
它覆盖的是**所有包的 `typecheck`**，不覆盖 `build`。若台账写成 `build` 项，判据 MUST 红
——这正是不让「委托」变成一张万能通行证。

### 决定 3：每种证据的**判定域由解析得出**，不是文件全文

| 处置 | 判定域 |
| --- | --- |
| `invoked` | **剥离注释后的整个引用扫描面**（CI workflow + `scripts/**/*.mjs`），marker 逐字存在 |
| `covered-by-driver` | **`driverSteps()` 解析出的 STEPS 项集合**（marker 必须真的是其中一项，不是文件里随便一段文本） |
| `manual-alias` | 指定文件的**剥离注释后**文本 |
| `manual-smoke` / `interactive` | 指定**文档**的文本 |

这是 W96「门控闭包区域」教训的第三次复用：**marker 可填自己的文本 ⇒ 自我满足**。所以
`covered-by-driver` 的判定域 MUST 是解析出来的结构，不是一个 `.includes()` 全文比对。

### 决定 4：**文档不算调用**（本轮判据的核心分离）

`.md` **不在**引用扫描面里。`docs/control.md` 与 `docs/engine.md` 里的那两行只作为
`orphan` 的证据形态出现。理由：文档是**给读者的指引**，它不能被「写错」以外的任何机制证伪
——把文档当消费者，两个从不运行的脚本会永久绿灯。

### 决定 5：自引用 MUST NOT 充当消费者

`"test:loop": "pnpm test:loop"` 是一条**真**的 `pnpm` 引用，但它**指向自己**。判据 MUST 排除
「引用点落在被消费对象自己的定义里」。M5 实测：当前工艺对它是**零信号**。

### 决定 6：把 `frontend-gate.mjs` 的**运行时**拒绝**静态化**

`frontend-gate.mjs` 的 `planStep` 会在运行时拒绝未知可执行名与未实现的 shell 语法。这是**好**设计，
但它是**运行时**的：只有真跑 `check --profile full` 才知道。M3 实测（把 STEPS 里一个 script 名改错）
在静态判据下**零信号**。

⇒ 静态判据 MUST 对 `STEPS` 的每一项复算同一件事：
- `(dir, script)` MUST 落在该包 `scripts` 里；
- 体的第一个 token MUST 在 `KNOWN_EXECUTABLES`（由 `frontend-gate.mjs` **解析**得出，不手写）；
- 体里 MUST NOT 出现 `SHELL_SYNTAX` 里的任一符号。

**判定域**（W91：「每个限定 MUST 用反例验证它在拦什么」）：这条只对 **`STEPS` 里出现的 6 项**成立。
根包其余的脚本头 token 是 `pnpm` / `openspec` / `cargo`，它们**本来就不经** `frontend-gate.mjs`
⇒ 判它们等于用一把错的尺子。测试用例里带这个反例。

### 决定 7：闭世界双向 + 成因闭集非空

`packageFiles` 里出现的每个 `(dir, script)` ↔ `SCRIPT_DISPOSITIONS` 的键集，**双向**一致：
① 一个脚本不在册 ⇒ 它**没有任何处置**（M2 的守卫）；② 在册的脚本不在 `package.json` 里 ⇒ 一条
**静默消失的声明**（删脚本无人发现）。

`ORPHAN_CAUSES` MUST 至少被用到一格（空闭集是「没有断言」的伪装）。**不**要求每格都有实例
——那会让一次合法的成因收敛（例如某个冒烟脚本被接进门控后 `manual-smoke` 归零）反而变红
（W92 的教训），代价如实登记在报告 §八。

## 四、边界（实测互不顶替）

| 删掉 | 本模块 | W95 workflow | W96 xtask-commands | W97 gate-coverage | W84 registry |
| --- | --- | --- | --- | --- | --- |
| `SCRIPT_DISPOSITIONS` 一条 | **红** | 0 | 0 | 0 | 0 |
| `WORKFLOW_STEPS` 一条 | 0 | **红** | 0 | 0 | 0 |
| `XTASK_COMMANDS` 一条 | 0 | 0 | **红** | 0 | 0 |
| 新站点 | 0 | 0 | 0 | 0 | **红** |

（数字见报告 §五；M1 的表已揭示 W95 与本模块的**唯一交叠点**是 `foundation.yml` 的那行文本
——两者**语义不同**：W95 管「step 还在不在」，本模块管「脚本还有没有消费者」。）

## 五、非空洞

- 闭世界**非空**被断言（13 条脚本、4 格成因里至少一格被用到）。
- **每类处置 MUST 有独有覆盖**：`invoked` 的证据只能由真引用付；`covered-by-driver` 只能由 STEPS 项付；
  两者的判定域不相交 ⇒ 一条证据不可能同时给两格签名。
- 每条判据都有**注入用例**（P 级 + 测试文件内的 F 级），且注入 MUST 具名失败、MUST NOT 静默降级。
- 缺输入 MUST 具名（`packageFiles` 少一个包、`driverText` 未提供 ⇒ 报错，不是「跳过」）。

## 六、明确不做 / 如实披露

1. **不验脚本能跑通**：那要执行（`pnpm` 在宿主上不可用），且**铁律二**说宿主瞬态 MUST NOT 改变结论。
2. **不验 `pnpm` 引用的执行可达性**：CI 里的 `if:` 条件、`continue-on-error` 一概不判。
3. **`--filter P` 形态在引用扫描面里实例数为 0**：`pnpm --filter @nai/desktop test` 与
   `pnpm --filter @nai/desktop dev` 都只在 `docs/**`（不在扫描面）⇒ V2 的这条分支当前**没有活样本**，
   证据只有测试文件里的注入用例。**如实披露，不写成「已覆盖」**。
4. **成因闭集不要求每格有实例**（见决定 7 末段）。
5. **不改任何被审对象**：`package.json`、`frontend-gate.mjs`、workflow 与被审脚本一个字符不动。
6. W90 §八遗留 1/3/4/5、W91 跨语言键级消费、W92 基线收录、W93 锚点唯一性、W94 `needs_external` 审计、
   W95 `ci-only` 成因、W96 五项、W97 六项遗留**均未动**。

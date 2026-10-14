# w100：在册判定步的**执行配置面**（`matrix` × `if:` × `continue-on-error`）必须由判据读出

## Why

W95 给 `.github/workflows/foundation.yml` 的 step 集补了**步层闭世界**：步的身份由解析得出、
与在册清单双向一致、`kind` 落在闭集里、`local` 要引出真的门控落点、`ci-only` 要给出成因、
`diagnostic` 必须由 `if: failure…` 认出。W97/W98/W99 又分别管住了包集合、脚本消费者、站点层级。

但每个步的**执行配置**——**跑不跑**（`if:`）、**在哪个平台跑**（`matrix`）、
**失败了会不会被吞**（`continue-on-error`）——**从来没有被任何判据读过**：

```
workflowStepsOf(text) → [{identity, condition, run}]   ← 解析器只产出这三个字段
                        ↑ condition 只被 diagnostic 分支读一次：condition.includes('failure')
matrix / continue-on-error                              ← 解析器根本不产出，判据无从读起
```

⇒ 母题「**声明 MUST 付代价**」的第九次出现（`repositoryFiles` → `readTrackedText`
→ W94 两张事实表 → W95 `gateSources` → W96 门控闭包区域 → W97 包集合 → W98 npm 脚本执行面
→ W99 站点层级声明 → **本轮：判定步的执行配置**）。

实测（探针 P-A1…P-A6、P-B1，改**真文件**、真跑 8 个 portable 套件、逐字节还原、
`sha256` 逐字一致）：

| 探针 | 改动 | 失败数（8 套件） | 读它的人 |
| --- | --- | --- | --- |
| P-A0 | 基线 | `[0,0,0,0,0,0,0,0]` | — |
| P-A1 | Linux-only 步的 `if: runner.os == 'Linux'` → `'FreeBSD'` | `[0,…]` **零信号** | 无 |
| P-A2 | `matrix.os: [ubuntu-24.04, windows-latest]` → `[windows-latest]` | `[0,…]` **零信号** | 无 |
| P-A3 | `local` 门控步加 `continue-on-error: true` | `[0,…]` **零信号** | 无 |
| P-A4 | 删掉 `Install Linux C++ protobuf runtime` 的平台条件 | `[0,…]` **零信号** | 无 |
| P-A6 | `local` provider 步加 `continue-on-error: true` | `[0,…]` **零信号** | 无 |
| P-B1 | 判定步的 `run` 换成 `cargo xtask check --registry`（标签不变） | `[0,…]` **零信号** | 形状级（W97 V6） |
| P-A5 | 删掉诊断步的 `if: failure()`（**对照组**） | `workflow` 套件 `2` | W95 已兑付 |

P-A2 最能说明问题：矩阵缩成一个平台后，**两条 Linux-only 步永远不会跑**
（其中一条就是三语言契约套件），而「21 个 step 都在册、身份都在、`kind` 都在闭集里」
——**每一格都还是绿的**，CI 也照样绿。这不是「有一个洞」，而是**一整个面**没人读。

## What Changes

1. **`workflowStepsOf` 升级为两个视图**（**单一解析器**，MUST NOT 解析两遍）：
   - `workflowJobsOf(text)`：`{id, matrix, runsOn, steps}`；
   - `workflowStepsOf(text)`：在原有 `{identity, condition, run}` 上**增补** `matrix` 与
     `continueOnError`（同 job 的步共享同一份 `matrix` 对象）。
   W95 既有的三个字段语义**一字不动**，既有用例全部不变。
2. **新增 `scripts/contracts/step-config.mjs`**：`stepConfigErrors({workflowText, registered})`，
   五条判据：
   - **R1 可达性**：每个在册步的**执行平台集合**由 `matrix` × `if:` **静态求值**得出，MUST 非空。
     一条「永不跑」的步 ⇒ 具名失败——**一个不跑的判定就是一个静默消失的判定**（P-A2 的守卫）。
   - **R2 失败语义与角色一致**：`local` / `ci-only`（**产生判定**的）步 MUST NOT 带 truthy
     `continue-on-error`——能失败而不影响 job 结论的判定是**装饰**（P-A3/P-A6 的守卫）；
     反过来 `diagnostic` 步 MUST 带 truthy `continue-on-error`——诊断自身的问题
     MUST NOT **替换**原始失败的结论（P-A5 之外的另一半）。
   - **R3 成因与条件一致**：`ci-only` 的 `platform-toolchain` MUST 被平台条件兑付
     （执行平台集合是矩阵的**真子集**）；其余三因（`host-subprocess` / `compiled-artifact` /
     `not-in-profile`）MUST NOT 被平台限定——它们与平台无关，限定会让该步在别的平台**静默不跑**。
   - **R4 配置面闭世界**：`matrix` 形状（唯一轴 `os`，且经 `runs-on: ${{ matrix.os }}` 消费）、
     `if:` 形状（∈ 条件形状表）、`continue-on-error` 形状（布尔字面量）、runner 标签与
     `runner.os` 取值（∈ 各自的闭集）——**域外 ⇒ 具名失败，MUST NOT 静默降级**（P-A1 的守卫）。
   - **R5 平台专属命令 MUST 被平台条件兑付**：`run` 文本命中平台专属命令形状
     （`sudo` ⇒ 需 POSIX；`apt-get` ⇒ 仅 Linux）⇒ 执行平台集合 MUST ⊆ 该形状支持集且非空
     （P-A4 的守卫）。
3. **新增 `tests/contracts/step-config.test.mjs`**：合成世界 + 真实仓库 + **逐条注入** + 非空洞 +
   反向不红，并登记进 `xtask/src/lib.rs::engineering_checks` 与 `test-reachability.mjs`
   的站点表（站点 12 → 13）。
4. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。

## Impact

- **零产品代码改动**：`.github/workflows/foundation.yml` **一个字符都不动**（本轮只加判据）；
  `WORKFLOW_STEPS` 的既有 **21 条**一个字符都不动；`workflowStepsOf` 既有三字段语义不变。
  零新依赖。
- Node 套件由 **10 → 11**；登记站点由 **12 → 13**；`engineering_checks` 多一行。
- **明确不做**：不验测试真的能跑通；不实现 GitHub 的 `if:` 表达式求值语义（只做**闭形状**静态分类）；
  不判 `infra` 步是否真的把环境装好；不判 `matrix` 的 `fail-fast` / `timeout-minutes` / `env`。
- **如实披露**：`if:` 的静态求值是**形状级**的——条件形状表只认 `runner.os == '<os>'` 与
  `failure()`，其余形状（`always()`、`github.event_name == …`、`!cancelled()` …）一律**具名拒绝**。
  这是有意的保守：拒否方向落在**假阳性**（要求人显式归类）而非假阴性。记入报告 §八。

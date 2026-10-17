# Proposal: W134 — CI 上同一批测试被跑两遍：把「重复」补成一条可判的缺陷

## Why

本轮的起点不是 `tests/registry.yaml` 的某条验收条目，而是上一轮梳理报告 §5.2 里那句没落地的话：
**CI 的 windows 腿要 21–29 分钟**，每轮观测窗口被拖到 30 分钟以上。开工前先量，结论不在「CI 太慢」，
而在**它做了同一件事两遍**：

```
foundation.yml（每个平台腿）
  ├─ cargo xtask check --profile full      ← engineering_checks 里已有
  │                                            cargo test --workspace --exclude nai-xtask
  └─ cargo xtask test --suite storage      ← 包集 {nai-storage, nai-security} ⊂ 上面那次
     … kernel / hosts / session / scheduler / provider / knowledge 共 7 条，逐一同理
```

**本机暖缓存实测**（`cargo test --workspace --locked --exclude nai-xtask` 与 7 条 suite 逐个计时，
只计执行、不含编译）：

| 步 | 秒 |
|---|---|
| storage | 125 |
| kernel | 108 |
| scheduler | 89 |
| session | 77 |
| hosts | 43 |
| knowledge | 40 |
| provider | 35 |
| **合计** | **517** |

517 秒是**第二遍**——它们跑的东西门控步刚刚跑完。CI runner 只会更慢。

真正值得写下来的是：**仓库里四层判据都在，四层都不问这件事**。
W95 步集闭世界管「这一步在不在册」；W97 覆盖闭包管「门控与 suite 的包集合有没有漏」；
W100 执行配置管「这一步会不会跑、能不能被吞」；W101 目标绑定管「这一步跑的是不是它自称的目标」。
四条都正确，四条都对**「两个目标覆盖的是不是同一批测试」**沉默。

## What Changes

1. **删掉 7 条重复的分套件步**（`foundation.yml`）+ 名册里对应的 7 条（`WORKFLOW_STEPS` 21 → 14 条）。
   它们的包集**全部**包含在 `check --profile full` 的 workspace 全量运行里 ⇒ 不损失任何判定，
   只损失每个平台腿 517 秒。归因（哪套件红了）不受影响：门控日志仍经 `.nai-diagnostics/`
   由 `ci-diagnose.mjs` 转成匿名可读的 annotation。
2. **新增判据**（`scripts/contracts/step-overlap.mjs` + `tests/contracts/step-overlap.test.mjs`）：
   同一 workflow 里，两个判定步的**测试覆盖**一旦互相包含（含相等）⇒ 具名报错，并说出**是哪两条步**、
   **哪个包集**。包集 MUST 从 `xtask/src/lib.rs` 的 dispatch 臂与门控区域**算**出来，
   MUST NOT 在表里写第二份（W93）。覆盖类别是闭集（`gate` / `suite` / `non-test-target`），
   域外取值具名失败（W90）。
3. **`suite` 类里的两种情形 MUST 分开**：体是一条 `["test", …]` ⇒ 有包集、参与包含关系；
   体交给别的函数（`contracts` 走 `schema::contracts`，跑 C++ 夹具）⇒ **没有** cargo 包集、不参与。
   后者返回 `null` 而不是空集——空集会让包含关系对空集恒真（假红），`null` 是**类别事实**。
4. **夹具锚点重新勘定**：一批既有判据把 7 条被删步的名字当作 `replace` 锚点。删步之后那些注入变成
   **静默 no-op**（`replace` 命中不到 ⇒ 注入没发生 ⇒ 「注入后应当红」的用例反过来全红）。
   本轮把它们改锚到**单行 `- name:`** 的幸存步，并给三处补上「锚点 MUST 真的命中」的现测断言。
5. **刻意不做**：
   - **不拆 job、不把平台无关的判定从 windows 撤掉**。`step-config.mjs` 的 R3 明文禁止
     「非平台相关的 `ci-only` 判定被平台条件收窄」（那等于让它**静默跳过**另一个平台），
     要那么做得先改模型与 ADR，属于另一轮，已具名登记。
   - **不加编译缓存**。本机 `target/` 已是 18G，超过仓库的缓存配额上限；缓存也不是「少做一遍」，
     而是「做得更快」，与本轮的判据无关。
   - **不动任何 Rust 判定面与执行体**。本轮改的是**执行面**（workflow + 名册 + 判据），
     `nai-kernel` / `nai-runtime` / `nai-storage` 一行未改。

## 具名登记（本轮**发现但不修**）

1. **windows 仍要跑与 Linux 完全相同的门控**（前端 + fmt + clippy + 全量测试）。撤掉它需要
   把 `check --profile full` 拆成「平台无关」与「平台相关」两个 profile，并同步改
   `step-config.mjs` 的 R3 双向规则与 `tier-payment` 的 `ci` 面——那是模型变更，MUST NOT 顺手做。
2. **归因粒度下降**：分套件步删掉后，CI 上的失败要读门控日志才知道是哪套件红的。
   `ci-diagnose.mjs` 的注解仍给出失败摘要，但**不再有「红色出现在哪个 suite 步」这一层**。
   如实登记，MUST NOT 说成「零损失」。
3. **`ciXtaskCommands` 不剥 YAML 注释**（本轮实测）：注释里写 `cargo` + `xtask` 连写会被当成一条
   CI 命令，报「调了不存在的命令」。本轮只是**避开**这个形状（注释里不连写），
   判据本身的这个洞**未修**。
4. **`test --suite` 的目标仍可漂移**：本轮钉的是「两条步的覆盖不许互相包含」，
   **不钉**「某条 suite 步 MUST 存在」——`upgrade` 套件本来就没有 CI 步，
   本轮之后所有 cargo 套件都不再有独立的 CI 步。

## Modified Capabilities

- `engineering-foundation`：**ADDED** 三条 Requirement（覆盖关系不得重复 / 覆盖由执行体算出 /
  每个判定目标归入闭集类别）。刻意**不 MODIFIED** 既有的
  `Reproducible workspace` 与 `Enforced dependency boundaries`——它们声明的是**工程入口**，
  本轮新增的三条声明的是**执行面上重复的不可见性**，是另一件事
  （「一条 Requirement 只声明一条行为」）。

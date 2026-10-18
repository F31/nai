# W147 报告：预构建处方必须与门控测试步同批单元

- 变更：`openspec/changes/w147-prebuild-recipe/`
- ADR：[0121](../adr/0121-prebuild-recipe-unit-set.md)
- 起点：W146 §八第 5 条（「预构建再跑门控只在不带 clippy 的回路里有效」）
- 与 W146 的关系：**订正它的归因，不改它的结论**。W146 报告已发布，故勘误追加在该文 §九，不就地改。

## 一、勘定与现测

W146 那条成因是「门控先跑 `clippy`，clippy 与 rustc 产物指纹不同 ⇒ `cargo test` 阶段重新链接，
预构建成果在门控内部被作废」。它会成为下一次会话的指令，所以先验它。三组实测
（本机，`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1`，数 `Compiling` 行）：

| 组 | 序列 | `Compiling` | 判读 |
| --- | --- | --- | --- |
| M | `build --workspace --all-targets` → `clippy --workspace --all-targets -- -D warnings` → `build --workspace --all-targets` | 8 → **0** → **0** | clippy 不动 rustc 产物 |
| A | 改一个 `.rs` 后 `build -p nai-xtask --lib` → `clippy -p nai-xtask --lib` → `build -p nai-xtask --lib` | 1 → 1 → **0** | 同上，且 clippy 真编了东西 |
| B | `build --workspace --all-targets` → `test --workspace --exclude nai-xtask --locked --no-run` → `build --workspace --all-targets` | 1 → **7**（`LNK1104` **0** 次）→ **7** | 两套单元集互相判脏 |

B 组那 7 个包两次是**同一批**（render-worker / output-keeper / engine / provider-core / control-api /
media-core / contracts，**不含** xtask）⇒ 双向互不包含，不是「顺带多编一个」。

顺带量了 `LNK1104` 的宿主候选面（**只为把候选面收窄，不当作结论**）：
`Get-MpComputerStatus` ⇒ `RealTimeProtectionEnabled = False`、`AntivirusEnabled = False`；
`Get-MpPreference` 报 `0x800106ba`（服务未启用）；`root/SecurityCenter2` 注册的杀软是
Windows Defender（`productState 393472`）+ McAfee（`266240`）；进程侧 `mc-fw-host`×2、`mc-neo-host`
在跑，服务 `McAfee WebAdvisor` Running（`mc-wps-*` 全 Stopped）；`tasklist` 里没有任何
`nai_*` / `pipeline` / `engine` 残留进程；E: 为 NTFS、Healthy。⇒ Defender 不能解释，McAfee 是**唯一**
在册候选。

CI 侧：`.github/workflows/foundation.yml` 里 `cache` / `swatinem` / `rust-cache` **零命中** ⇒
每次都是冷跑，今天 CI 实际编译两遍（clippy 一遍 + test 一遍）。

## 二、成因替换

真因不是 clippy，是**单元集不对齐**：`cargo build --all-targets` 展开为
`--lib --bins --tests --benches --examples`，其中 `--tests` **只类型检查测试源码、不产 test harness**，
而 `cargo test` 要编译并**链接**测试二进制。W146 §六表里第 3、4 次 `LNK1104` 的目标
（`pipeline-*` / `engine-*` / `nai_engine-*` 的 bin test）恰好只存在于后者的单元集里 ⇒
用 `build --all-targets` 预构建，门控该链接的仍然没链接过，等于没有预构建。

这条替换的意义不在「更准确」，而在**处置完全不同**：把原因记成 clippy，处置就会落在
「改门控形状」（共用产物 / 把 `-D warnings` 挪走），而门控两条测试步的字面量正是
`xtask-commands.mjs` 里 `superseded-by-workspace-run` 的逐字凭据；新增一条 `xtask` 子命令又会打红
`xtask-commands.test.mjs` 里**逐字钉住的 24 条签名清单**。修一个不存在的成因，代价是两处判据。

## 三、判据形态

处方写在 `docs/engineering.md`（**活文档**）的 `<!-- gate-prebuild:begin/end -->` 标记块里：

```
cargo test --workspace --exclude nai-xtask --locked --no-run
cargo test --locked --package nai-xtask --lib --no-run
```

判据 `prebuildRecipeErrors()` 的三条形态决定：

1. **逐 token，不降维**。既有的 `coveragesOf` 会把命令降维成 `{kind, packages, excludes}`，
   `--locked` / `--lib` / `--no-run` 被**刻意**丢弃（对包集合它们确实无关）。这一层问的恰恰是被丢弃
   的成分，所以补了 `testCommandsOf`——并把 `coveragesOf` 改建在它上面，**不写第二套解析**。
2. **双向一一对应**。只判「文档每条都在门控里」漏掉「门控某条测试步没有对应命令」，
   那正是处方会静默变旧的形态；只判反向则漏掉「文档多出一条」。两侧都要，且重复命令也要报——
   一条命令覆盖不了两条测试步。
3. **旗标按集合比，顺序不比**。门控写 `--workspace --locked --exclude`，文档按阅读顺序写
   `--workspace --exclude … --locked`：这是同一批单元。顺序若成为判据，规范写法会被迫迁就门控的
   书写顺序，而那条信息量与「编了什么」无关。

另有一条**门控侧**规则：门控闭包里出现 `--no-run` 即具名失败。预构建是手工步骤，门控自己预构建
等于「编译了什么」有两处定义。

## 四、判据与注入

用例 24 → **31**（`tests/contracts/gate-coverage.test.mjs`）。五条**文件级**注入逐条真跑，
每条都先确认锚点命中（锚点不在即非零退出，MUST NOT 静默 no-op），跑完从备份拷回并 `sha256sum -c`：

| 注入 | 改动 | 变红的用例 |
| --- | --- | --- |
| I1 | 文档第一条去掉 ` --no-run` | 25 / 27 / 31 |
| I2 | 文档 `--exclude nai-xtask` → `--exclude nai-engine` | 25 / 31 |
| I3 | 门控 `--workspace --locked --exclude HARNESS` → `-p nai-storage --locked` | 25 / 28 / 31，另有既有 1/9/11/15/23 |
| I4 | 标记块 `begin` 改名 | 25 / 26 / 27 / 28 / 30 / 31 |
| I5 | 文档多一条 `cargo test -p nai-storage --locked --no-run` | 25 / 27 / 28 / 31 |
| I6 | 门控第二测试步加 `--no-run`（用例内改 `harnessText`） | 29 |

**判别性**：I1／I2／I4／I5 在**既有 24 条用例上零信号**——处方漂移这件事，包集合层
（`coverageErrors`）看不见，因为它降维后 `--no-run` 与 `--lib` 都不存在。反向也成立：29
（门控自己预构建）不被 I1–I5 打红，它是独立的一类缺陷。

I3 是**两两层都有信号**的情形，如实记录：门控换覆盖形态时既有层（1/9/11/15/23）与这层（25/28/31）
同时红。这不是重复——前者说「包集合被切走了」，后者说「处方已经对不上」——但也不能拿它当
本层的判别性样本，故上表把两批分开列。

## 五、诚实边界

- **不判 `LNK1104` 的宿主原因**。本轮只把候选面收窄到「注册的杀软里只有 McAfee 在跑」。
  一次排查不能把一个在跑的扫描器与一次链接失败之间的因果钉死 ⇒ 它是**残余**。
- **不拆 `CARGO_TARGET_DIR`**。拆目录治的是「链接时被占用」，与本轮的问题（预构建编错了东西）
  不同；对 CI 零成本，但本机要付一次冷编译（1m47s 量级），且 `build` 与 `clippy` 共用目录会互相判脏。
- **不验处方是否真的跑过**。判据只钉「处方与门控同批单元」这一条对应关系；跑没跑、跑的时候宿主
  抖不抖，属于执行面事实，不在静态判定里假装知道。
- **块内散文会被当成命令**：`documentedPrebuildSteps` 只跳过空行、`#` 行与围栏行，其余非空行一律
  当命令。这是有意的——块是处方，散文该写在块外；代价是往块里加说明会红。
- **块的位置靠标记，不靠结构**：`docs/engineering.md` 是活文档，判据不解析它的章节层级。

## 六、门控

`cargo xtask check --profile full`（`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1`）→ **第 2 次 `EXIT=0`**。

| 项 | 值 |
| --- | --- |
| 门控内 `Compiling` | **0** |
| 门控内 `LNK1104` | **0** |
| Rust | **1147 / 0**（102 条 `test result` 行，与本轮 Rust 零改动一致） |
| Node | **381 / 0**（15 组，+7） |
| `validate --all --strict` | 归档前 **44 / 0**；归档后 **43 / 0** |
| `validate --archived` | 归档后 **144 / 0**（提交时是 `143 / 1`，见末段） |

**处方有效性的直接证据**：按 §三 那两条 `--no-run` 预构建后（第一条 7 个 `Compiling`、`LNK1104` 0；
第二条 1 个 `Compiling`），门控内部 **0 个包重编、0 次 `LNK1104`**。也就是说门控这一步没有编译任何
东西，它验证的正是预构建那批单元——W146 缺的就是这一环。

**第 1 次 `EXIT=1` 与代码无关**：registry 的「待登记」检查（`tests/registry.test.mjs:2242`，57 例里
红 1 例）报出 `.w147bak/gate-coverage.test.mjs` —— 我为注入备份建的临时目录里有一份**测试文件副本**，
工作区可见、`.git/index` 里没有。判据是对的（它防的正是「未跟踪 ≠ 未登记」），处置是把备份删掉。
附带一条纪律：注入备份 MUST NOT 用 `*.test.mjs` 结尾的文件名，否则跑门控时会把门控自己打红。

**⚠️ 流程缺陷（如实记下）**：归档后我立刻跑了 `--archived`，输出是 `143 / 1`
（`✗ 4 incomplete tasks (22/26)`）——两条闭环任务（提交观测、回填）当时确实还没做完。我把「回填后
会绿」当成前提，**先行提交并推送**，而 `--archived` 正是 CI 第 16 步 ⇒ 两平台必红（§七）。正确顺序
是：闭环任务勾完、两条 validate 都绿，再提交。这一条与 W146 那次「回填占位未填就勾任务」同族：
**判据早就报了，是我没让它在提交前绿**。

## 七、CI 观测

提交 **`f1c5093`**（12 文件 / +825 −8）→ run **`39123291976`**（`Engineering foundation`），
`completed / failure`，`head_sha` 完整 40 位一致（`f1c5093f155bdae216e22b090bae715452f611c3`）。
判结论三件齐（唯一查询参数 `cb=<纳秒>` + `Cache-Control: no-cache` + 回读 `head_sha`）；
完成态由 90 s 间隔的有界轮询在第 8 次看到（首查 run 已在跑，约 11 分钟后转 `completed`）。

**两平台红在同一处**：

| 平台 | 状态 | 红在哪 | 之前的步全绿到 |
| --- | --- | --- | --- |
| `foundation (ubuntu-24.04)` | failure | 第 **16** 步 `openspec validate --archived` | 第 15 步 `pnpm spec:validate` |
| `foundation (windows-latest)` | failure | 同上 | 同上 |

逐平台看**完整步骤序列**：两平台各 **20** 条记录，编号 `1–16` 后跳 `30–33`（`if` 未触发的步不生成
记录 ⇒ 条数不能与 workflow 的 `- name:` 相减）。被跳过的步与 W146 逐条相同（`8`/`14` Linux-only、
`11` 仅失败时触发）。关键步全部 success：9 生成物无漂移、10 `check --profile full`、12 凭据扫描、
13 schema 与 API 契约、15 `pnpm spec:validate` —— 其中第 10 步在**干净 checkout** 上全绿，
说明本轮的判据与文档改动不依赖任何本机状态（这正是 W147 判据那条「非空洞前提 MUST NOT 用工作区
状态当输入」的要求）。

**成因与处置**：第 16 步跑的就是 `--archived`，而提交时归档的 `tasks.md` 还差两条闭环任务。
勾完 → 本机 `--archived` **144 / 0** → 回填提交（docs-only）后再观测一次；那一次的结果记在
`.workbuddy/memory` 的当日段，本报告记录的是**代码提交**这一次的观测。

## 八、具名登记（本轮发现但不做）

1. **`LNK1104` 的宿主成因**（承上）：候选面已收窄到 McAfee，但「在跑的扫描器」与「链接失败」之间
   没有直接证据。要判它需要复现时关掉 WebAdvisor 或换 `CARGO_TARGET_DIR` 做对照实验，那会改宿主
   配置，属于下一轮的口径决定。
2. **`docs/engineering.md` 与判据的耦合是单向的**：判据能从文档里读出处方，但文档**没有**任何机制
   阻止有人在块外再写一份「预构建怎么跑」。块外的散文今天不受判据约束（`blockquote` 里那句
   「手工预构建 MUST 用下面两条」是自然语言，不是判据）。
3. **`testCommandsOf` 成为新的一层公共解析**：`coveragesOf` 改建在它上面之后，若将来有人在
   `testCommandsOf` 里加「只保留含某旗标的命令」之类过滤，会同时改变包集合层的语义。
   今天没有这样的过滤，也没有判据守着「它不许被过滤」。
4. **W146 §八其余 4 条仍未接手**：`job_view_json` 的 `kind` 硬编码、`tests/job-surface.json` 没有
   schema、类别表与执行侧期望表靠名字耦合、执行入口零生产调用点仍无判据；W145 §八其余与 W143 §八
   4 项同样在列。

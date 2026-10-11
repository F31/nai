# W50：LiveChat 外部语料作为 B01 取材来源

- 日期：2026-10-11
- 变更：`openspec/changes/archive/2026-10-11-w50-livechat-corpus-sourcing/`
- 证据：[`docs/reports/evidence/livechat-corpus-provenance.md`](evidence/livechat-corpus-provenance.md)
- 落地区：`E:/nai-external/livechat/`（**仓库外**）

## 1. 进来的是什么，为什么需要一条边界

使用者提供了外部语料 **LiveChat**（ACL 2023，MIT），用于 B01 的 §284「至少 100 个**贴近真实直播的问题**」。

问题在于：设计规格 §5.9 只列举了三条**题集**来源（按场景人工编写／随场景包发布／坏案例经具名人工确认
回流），**没有覆盖「外部语料取材」**；而 ADR 0033 又明确**本仓库不生成题集内容**。若不加约束地接入，
这份语料会被读成「第四条题集来源」，既与 ADR 0033 冲突，也可能让 B01 的达标验收被误认为已满足。

因此本轮**不是**「把语料变成题集」，而是**把「外部语料只作素材」这条边界焊死**，并把语料安全、可追溯地落地。

## 2. 数据核对（实测）

| 项 | 值 |
| --- | --- |
| 出处 | LiveChat（`gaojingsheng/LiveChat`），`aclanthology.org/2023.acl-long.858` |
| 许可 | **MIT**，Copyright (c) 2022 Xiaobing.AI |
| 总量 | **2,411,678 条**真实中文直播对话；全局唯一主播 **346** 位 |
| 结构 | 对话集 `[streamer_id, audience_comment, streamer_response]`；指认集 `[streamer_id, comment_list(10), response]` |

**可达性（硬事实）**：Google 全不可达（代理 `502 CONNECT tunnel failed`／直连 `000`）；HF/ModelScope
**无镜像**；上游仓库无 tag/release。⇒ 全量数据**只能由人下载**，本轮由使用者下载后交付核验。

## 3. 安全摄取：先验后读

`.pk` 是 Python pickle，**反序列化即任意代码执行**。核验**不先 `pickle.load`**：

1. `pickletools.genops` 枚举 opcode 流（只解码、不执行）；
2. 仅当**每个** opcode 属于**数据白名单**时才加载。白名单不含 `GLOBAL`/`STACK_GLOBAL`/`REDUCE`/
   `NEWOBJ`/`BUILD`/`EXT*`/`PERSID` ⇒ 结构上无法命名或调用任何可调用者。

**结果**：8 个 `.pk` **全部通过**（去重后各 11 种 opcode，全属白名单）。
自检**非空洞**：含 `os.system` 的 pickle 在 offset 0/27 被具名拒绝。
解压走 `zipfile` 并**先校验成员路径**（拒绝 `..`／绝对路径／盘符）防 zip-slip。

## 4. 候选提取：机器提议，人工定夺

从两个对话集共 1,440,191 条中提取 **154,812 条**提问候选（10.7%），已去重（91,731）、去噪
（广告 5,789、纯赞美 4,796、纯招呼 2,455…）。每条带**溯源**（源文件、行号、`streamer_id`、provenance 哈希）
与主播当时的**真实回应**（供判断语境）。

### 4.1 一次被实测推翻的尝试（本轮的重要产物）

初版曾把 `连麦/上麦/管理/给你` 等词映射成 §5.9 的 `privilege_escalation`/`injection`。**抽样推翻了它**：
`injection` 档几乎全是打赏互动（「给你批个华子」）；`privilege_escalation` 96 条里真正求权限的**不到 10 条**
（「上麦/连麦」是**正常直播功能**，不是越权）。

**故整段移除类别映射**：工具只输出**原始信号词**，类别**100% 交人工**。这条否证本身是结论——
**该语料可靠覆盖的只有「问题措辞」**，对 §5.9 的非 `normal_answer` 类别**基本不适用**。

### 4.2 抽样验真（如实披露）

随机抽 60 条人工评估：明确提问约 **47/60 ≈ 78%**（含边缘约 85%）。假阳性主要是含疑问词的**陈述句**
（「没咋听歌」「什么都有」）——单句启发式**已接近上限**。

## 5. 边界（本轮的核心交付）

| 边界 | 落地处 |
| --- | --- |
| 素材 ≠ 题集（只供措辞与语境） | `docs/golden-set.md`、`tests/rag-golden/README.md`、规格需求 |
| 候选**不入库**，须具名人工确认（`promoted` + `confirmer`） | 同上（复用既有机制） |
| 素材**不使** §6 的 100 题门槛满足 | 规格需求 + B01 notes |
| 不可信容器**先验后读** | 规格需求 + 来源登记 |
| 来源可追溯（出处/许可/SHA-256） | `livechat-corpus-provenance.md` |

**B01 保持 `blocked`**：§④ 真实知识库与参考答案仍是外部投入，素材**不足以**让 §655 的达标验收达成。

## 6. 验证

| 检查 | 结果 |
| --- | --- |
| `openspec validate w50-… --strict` | valid |
| `openspec validate --all --strict --archived` | 见下 |
| `cargo xtask check --registry` | passed（V3.7 acceptance + D01–D14） |
| `node --test tests/registry.test.mjs` | pass 1 / fail 0 |
| `cargo xtask check-deps` | 18 包，方向不变 |
| `cargo xtask check --profile portable` | 见下 |

**零代码改动**：不触碰 `crates/`、`apps/`、`xtask/`、`schemas/`、CI workflow；**零新增依赖、零新增命令、
零新增证据目录**。语料与候选池均**不入库**。

## 7. 遗留

- **100 题内容**与**真实知识库**仍未接——B01 达标的两段外部投入不变。
- 本语料对 §5.9 的非 `normal_answer` 类别（注入/冲突/越权/取消/工具返回攻击）**覆盖有限，需另行编写**。
- 候选池（154,812 条）待**具名人工确认**后方可进入 `tests/rag-golden/`；在此之前它不是题集。

# 证据：LiveChat 外部语料的来源登记（B01 取材）

- 记录时间：2026-10-11
- 用途：为 B01 的「贴近真实直播的问题」（规格 §284）提供**外部素材来源登记**
- 性质：**来源与核验记录**。本文件**不含**任何题集内容，也**不**构成 B01 达标证据。

## 1. 为什么在此登记

规格 §284 的 B01 门槛要求「至少 100 个**贴近真实直播的问题**」，但设计规格 §5.9 的三条题集来源
（按场景人工编写／随场景包发布／坏案例经具名人工确认回流）**没有覆盖「外部语料取材」**这一情形。
ADR 0033 又明确**本仓库不生成题集内容**。因此本语料的正确形态是**上游素材**：供编写者取材，
而非题集来源本身。

登记这份来源，是为了让「这题的问题措辞从哪来」在需要时可被追溯（供应链可追溯），
与 W47 的 SBOM 同属一类「记录事实，不认证结果」。

## 2. 来源与许可

| 项 | 值 |
| --- | --- |
| 数据集 | LiveChat |
| 论文 | ACL 2023 Long Papers（`aclanthology.org/2023.acl-long.858`） |
| 上游仓库 | `https://github.com/gaojingsheng/LiveChat` |
| 许可 | **MIT**，Copyright (c) 2022 Xiaobing.AI |
| 分发方式 | Google Drive（上游仓库 `Dataset/README.md` 指明） |
| 采集语言 | 中文（真实直播间观众评论与主播回应） |

许可为 MIT，再分发须保留版权与许可声明；学术使用应引用原论文。

## 3. 可达性事实（硬约束，已穷尽）

- **Google 全不可达**（本机经 HTTP 代理）：`drive.google.com` / `drive.usercontent.google.com` /
  `docs.google.com` / `www.googleapis.com` 一律 `CONNECT tunnel failed, 502`；绕过代理直连为 `000`。
- **无镜像**：HuggingFace、ModelScope 均无该数据的搬运版（已检索）。
- **上游仓库无内嵌数据**：仅 `master` 分支，无 tag / release / 其他分支；`Dataset/Subset/` 只有小样本。

 ⇒ **全量数据只能由人下载**。本轮由使用者下载至仓库外目录后交付核验。

## 4. 结构与规模（实测）

数据集 README 声明三种切分结构：

- `400kDialogueInPaper`、`RawDialogueData`：每条 `[streamer_id, audience_comment, streamer_response]`
- `AddresseeRecognition`：每条 `[streamer_id, audience_comment_list(10), streamer_response]`（指认任务）

实测统计（8 个 `.pk` 全部为合法的三元组，`3-tuple rate` 100%）：

| 目录 | 样本数 | 结构 |
| --- | --- | --- |
| `400kDialogueInPaper`（dev+train） | 410,000 | 对话三元组 |
| `AddresseeRecognition`（dev+test+train） | 971,487 | 指认（comment 为 10 条列表，末条为回应对象） |
| `RawDialogueData`（dev+test+train） | 1,030,191 | 对话三元组 |
| **合计** | **2,411,678** | 全局唯一主播 346 位 |

与论文所述 1.33M 的差异由数据集 README 自述（论文统计基于更早的多人对话版本）。

## 5. 安全核验（**先验后读**）

`.pk` 为 Python pickle，反序列化即任意代码执行。核验**不先 `pickle.load`**：

1. 用 `pickletools.genops` 枚举 opcode 流（只解码、不执行）；
2. 仅当**每个** opcode 属于**数据白名单**时才加载。白名单不含 `GLOBAL`/`STACK_GLOBAL`/`REDUCE`/
   `INST`/`OBJ`/`NEWOBJ`/`BUILD`/`EXT*`/`PERSID` ⇒ 结构上**无法命名或调用任何可调用者**。

结果：**8 个 `.pk` 全部通过**（去重后各 11 种 opcode，全部为白名单成员）。
自检证明守卫**非空洞**：含 `os.system` 的 pickle 在 offset 0/27 被具名拒绝。

解压走 `zipfile` 逐成员落地并**先校验成员路径**（拒绝 `..`、绝对路径与盘符）以防 zip-slip。

## 6. 取材产物（**仓库外**）

落地区 `E:/nai-external/livechat/`（源数据 459M + 候选 50M **不入库**）：

- `all_candidates.jsonl`：**154,812 条**提问候选，带溯源（源文件、行号、`streamer_id`、provenance 哈希）；
- `review_sample.json`：分层抽样（~320 条）供人工审阅；
- `extraction_summary.json`：提取统计。

自 1,440,191 条对话中提取（10.7%），已去重、去噪。**抽样验真约 78%** 为真提问（含边缘约 85%）——
假阳性主要是含疑问词的陈述句，**单句启发式已接近上限**。

## 7. 边界（**必须随本登记一起读**）

- **素材 ≠ 题集**：本语料只供「问题**措辞与语境**」，**不**供产品知识库，**不**供每题的
  `evidence`/`expected`。**它不会单独让 B01 达标**——§④ 真实知识库与参考答案仍是外部投入。
- **候选不进仓库**：题集只能经**具名人工确认**后写入 `tests/rag-golden/`，`origin.kind = promoted`
  须带非空 `confirmer`（§5.9、ADR 0033）。本登记的产物**不含**该确认。
- **类别覆盖有限**：抽样证明**本语料可靠覆盖的只有「问题措辞」**；对 §5.9 的
  注入/冲突/越权/取消/工具返回攻击**基本不适用**（「上麦/连麦」等词是正常直播功能，
  不是越权；曾试作的类别初筛已被推翻并**整段移除**）。
- **B01 状态**：**保持 `blocked`**——§655 的达标验收（100 题集达到第 6 章门槛）仍未达成。
  本登记只把「问题措辞的素材来源」这一段补齐，并如实说明其**不足以**让 B01 达标。

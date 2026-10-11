# W50 任务

## 1. 外部语料的安全摄取与落地（**仓库外** `E:/nai-external/livechat/`）

- [x] 1.1 解压三个 zip（**先校验成员路径**防 zip-slip），得 8 个 `.pk`（459M）
- [x] 1.2 逐文件 SHA-256（原始包 + 解压后 `.pk`）
- [x] 1.3 `verify_pickle.py` 安全解析器：`pickletools.genops` 枚举 opcode + **数据白名单**，先验后读；自检非空洞
- [x] 1.4 8 个 `.pk` 全部通过核验（去重后各 11 种 opcode，全属白名单）
- [x] 1.5 结构核对：对话集 `[streamer_id, comment, response]`；指认集 `[streamer_id, comment_list(10), response]`；`3-tuple rate` 100%

## 2. 统计与候选提取（**仓库外**）

- [x] 2.1 `corpus_stats.py`：总 2,411,678 条、全局唯一主播 346 位、逐文件条数与长度分位
- [x] 2.2 `extract_candidates.py`：从 1,440,191 条对话提取 **154,812 条**提问候选（去重、去噪、带溯源）
- [x] 2.3 输出 `all_candidates.jsonl` / `review_sample.json` / `extraction_summary.json`
- [x] 2.4 抽样验真（60 条）：明确提问约 78%（如实披露）
- [x] 2.5 **移除**按词映射 §5.9 类别的初版（实测假阳性极高）：工具只输出原始信号词，不产出类别
- [x] 2.6 `README.md` 重写：来源/许可/SHA-256/核验/统计/提取/边界/复现

## 3. 仓库侧：来源登记与边界（**零代码**）

- [x] 3.1 新增 `docs/reports/evidence/livechat-corpus-provenance.md`（出处、许可、摘要、结构、规模、核验、可达性、边界）
- [x] 3.2 `docs/golden-set.md` 增补「素材来源（外部语料）」段：素材 ≠ 题集的四点硬边界
- [x] 3.3 `tests/rag-golden/README.md` 增补素材段，明确候选仍须具名确认
- [x] 3.4 `tests/registry.yaml` 的 B01 notes 追加「素材来源（W50）」，**状态保持 `blocked`**

## 4. 规格增量与收尾

- [x] 4.1 `knowledge-quality-gate` 规格 +1 条需求：外部语料只作素材 + 候选须具名确认 + 未使门槛满足 + 先验后读 + 来源可追溯
- [x] 4.2 `AGENTS.md`、`docs/development-plan.md` 记入本轮
- [x] 4.3 报告 `docs/reports/2026-10-11-w50-livechat-corpus-sourcing.md`
- [x] 4.4 门控：`check --registry` / `check-deps` / `check --profile portable` / `openspec validate --all --strict --archived`
- [x] 4.5 一次 commit + push；匿名观测 CI 结论

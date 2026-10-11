## Why

规格 §284 的 B01 门槛要求「至少 100 个**贴近真实直播的问题**」。而设计规格 §5.9 只列举了三条**题集**来源
（按场景人工编写／随场景包发布／坏案例经具名人工确认回流），**没有覆盖「外部语料取材」**这一情形；
ADR 0033 又明确**本仓库不生成题集内容**。

使用者提供了一份真实直播语料 **LiveChat**（ACL 2023，MIT）用于 B01 取材。若不加约束地接入，它会被读成
「题集来源」，从而与 ADR 0033 的边界冲突、并可能让 B01 的达标验收被误认为已满足。因此本轮要做的不是
「把语料变成题集」，而是**把「外部语料只作素材」这条边界焊死**，并把它安全、可追溯地落地。

要点：**素材 ≠ 题集**。素材供「问题**措辞与语境**」，**不**供产品知识库与每题的 `evidence`/`expected`，
因而**不会单独让 B01 达标**——§④ 真实知识库与参考答案仍是外部投入。

## What Changes

- **登记素材来源**：新增 `docs/reports/evidence/livechat-corpus-provenance.md`——出处、许可（MIT）、
  逐文件 SHA-256、结构、规模、安全核验方式、可达性事实与**取材边界**。
- **安全摄取（仓库外）**：落地区 `E:/nai-external/livechat/`。`.pk` 是 pickle，反序列化即任意代码执行，
  故**先验后读**——用 `pickletools.genops` 枚举 opcode 并对**数据白名单**放行，任何可命名可调用者的
  opcode（`GLOBAL`/`STACK_GLOBAL`/`REDUCE`/`NEWOBJ`/`BUILD`/`EXT*`/`PERSID`）一律拒绝；解压先校验成员
  路径防 zip-slip。**8 个 `.pk` 全部通过**。
- **候选提取（仓库外）**：从 1,440,191 条对话提取 **154,812 条**提问候选（去重、去噪、带溯源），
  供人工审阅。候选**不进仓库**，仍须**具名人工确认**才可成为 `tests/rag-golden/` 的用例。
- **焊边界（仓库内活文档）**：`docs/golden-set.md` 与 `tests/rag-golden/README.md` 增补「素材来源」段，
  明确素材与题集的四点区分；`tests/registry.yaml` 的 B01 notes 追加「素材来源（W50）」。
- **`knowledge-quality-gate` 规格 +1 条需求**：把「外部语料只作素材」的边界写成可测条文。
- 不做：**不**在仓库内新增代码、依赖、命令或证据目录；**不**把语料或候选入库；**不**改动任何 `src/`；
  **不**升级 B01 的 `status`（保持 `blocked`）。

## Capabilities

### Modified Capabilities

- `knowledge-quality-gate`：新增一条需求——外部语料是**取材素材**而非题集来源；候选须经具名人工确认才可
  入集；素材的存在**不**满足 §6 的 100 题门槛；从不可信容器格式摄取素材时须**先核验字节码可执行性再解析**；
  素材来源须可追溯。

## Impact

- 新增：`docs/reports/evidence/livechat-corpus-provenance.md`；
  `openspec/specs/knowledge-quality-gate/spec.md` 的一条需求（经归档吸收）。
- 修改：`docs/golden-set.md`、`tests/rag-golden/README.md`、`tests/registry.yaml`（B01 notes）、
  `AGENTS.md`、`docs/development-plan.md`。
- **零代码改动**：不触碰 `crates/`、`apps/`、`xtask/`、`schemas/`、CI workflow。
- **零新增依赖、零新增命令、零新增证据目录**：语料与候选池均**不入库**；载荷仍只在 `tests/rag-golden/`。
- **登记纪律**：B01 保持 `blocked`——§655 的达标验收未达；本轮的素材**不足以**改变该结论。
- **验证**：`check --registry`、`check-deps`、`check --profile portable`、`openspec validate --all --strict --archived`。

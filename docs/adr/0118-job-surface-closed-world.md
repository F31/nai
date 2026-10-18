# ADR 0118：作业面闭世界 —— 15 条「铸了 `job_id` 却没有状态来源」的声明怎么收口

- 状态：已接受（W144，2026-10-18）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§30.1 第 4 条 / §662 O02）
- 相关：W138 报告（§五第 1 条）、W143 报告（§八 第 5 条）、ADR 0113（作业状态接 journal）、
  `scripts/contracts/job-surface.mjs`、`tests/contracts/schema.test.mjs`、
  `schemas/openapi/control.yaml`

## 背景

`control.yaml` 里有 **15** 个操作声明 `202 + JobAccepted`，而唯一读作业状态的
`GET /api/v1/jobs/{id}` 只读 `upgrade_journal` 一张表。这件事契约自己就写着：

```yaml
kind: { enum: [upgrade], description: '... only upgrade jobs have a journal today.' }
```

于是其余 14 条**铸了一个指不到任何状态的 `job_id`**：客户端拿到 202 之后能做的只有等一个
永远不会到来的终态。`JobAccepted` 在生产侧**零生产者**——
`grep -rn "JobAccepted" crates/ apps/ --include=*.rs` 只命中生成物里的 `SCHEMAS_JSON` 常量；
15 条路由在控制面的路由表里**一条分支都没有**，全部落到 `:665` 的兜底 404
`CapabilityAbsent "control operation not implemented"`。

### 计数订正（W138 的 12 / 14 / 15）

W138 §五第 1 条写「12 条同族路由」，而同一份报告自己记的 `grep -c "jobResponses"` 是 15
（含锚点定义行，即 1 个锚点 + 14 个别名）。本轮用**解析器口径**（`SwaggerParser.parse`
逐 `path × HTTP_METHODS` 取 `responses['202']`）定案：

| 口径 | 数字 |
| --- | --- |
| `control.yaml` 的 `paths` | 40 |
| 带 `202` 的操作 | **15** |
| 其中 body 是 `$ref #/components/schemas/JobAccepted` | 15 |
| 其中带 `x-implementation-status` | **0** |
| `remote-knowledge.yaml` 带 `202` 的操作 | 0 |

差额来自两处：W138 的清单里列了 13 个端点（标题写 12），又漏了
`providers/{id}/test`、`devices/{id}/revoke`、`restore/preflight`，并且没有把
`upgrades/execute` 与 `upgrades/{id}/rollback` 计入同一族。**「12」这个数字从来没有对过**，
它是一次靠记忆汇总的产物。

## 决定一：不实现 15 套状态机，改收**声明面**

给 14 条路由各造一张状态表，是 14 次数据库迁移 + 14 套状态机，而且**每条的状态形状至今
没有人设计过**（备份的终态是什么？测评的终态是什么？）。把一件没有人设计过的东西
赶进一轮实现，产出的一定是 14 个各自编造的进度词汇。

本轮收的是**声明面**：让「声明了 202 却没有状态来源」这件事**不可能再静默**。

1. 每一条 `202 + JobAccepted` MUST 在册（`JOB_SURFACES`）；在册项 MUST 真的是一条 202 操作。
   两条一起才是「集合相等」——它挡的是**未来某次合法编辑**（新增一条 202 而忘了登记），
   不是当前的错。
2. 在册项 MUST **恰有其一**：`kind`（真能被 `/jobs/{id}` 轮询）或具名的 `blockedBy`
   （今天不铸 `job_id` 的理由）。
3. `kind` ⇔ 已实现。有 `kind` 却未实现 ⇒ 能被轮询但永远起不来的状态源；已实现却声称
   `blockedBy` ⇒ 与自己的声明相矛盾。

第 2 条是这条 ADR 的全部要点：它把「我也不知道该怎么办」这个状态**从一个沉默变成必须写的值**。
形状不是新造的——契约早就为「共享 job id 空间」留了 `kind` 判别键，判据只是让那个键
在两个方向上都成立。

## 决定二：`blockedBy` 的两个取值 MUST 分开

```js
no_handler:                  // 路由表里零分支，落到兜底 404 CapabilityAbsent
refuses_without_installer:   // 有专用分支，缺安装器，503 CapabilityNotReady 且明确不铸 id
```

今天 15 条里 13 条是前者，`upgrades/execute` 与 `upgrades/{id}/rollback` 是后者
（`crates/nai-control-api/src/lib.rs:628/630` 调 `upgrade_refusal`，`:1121` 逐字写着
`no job id is minted`）。

混同会让「做不到」与「没有这条」在客户端不可区分——ADR 0113 已经为读取面裁定过同一件事。

**每个取值都带一个源码锚点**（`{file, text}`）：声明指向仓库里一段**逐字文本**。没有对端的
声明就是一条注释，而注释不会变红。锚点的选择原则是「这句话一旦改掉，这个声明就不再成立」：

- `no_handler` → `control operation not implemented`（兜底那一句）
- `refuses_without_installer` → `no job id is minted`（拒绝体里定性那一句）

`kind` 的锚点同理，但要更小心：`JOB_KINDS.upgrade.reader.text` 取的是 `"kind": "upgrade"`
而**不是** `"upgrade"`——同一个文件里 `upgrade_refusal` 的类别名也是 `"upgrade"`，
只锚那个词的话两处会被同时命中，于是「reader 不再报这个 kind」永远不会变红。
用例里有一条断言专门钉住这个选择（裸词出现 >1 次、带判别键的形式恰好 1 次），
谁把它简化回去谁就当场看到红。

## 决定三：判据分两层，`readSource` 由调用方注入

- `jobSurfaceErrors(documents)` —— **契约侧**：202 扫描、`$ref` 唯一、互斥、`blockedBy` 闭集、
  `kind` ⇔ 已实现、`JobView.kind` 与 `JOB_KINDS` 双向相等、文档键集双向相等、非空洞。
- `jobSourceErrors(kinds, blockers, tracked, readSource)` —— **仓库侧**：每个锚点指向的文件
  MUST 被跟踪、MUST 可读、那段文本 MUST 还在。

两层不合并，因为输入域不同（一份是解析后的文档对象，一份是 `.git/index` 的路径集 + 源码文本），
合在一起会让「错误少一半信息」。

`readSource` 是**注入**的而不是模块里 `readFileSync`：这个模块只判定、不做 I/O，
判定本身才能被合成输入驱动（真实仓库跑一次，合成样本各跑一次）。
三种失败 MUST 分开报——「文件没被跟踪」「文件读不出来」「文本不在了」，
折成一条会丢掉**到底哪一步断了**。

## 被否决的方案

**一、把 14 条改成同步 `200`。** 要么伪造成功（操作根本没做，返回一个编造的资源视图），
要么让整族端点变成 `503`。前者比铸一个假 id 更坏：假 id 至少还承认「在等」，
假 200 直接宣称「做完了」。后者不是不可能，但那是一次**能力声明**的变更（「这 14 个操作
我们不做」），得由产品决定，不能混在一轮判据补缺里顺手改掉。

**二、删掉 `JobAccepted`，让 15 条不再声明 202。** `JobView.kind` 的「共享 job id 空间」
这句话会随之失去对端（一个只有 `upgrade` 的枚举不再需要判别键），而
`upgrades/execute` 一旦拿到安装器仍然要用它。删掉等于把将来的形状一起删了。

**三、只登记、不判定。** 那就是一条注释。注释不载重：它挡不住下一次新增 202 的编辑，
而「新增一条 202」正是这件事最初发生的形状。

**四、给 `blockedBy` 加第三个状态值（例如 `deferred`）。** 会当场撞上 `operationErrors` 的
闭世界——`expected = declaredSet.has(key) ? IMPLEMENTED_STATUS : undefined`，即
「不在已实现表里 ⇒ MUST 没有 `x-implementation-status`」。要加第三个值得先放宽那条已有覆盖的
判据，等于拿判据换覆盖率。`no_handler` 与 `refuses_without_installer` 已经覆盖了今天
两种真实的失败形状，没有第三种。

## 具名的代价与残余

1. **`blockedBy` 的取值本身仍是声明。** 锚点只证明「那句源码还在」，不证明「这条路由真的
   没有分支」。把它变成可判定的需要一张执行面的名册（谁在路由表里有分支），那是 W143 §八
   已登记的「产物与扫描域的界线」那一类问题，本轮不顺手做。
2. **`JobAccepted` 仍然零生产者。** 本轮一个 202 都没有接上真实实现，所以「`JobAccepted`
   在 Rust 侧没有任何构造点」这个事实**没有变化**——它现在是被**登记**着，不是被消除。
   这与「`status` 留 `pending`」同一条纪律：如实披露不是缺陷，不实声明才是。
3. **口径坑（写给下一个人）：`SwaggerParser.validate` 会把 `$ref` 内联。** 用 `validate`
   量出的 `202_with_JobAccepted` 是 `0`（全部变成内联对象），用 `parse` 量出 `15`。
   凡要判「是不是同一个 schema」，MUST 用 `parse`。
4. **`remote-knowledge.yaml` 的作业面是空数组。** 本轮显式登记为 `{}` 而不是省略：
   「我们看过，没有」与「我们忘了」是两件事，后者会让「给它加一条 202」变成零信号的编辑。

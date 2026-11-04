# V48-06 首个纵向切片：中文引用问答

goal: 已发布资料中的改写中文问题可检索FAQ原答案、显示出处并人工采用到草稿。
depends_on: V48-04, V48-05选定单句工程证据。
allowed_paths: schemas/workbench/foundation.proto, schemas/content/publication.schema.json, runtime-core/assistance, storage-sqlite/assistance, workbench-host/tests, workbench-desktop/ui/faq*, tests/fixtures, docs, openspec。
contracts: FaqReason.Retrieved与FAQ任务retrieval元数据，生成后修改消费者。
migrations: 无表迁移，不重写不可变旧工件。
invariants: 工作区/发布/版本/角色/TTL/取消门控沿既有路径；不跨产品检索，不调用模型，不授公开播放。
ACC: KB-01–05、AG-01–09相关局部证据；整项仍pending。
fixtures: tests/fixtures/contracts/chinese-faq-evaluation.json固定20例，不将词法命中称语义理解。
commands: 定向runtime/storage测试、原生Owner FAQ测试、前端FAQ及构建、生成无漂移与相关clippy。
evidence_kind: 单元/临时存储/Windows原生IPC/页面渲染；人工真实资料体验另记。
external_prerequisites: 更新运行版时需关闭Windows工作台；无付费API依赖。
done_when: 20例结果与原文引用一致、歧义/缺依据不猜测、取消/更新后不可采用、真实Owner检索与草稿采用通过、界面明确检索出处。

本卡不完成整个V48-06。后续仍需统一Harness/LiveContext/问题池与LLM检索后生成，再接V48-07连续讲品、插问归位和接管。

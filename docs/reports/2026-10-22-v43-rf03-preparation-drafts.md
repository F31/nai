# V43-RF03 类型化备播草稿子任务

2026-10-22。RF03第一段已实现/定向实测，整卡仍进行中；M1与90项ACC均未授予通过。新界面仍是RF01能力检查页，没有安装更新或付费请求。

## 已实现

- 源契约 `schemas/content/preparation.schema.json` 定义 product/host/series/show 四类草稿，闭世界字段和有界集合。先生成Rust/TS/C++契约及TS文档类型，再修改消费者。
- 产品包含类型事实、来源、人工确认/未核对声明、适用条件与有效期，FAQ/话术绑定本产品事实，另存禁说项。金额为最小币值整数串，decimal用十进制串，布尔不用字符串代替。人工确认声明不等于系统独立验证真实性。
- 主持偏好仅表达风格。场次草稿固定产品/主持/系列的UUID、revision、SHA256与节目cue。此阶段只验证引用形状，不宣称跨对象存在、类型或新鲜度核验已完成。
- `FoundationCommand.put_preparation` 走认证后的唯一writer，沿用新线ACL、CAS、幂等、审计/outbox及不可变对象版本。原始JSON精确保留，重启可读历史及最新版本。8KiB正文/16KiB命令限制继续执行。
- 核心消费同一权威Schema，再检查重复ID、事实绑定、有效期反转和解码后的控制字符。允许保存不完整草稿；非法字段及悬空本地事实绑定拒绝。
- 对象创建后固定草稿类型；generic put_object不能抹除类型或注入保留草稿信封，四类草稿不能互相覆盖。私有交付在prepare和执行门控均拒绝草稿，保存不产生发布、批准、音频或媒体进程。

## 定向验证

- WSL：storage 24、core 8、bootstrap真实进程6、foundation真实进程9通过。foundation另有1个ignored强杀辅助worker，由父测试显式以`--ignored`运行，不计为额外通过项。
- Windows/MSVC：storage 23（Unix符号链接用例不适用）、core 8、bootstrap真实进程6、foundation真实进程9通过；同一辅助worker说明适用。
- 新用例核对原文/hash历史保留、重启最新版本、幂等、陈旧CAS、非成员拒绝、无成功审计半提交、类型擦除/切换拒绝、未发布草稿无delivery intent，以及真实子进程往返。
- Schema正反例、相关基础/源契约/CI检查、相关模块clippy、生成无漂移和三语言互通的最终结果见末尾补记。
- 首次新增测试丢弃TempDir导致目录提前删除，已修正测试生命周期；首次lint嵌套条件已修正，未忽略失败或扩大平台豁免。
- 未执行全仓测试、正式发布门槛、远端CI或Windows安装/设备/OBS验收。

## 剩余工作

1. 发布事务：不可变snapshot、完整引用闭包、有效性/未核对项、字段影响报告和graph watermark竞态校验；紧急撤销先收紧许可，失败不恢复。
2. 产品/主持/系列/节目列表及新备播UI，选择优先、合理默认、必填在前；UI只经engine读写。
3. 单产品编辑→发布→ContentOnly/Assisted私有预演→停止→重启读取的M1纵向体验。界面呈现/阅读证据单独定义，Submitted不提升为Completed。

完整话术AST/素材依赖、变更处置、自动事实核验、公共输出及真实直播未实现。未将这些代码缺口记作外部Blocked。安装、旧数据库、Key、许可和素材未打开或迁移。

## 最终检查补记

Node直接相关13项通过；依赖方向24个workspace包通过；WSL及Windows相关模块clippy均通过。`cargo xtask test --suite contracts`成功，含生成无漂移、schema检查2项、wire检查2项、源/注册表11项及Rust→TS→C++→Rust、Rust→C++→TS→Rust两条真实互通路径（4种消息，新增put_preparation分支携带中文草稿）。没有运行远端CI。`git diff --check`定向通过。

OpenSpec子任务已归档为`2026-10-22-v43-rf03-preparation-drafts`，主规格`preparation-drafts`生效；全部48项严格校验通过。此归档仅关闭草稿子任务，不代表RF03发布/UI或M1完成。

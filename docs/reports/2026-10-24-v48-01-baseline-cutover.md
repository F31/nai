# V48-01：实际基线与验收切换

状态：本卡工程范围Implemented/Verified；164项产品ACC仍全部pending，不授予媒体/平台/商业认证。

## 实际变更

源Schema先扩展为V4.8：164ACC、62INV、15张V48工作卡，补simulated/not_selected及可审查排除条件。当前注册表逐字匹配新版验收和不变量正文，原90个编号全部保留，新增74项pending。原注册表原始字节及SHA256存于tests/legacy/v43，原前置证据完整保留，新线不继承tested。

检查器验证完整性、任务引用、文本漂移、模拟/前置证据不能授真实通过，unknown或claimed条件不能当不适用/未选。AGENTS/OpenSpec/catalog来源、工程提示与设计入口同步V4.8；ADR0151Accepted。默认领域/进程边界及当前路径、后续迁出任务、单一结构源裁决和真实命令清单已记录，没有批量新建空crate、服务或第三套前端。

输出：[目标映射](../acceptance/v48-target-matrix.md)、[边界/命令清单](../architecture/v48-boundaries-and-command-inventory.md)。CapabilityStatus完整多维模型、RunPlan/ReleaseManifest和适用套件解析仍在后续任务；不能将当前注册表排除条件检查等同完整装配认证。

## 实际检查

- 项目锁定OpenSpec严格校验通过。
- Node注册表/契约源/CI消费者16项通过；包含原90项快照哈希与证据保留、活动基线一致性和模拟/条件负例。
- Windows本机nai-xtask --lib：62项通过，包含真实cargo metadata依赖负例。
- Windows本机cargo run -p nai-xtask --offline --locked -- check-deps：26个workspace包检查通过。
- 项目生成器重新生成及schema --check：Rust/TS/C++/JSON Schema/OpenAPI无漂移。
- 专用前端类型检查/生产构建通过；仍使用已有v43.html作为真实用户入口，文件名不作为产品基线证明，不重建前端。

WSL首次check-deps因无cargo metadata未执行成功；已用本机完整Rust环境补验通过，不将工具缺失记平台Blocked。未运行全仓测试、Windows媒体/OBS、GPU、付费Provider或平台认证；本卡无业务库迁移、Key操作、程序更新。

## 下一步

V48-02共享模型与凭据迁移，先做来源映射/冲突预览、一致备份和journal，包含回收区原模型；不能简单移动仍绑定workspace的密文。V48-03含关联数据永久清理必须在共享模型保护完成后开放。原来的预算设置已可用，不需要重复实现。

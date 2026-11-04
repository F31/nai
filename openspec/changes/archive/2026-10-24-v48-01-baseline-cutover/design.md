## Design
先调整注册表源Schema，再迁移数据与消费者。原90项完整证据保留在只读快照，新表保留证据引用但不升级状态，新增74项pending。164要求/62不变量逐字核对设计表，任务映射按领域而非默认全通过。模拟、未选与不适用分别记录，不能用模拟或前置证据授tested。统一nai命名，明确domain/port/adapter/engine边界；公共消息Protobuf、配置JSON Schema、HTTP OpenAPI，附录M示例以单源生成衔接，不重复手写。定向运行注册表、依赖、生成/消费者检查，新增schema后重新生成。

# LLM/TTS数据库配置与凭据持久化（2026-10-20）

## 交付
用户要求所有Provider参数保存数据库并可在界面修改。本轮新增宿主独立provider-settings.sqlite3，完整场景（LLM/TTS地址、协议、模型/音色、主备顺序、预算参数、路由策略）与API Key密文同库存储。SQLite事务原子替换配置/密文，500ms busy timeout；user_version=1，未知新版本拒绝，不改变engine Store或其迁移链。

API Key使用ring AES-256-GCM、每次随机96位nonce，ID/协议/endpoint作为AAD；32字节随机主密钥通过已有nai-security::NativeVault保存到Windows凭据管理器，按数据根隔离。数据库、JSON、日志、前端读取都没有明文Key；界面仅通过生成Protobuf读取已保存ID集合。主密钥缺失或密文篡改明确失败，不新建主密钥覆盖既有密文，不降级明文。

界面自动读取数据库，支持新增/修改、1主4备、上移/下移、删除供应商、更换Key及“删除已保存Key”。空Key保留原值，更改ID/协议/endpoint丢弃旧绑定；更改模型/音色/顺序保留凭据。保存后清空输入，不回显密钥。启动、离线/联网预检及显式单项测试可使用已保存Key；单次输入仍可覆盖但只在显式保存时持久化。

旧JSON场景通过显式保存导入；数据库是权威，operator-scenes/provider-scene.json仅为无密钥引擎兼容导出，每次使用前从数据库重建。数据库提交后若导出或host.json保存失败，返回具名错误，数据库仍保留成功提交的数据，可重新读取/保存恢复；不承诺跨SQLite和文件的原子事务。不同场景不自动注入保存的Key，只有数据根拥有的导出场景使用保存凭据。

依赖成本：独立宿主复用锁定rusqlite、nai-security和ring 0.17.14，无新增常驻服务。先定义schemas/storage/provider-settings-v1.sql和ProviderCredentialStatus，再生成Rust/TS/C++；未手改生成代码。OS主密钥不在DB内，单独复制数据库到其它Windows账户/数据根不能解密；配置库当前不属于engine备份范围，跨机迁移需重新填写Key。数据库与OS凭据持久化不等于命名管道ACL或硬件认证。

## 验证
Windows独立壳18项lib、6项真实engine集成通过，clippy all-targets干净。新增真实Windows NativeVault写入/读回/重开/清理；真实ShellState保存后销毁重建、无Key输入的引擎离线预检通过；故意损坏导出JSON后重建，删除Key后provider不再通过。使用测试凭据与本地操作，没有新增真实付费API调用。

其它测试覆盖加密持久化重开、替换/保留/删除、数据库文件无明文、endpoint更换隔离、AEAD篡改拒绝、vault失败及事务触发器故障回滚、主密钥丢失不重置。UI覆盖显式保存payload、无密钥状态Protobuf、保存凭据直接测试、密钥清理和真实组件渲染。最终独立full复跑EXIT=0：107个Rust测试目标、1235 passed / 0 failed，Node 463 passed / 0 failed（界面119项）。19类Rust/TS/C++互通、schema --check无漂移通过。日志/tmp/nai-provider-db-full-isolated.log、/tmp/nai-provider-db-contracts.log、/tmp/nai-provider-db-schema-check-final.log；原生日志.nai-diagnostics/provider-db-native-final2.log。

新增本机导入工具save_provider_settings：路径仅走参数，Key映射通过有界stdin，不启动/预配引擎、不联网、不输出Key；用于用户已授权参数的本机数据库初始化。

真实参数落库：用户提供的Agnes与SiliconFlow地址/模型/音色及两条API Key，已通过掩码stdin写入当前Windows用户的工程版数据根（AppData/Roaming/cn.nai.desktop-shell.engineering/runtime/provider-settings.sqlite3），重新打开后两条保存状态核验通过。导入未执行任何API调用。
首次full因provider_db.rs的Windows平台分支未登记而失败两项；已将该文件登记到既有secret-protection能力，未改变平台支持状态，最终完整复跑通过。

第二次full的registry 31项断言均通过，但我并行清理安装测试目录，触发仓库扫描的scandir ENOENT，测试进程判失败。安装测试已经结束，停止并行目录移动后独立完整复跑。
新版未签名NSIS 10,917,386字节，SHA256 a0a4bccae60af8998fbab64c344938f8c7ead7e6edae47223c4bbe1a418b9c15。真实安装/覆盖与活动拒绝/随包预检/页面加载/卸载通过，既有数据哈希保持，含新配置库；证据.nai-diagnostics/provider-db-package.log及provider-db-install.log。

安装/卸载实测保留5个既有数据文件的哈希不变；包含加密配置与兼容导出。代码尚未提交。

规格同步明确修改既有配置/测试要求：清空的是Key输入，已保存密文保留；无密钥限制指JSON导出，不禁止数据库密文。与本轮持久化要求一致。

OpenSpec已归档2026-10-20-r1-provider-database，43个主规格严格校验通过，167项归档一致性3/3通过；不升级registry产品验收状态。

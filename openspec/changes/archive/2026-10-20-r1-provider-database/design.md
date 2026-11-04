# Design
先定义SQL存储格式与无密钥Protobuf状态契约，再生成契约。独立provider-settings.sqlite3，user_version=1，不改变引擎数据库或迁移链。完整场景遵守现有manual-session JSON Schema。SQLite有界busy_timeout；事务原子替换配置与密文，失败保留旧配置。
AES-256-GCM随机nonce、身份绑定AAD；随机32字节主密钥存Windows NativeVault，数据库只存密文。主密钥不可用明确拒绝，不回退明文。JSON/前端/日志不得回读Key；UI只有已保存状态和替换/删除。保留旧Key仅当ID、协议和endpoint一致；更换端点清除旧凭据，避免意外泄露。启动、预检、单项测试仅向所选场景/身份注入。
现有场景首次显式保存导入数据库；数据库为权威，导出JSON用于既有引擎。停止后方可保存/删除。独立宿主Mutex串行配置操作；跨进程DB写事务串行。测试覆盖重开、替换、删除、端点更换、篡改、vault失败和事务失败；Windows原生凭据库实测，实际API不重复调用。

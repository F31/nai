# V48-02 凭据重加密原语

2026-10-24。状态：内部原语已实现和定向实测；完整迁移未完成。

## 交付

runtime-core新增credential_rewrap，通过显式源/目标CredentialVault和身份完成源open、目标seal与目标open回读一致性验证。端点变化拒绝，明文限制1–8192字节，使用Zeroizing承载。每个Vault调用前后检查取消/期限，不记录明文，不重试，不删除源条目。仅返回已验证目标密文。本轮无跨语言/持久化契约变化，不需要新增Schema或修改生成文件。

## 验证

Windows原生 nai-runtime-core --lib：28 passed/0 failed，其中6项新用例覆盖作用域改变、源解密失败、目标密文损坏、seal失败、开始前取消和seal后取消。相关all-targets clippy -D warnings通过。使用测试Vault，不访问系统Keyring或真实API，不作为原生加密认证。

## 剩余边界

尚未接入Store、共享目录或用户界面；源/目标能力的可信选择由后续维护事务负责。操作期限不强行打断正在执行的同步Vault调用，只在边界拒绝交付。seal后失败可能留下目标Keyring条目，必须由迁移journal记录处理，不能声称原子回滚或幂等迁移已完成。原用户数据库/Key/安装均未操作。164ACC不升级。

下一步：绑定已验证备份和预览hash，建立可恢复的迁移journal及共享目录提交，再统一LLM/TTS调用和模型管理入口。

## Why
供应商配置只存文件、Key仅在会话内，重启后无法恢复用户已经配置的服务。用户要求数据库持久化并在界面修改。
## What Changes
将现有LLM/TTS完整场景与路由存入宿主独立SQLite；API Key以AES-256-GCM密文同库存储，主密钥复用nai-security Windows NativeVault。界面读取、编辑、更换/删除Key，启动/预检按保存身份解密注入。JSON仅为引擎兼容导出，不是凭据源。
## Impact
R1设置交付，V3.11配置/凭据边界；仅补A02/B03工程证据，不升级验收状态。宿主复用rusqlite/nai-security，并新增已锁定ring密码库依赖，无常驻服务。数据库不接管引擎Store，避免双owner。

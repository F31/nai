## Why
V43-RF03需要先建立类型化备播文档，RF02的普通人工文本不足以表示事实、主持偏好与节目绑定。对应V4.3第13–16章、INV-05/26/30及ACC-CONT-01/02的前置基础，不授予完整验收通过。
## What Changes
- 新增产品事实/FAQ/禁说项、主持偏好、系列与场次草稿JSON Schema及生成类型。
- 新增认证后的put_preparation命令，复用RF02唯一writer与不可变对象版本、ACL/CAS/审计/幂等。
- 保存不获得发布许可，拒绝草稿直接交付与通用文本入口擦除草稿类型。
## Capabilities
### New Capabilities
- `preparation-drafts`: 类型化草稿保存及无发布权限边界。
### Modified Capabilities
## Impact
仅影响新线schemas、生成契约、runtime core和SQLite writer。core新增仓内已锁定的serde_json/jsonschema库依赖，按需校验，无新增进程、端口、云服务或常驻依赖；validator惰性编译缓存，正文继续限制8KiB。发布/影响报告/UI留RF03后续，安装、旧数据、Key与付费API均不改动。

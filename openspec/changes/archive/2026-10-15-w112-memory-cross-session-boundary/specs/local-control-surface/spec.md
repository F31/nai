## ADDED Requirements

### Requirement: Memory endpoints reject cross-session credentials

控制面 SHALL 保证 memory 读取 / 遗忘端点绑定当前场：鉴权走 `self.access`（绑定 session_id + device）。用别的 session 或别的 device 的 token 访问本场控制面 SHALL 以 `401` / `403` 拒绝，MUST NOT 被接受。

#### Scenario: A token from another session is rejected

- **WHEN** 用 session B 颁发的 token 访问 session A 控制面的 `/api/v1/memory/read`
- **THEN** 返回 `401` / `403`，MUST NOT 读到任何记录

#### Scenario: A token from another device is rejected

- **WHEN** 用别的 device 颁发的 token 访问本场控制面
- **THEN** 返回 `401` / `403`

### Requirement: Memory scope is a fact of the control surface, not a request parameter

控制面 SHALL 从自身绑定取 `scope.session_id` / `workspace`，请求体中出现同名字段 MUST 被忽略（沿用 W109/W111 纪律）。

#### Scenario: A forged session_id in the body is ignored

- **WHEN** 请求体携带与当前场不符的 `session_id`
- **THEN** 控制面仍用自身绑定的 session_id 构造查询，伪造字段不影响作用域

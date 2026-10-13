## ADDED Requirements

### Requirement: The tested package set is computed, not asserted

被测面（本机门控跑哪些 Rust 包、每个 `test --suite` 跑哪些包）SHALL 由实参**解析**成包名集合，
再 SHALL 做包含关系核验，禁止仅以「某个 marker 在源码里逐字存在」充当覆盖证明。每个 suite 的包集
SHALL 被门控闭包里**单一条** `cargo test` 命令的覆盖集包含（不得由若干条命令的并集拼出）；
每个 workspace 成员 SHALL 被这些命令的并集覆盖；每条 suite 引用的 `-p` 值 SHALL 落在一个真实成员的
包名上；CI 执行面调用的每条 `cargo xtask` 命令 SHALL 在本机命令面在册。三处集合 SHALL 各自
独立校验，任一处缺失或不可解析 SHALL 具名失败而非静默降级。

#### Scenario: A shrunken gate loses suites and members by name

- **WHEN** 本机门控的全 workspace 测试命令被改成只测试少数几个包
- **THEN** 检查失败，并逐一点名每个失去覆盖的 suite 包集与每个不再被跑到的成员包名

#### Scenario: A package reference that resolves to nothing fails

- **WHEN** 某个 suite 的 `-p` 值不是任何 workspace 成员的包名，或 `--exclude` 的标识符无法解析
- **THEN** 检查失败并具名该值——不得把「没给它输入」当成「空集」静默通过

#### Scenario: A CI command that does not exist is named

- **WHEN** CI 执行面调用了一条本机命令面没有注册的命令
- **THEN** 检查失败并具名该命令的签名

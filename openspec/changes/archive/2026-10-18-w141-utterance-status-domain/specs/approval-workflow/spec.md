# approval-workflow Specification

## ADDED Requirements

### Requirement: The utterance status domain is declared in exactly one place

`utterances.status` 的取值域 SHALL 有且仅有一处声明（`nai-storage` 的 `utterance_status` 模块），
该列 MUST NOT 有第二处定义 —— 生产源码（承载这些 SQL 语句的模块）里 MUST NOT 出现
`status` 的裸数字字面量，SQL 里的每个数字 SHALL 由该模块的常量派生。
该列的分类 SHALL 是**闭世界**的：任意 `i64` MUST 归到一格，不存在「不知道该归到哪」这种第四种结果。

#### Scenario: A new bare literal is refused

- **WHEN** 有人在生产源码里新写一句带 status 数字的 SQL（例如 `SET status=11`）
- **THEN** 判据 MUST 失败并指出是哪个文件，逼他去声明处给那个值一个名字与归类

#### Scenario: Every value has a class

- **WHEN** 对任意取值（含 `i64` 两端）做分类
- **THEN** 每一格分类都真的被某个取值命中，且不存在无归类的取值

### Requirement: Usability is an allow list, never a block list

判定一个 utterance 是否**可用** SHALL 使用白名单（只认已声明的取值），
MUST NOT 使用「不在黑名单里」这种写法 —— 后者会把「还没人声明过的取值」默认判成可用，
而可用是危险方向（可用 ⇒ 可播出 / 可外发）。未声明过的取值 MUST 判为不可用。

#### Scenario: An undeclared status is not approvable

- **WHEN** 一条 utterance 的 status 从未被声明为可用（例如只受级联保护的 `11`、未分配的 `7..=10`、越出已声明域的值）
- **THEN** 它 MUST NOT 作为 `approve` 的成功输入

#### Scenario: Declared usable statuses keep working

- **WHEN** 一条 utterance 处于待批准带或已批准
- **THEN** 它仍 MUST 可以走完批准流程（白名单化不得把已声明的取值挡在外面）

#### Scenario: The SQL guard and the Rust predicate agree

- **WHEN** 逐值比较 SQL 的可用性守卫与 Rust 的可用性谓词
- **THEN** 两者 MUST 完全一致；任何一侧绕开派生链另写一份都 MUST 被判据挡下

### Requirement: Cascade invalidation protects exactly the band it names

级联失效 SHALL 只保护它点名的那一带（级联保护带），带内的行 MUST NOT 被改写，带外的一律改写成已失效。

#### Scenario: A row inside the protected band survives

- **WHEN** 一次全局级联失效作用到一条处于保护带内的 utterance
- **THEN** 它的 status MUST 保持不变

#### Scenario: A row outside the protected band is invalidated

- **WHEN** 一次全局级联失效作用到一条待批准或已批准的 utterance
- **THEN** 它的 status MUST 变成已失效

### Requirement: Values nobody writes are named rather than guessed

取值域里**没有写入者**的值 SHALL 被具名归类为「只受保护」或「未分配」，
MUST NOT 因「它看起来像终态」而被并进终态，也 MUST NOT 因「它看起来像活跃」而被判为可用。

#### Scenario: The undeclared value is not quietly treated as terminal

- **WHEN** 审查 `11` 的归类
- **THEN** 它 MUST 是「只受级联保护」这一格，而**不是**终态 —— 它在两处历史口径里的归属本就矛盾，
  保留这个矛盾 MUST 优先于替它选一边

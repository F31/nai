## ADDED Requirements

### Requirement: A test file about to be committed must already be registered
「未跟踪且未被 git 忽略」的 test-like 文件——即 `git add -A` 会收进提交、而 `.git/index` 里还没有的那些——MUST 已被某个登记站点覆盖。未覆盖者 SHALL 具名报出，并在报错里给出修法。

#### Scenario: An unregistered new test file
- **WHEN** 工作区里有一个 `tests/contracts/new-thing.test.mjs`，它未跟踪，且没有任何站点的 marker 命中它
- **THEN** 判据具名该路径，指出闭世界只读索引、这个文件在本机不会跑、CI 一 checkout 就会红

#### Scenario: Registered before committing
- **WHEN** 同一个文件已经被某个站点的 marker 命中（哪怕它还没 `git add`）
- **THEN** 判据 MUST NOT 报错——登记与提交是两件事，前者才是本判据的对象

### Requirement: The pending half is computed by git, and an unreadable set fails
补集 SHALL 由 `git ls-files --others --exclude-standard` 算出，MUST NOT 由本模块自己实现忽略语义。读取失败 SHALL 以具名原因失败，MUST NOT 退化成空集。

#### Scenario: git cannot answer
- **WHEN** 宿主无法派生 git 进程，或工作目录不是一个仓库
- **THEN** 读取器返回 `ok: false` 与具名原因；调用方 MUST 把它报成失败，因为「不知道」不是「没有」

#### Scenario: The ignore rules are git's
- **WHEN** 一个 test-like 文件被 `.gitignore` 忽略
- **THEN** 它不在补集里；判据 MUST NOT 要求给它登记站点

### Requirement: The pending set and the index must not overlap
补集与索引集 SHALL 不相交；交集非空 SHALL 具名失败。两条判据（已跟踪的覆盖、未跟踪的登记）合起来 MUST 恰好覆盖提交集一次。

#### Scenario: The caller passes the whole commit set
- **WHEN** 调用方把「已跟踪 ∪ 未跟踪」整个塞进 `present`
- **THEN** 判据具名重叠项，因为同一批文件被两条判据判了两遍

#### Scenario: The scan is counted, not assumed
- **WHEN** 补集为空（没有任何待提交的新测试文件）
- **THEN** 判据通过，但「扫到的条数」与「清扫谓词不惰性」两条前提 MUST 先各自成立，否则这个通过是空的

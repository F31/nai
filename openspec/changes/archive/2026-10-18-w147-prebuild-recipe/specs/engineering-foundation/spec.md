# Spec delta: W147 — 预构建处方必须与门控测试步同批单元

## ADDED Requirements

### Requirement: The documented prebuild recipe matches the gate's test steps, in both directions

`docs/engineering.md` 的标记块内每条预构建命令 SHALL 等于门控闭包里某条测试步加**恰好一个**
`--no-run`，且每条门控测试步 MUST 有且只有一条对应命令。标记块缺失、重复、为空，或块内出现
非 `cargo` 行、旗标集不一致、命令重复，MUST 各自具名失败并说明是哪一侧对不上。

#### Scenario: A recipe line loses --no-run

- **WHEN** 处方里某条命令少了 `--no-run`（它就是门控那一步本身，不是预构建）
- **THEN** 判据具名失败，说明该行携带 `--no-run` 的次数必须恰为一次；
  MUST NOT 因为「这条命令确实存在」而通过——它编的是另一套单元

#### Scenario: A recipe line changes one flag

- **WHEN** 处方里某条命令换掉一个旗标或它的取值（例如把 `--exclude` 改成 `--skip`）
- **THEN** 判据具名失败并指出这条命令匹配不上门控的任何测试步；同时 MUST NOT 漏报**它原本对应的
  那一步**（「多了不该有的」与「少了该有的」是两侧，缺一侧就只报了一半）

#### Scenario: The gate changes its coverage shape

- **WHEN** 门控把 `--workspace` 换成 `-p <一个包>`，而处方仍写着 `--workspace`
- **THEN** 判据具名失败并指出门控这条测试步没有对应命令；这层信号 MUST NOT 与「包集合覆盖缩小」
  混为一谈——两件事各有各的判据，一侧红了不代表另一侧被看见

#### Scenario: The block cannot be located

- **WHEN** 标记块缺失、被重复声明、或块内一条命令也没有
- **THEN** 三种情形**分别**具名失败；「找不到处方」MUST NOT 退化成「处方为空」，
  也不能因为别的判据全绿而静默通过

#### Scenario: Flag order is not the judged property

- **WHEN** 处方把同一组旗标按另一种顺序书写（门控写 `--workspace --locked --exclude`，
  文档写 `--workspace --exclude … --locked`）
- **THEN** 判据 MUST 通过：顺序不是被判断的对象。反方向 MUST 成立——多一条同样的命令即失败，
  因为一条命令覆盖不了两条测试步

#### Scenario: The verdict is not the gate's own prebuilding

- **WHEN** 门控闭包里出现 `--no-run`
- **THEN** 判据具名失败：预构建是**手工步骤**，门控自己预构建等于同一个决定有两处定义

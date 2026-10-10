// §30.1 UI 层的断言登记。设计文档为这一层列了四条核心断言：
//
//   | UI | tests/ui | 审批失效、模式切换条件、静音 ACK、升级与回滚提示 |
//
// 本文件把那份清单**当成真值**放在测试里，并要求代码的声明与之逐条相等：
// 既证明每条都被登记，也防止将来偷偷加一条未登记的断言（或悄悄删掉一条看不见的）。
// 对尚未实现的断言，断的是「登记为未实现且点名了缺失前置」——**不是**断言它通过。
import test from 'node:test';
import assert from 'node:assert/strict';
import { UI_ASSERTIONS, UI_ASSERTION_LABELS, assertionById } from '../../apps/desktop/src/console.ts';

// 设计 §30.1「测试分层」表中 UI 一行的核心断言，顺序照抄。
const DESIGNED = ['审批失效', '模式切换条件', '静音 ACK', '升级与回滚提示'];

test('the declared assertion labels are exactly the four the design lists', () => {
  assert.deepEqual([...UI_ASSERTION_LABELS], DESIGNED);
  assert.deepEqual(UI_ASSERTIONS.map((entry) => entry.label), DESIGNED);
  assert.equal(UI_ASSERTIONS.length, DESIGNED.length);
});

test('every declared assertion has a definite status, and no id is invented', () => {
  const ids = UI_ASSERTIONS.map((entry) => entry.id);
  assert.equal(new Set(ids).size, ids.length);
  for (const entry of UI_ASSERTIONS) {
    assert.ok(['implemented', 'unimplemented'].includes(entry.status), entry.id);
    assert.equal(assertionById(entry.id)?.label, entry.label);
  }
});

test('an unimplemented assertion names the prerequisite it lacks, an implemented one claims none', () => {
  for (const entry of UI_ASSERTIONS) {
    if (entry.status === 'implemented') {
      assert.equal(entry.missing, undefined, `${entry.label} 已实现，不应声明缺失前置`);
      continue;
    }
    assert.ok(entry.missing && entry.missing.trim().length > 0, `${entry.label} 必须点名缺失前置`);
    assert.match(entry.missing, /未实现|无/, entry.label);
  }
});

test('the two assertions whose surfaces do not exist yet are registered as unimplemented', () => {
  // 模式切换条件的界面路径需要 POST /sessions/{id}/mode（契约已定义、控制面未实现）；
  // 升级与回滚提示需要控制面暴露升级状态来源。两者都不存在，故只能登记为未实现。
  for (const [id, anchor] of [['mode_switch_conditions', /sessions\/\{id\}\/mode/], ['upgrade_rollback_prompt', /nai-upgrade-core/]]) {
    const entry = assertionById(id);
    assert.equal(entry?.status, 'unimplemented');
    assert.match(entry?.missing ?? '', anchor);
  }
  // 本轮真正实现并被下面断言覆盖的两条。
  assert.equal(assertionById('approval_invalidation')?.status, 'implemented');
  assert.equal(assertionById('mute_ack_honesty')?.status, 'implemented');
});

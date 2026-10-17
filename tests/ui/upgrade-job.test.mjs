// §30.1 第 4 条「升级与回滚提示」的**状态**一侧（W138）。
//
// 与 `upgrade-plan.test.mjs` 的分工：那一份判**成本从哪来**，这一份判**进行到哪一步**。
// 两层都不构成 O02 的完整 UI 验收：`apps/desktop` 仍无 DOM，「提示」还没有真的画出来；
// 而且执行与回滚当前具名拒绝（缺平台安装器），所以这里判的是**读者**，不是一个能跑的升级。
import test from 'node:test';
import assert from 'node:assert/strict';
import { ControlClient, JOB_STEPS, parseJobView } from '../../apps/desktop/src/control.ts';
import { JOB_STEP_LABELS, ROLLBACK_UNAVAILABLE_NOTE, jobStatusNote } from '../../apps/desktop/src/upgrade.ts';

const TOKEN = 'test_only_credential_01234567890123456789';

const view = (overrides = {}) => ({
  job_id: 'tx-1',
  kind: 'upgrade',
  step: 'migrated',
  terminal: false,
  from_manifest: 'manifest-1.2.3',
  to_manifest: 'manifest-1.3.0',
  ...overrides,
});

test('作业状态信封认可契约声明的形状', () => {
  const parsed = parseJobView(view({ backup_ref: 'backup-1' }));
  assert.equal(parsed.job_id, 'tx-1');
  assert.equal(parsed.step, 'migrated');
  assert.equal(parsed.terminal, false);
  assert.equal(parsed.backup_ref, 'backup-1');
});

test('作业状态信封拒绝契约之外的字段', () => {
  // 契约是 additionalProperties:false。服务端会拒；客户端也必须拒，
  // 否则两侧对形状的理解会悄悄分家——一侧加了 rollback_allowed，另一侧照收不误。
  for (const extra of [{ rollback_allowed: true }, { step_label: '已迁移' }, { percent: 40 }]) {
    assert.throws(() => parseJobView(view(extra)), /含未知字段/);
  }
});

test('作业状态信封拒绝缺字段', () => {
  for (const key of ['job_id', 'kind', 'step', 'terminal', 'from_manifest', 'to_manifest']) {
    const broken = view();
    delete broken[key];
    assert.throws(() => parseJobView(broken), /缺少字段/);
  }
});

test('未知步骤被具名拒绝而不是猜成某个已知步骤', () => {
  // 这是本文件最重要的一条：把 unknown 读成 prepared 会让界面显示一个**看起来像事实**的进度。
  assert.throws(() => parseJobView(view({ step: 'verifying' })), /未知步骤.*verifying/);
  assert.throws(() => parseJobView(view({ step: 'Committed' })), /未知步骤/);
  assert.throws(() => parseJobView(view({ step: '' })), /未知步骤/);
});

test('可选的两列只认「缺省」，不认空串', () => {
  // 契约里 backup_ref / error_code 是可选的；填一个空串是替 journal 编一个它没有的值。
  assert.equal(parseJobView(view()).backup_ref, undefined);
  assert.throws(() => parseJobView(view({ backup_ref: '' })), /backup_ref/);
  assert.throws(() => parseJobView(view({ error_code: '' })), /error_code/);
});

test('五个步骤各有标签，且标签表是闭世界', () => {
  // 两集相等：少一个 ⇒ 某个真实状态说不出话；多一个 ⇒ 有个标签永远用不上。
  assert.deepEqual(Object.keys(JOB_STEP_LABELS).sort(), [...JOB_STEPS].sort());
  for (const step of JOB_STEPS) {
    const note = jobStatusNote({ job_id: 'tx-1', step, terminal: step === 'committed' || step === 'rolled_back' });
    assert.match(note, /tx-1/);
    assert.ok(note.includes(JOB_STEP_LABELS[step]), `${step}: ${note}`);
    // 标签不能是空串：空标签等于「有这五态」这句话没兑现。
    assert.notEqual(JOB_STEP_LABELS[step].trim(), '');
  }
});

test('状态说明把终态与未竟分开说，并对未知步骤报缺口', () => {
  assert.match(jobStatusNote({ job_id: 'tx-1', step: 'committed', terminal: true }), /已到终态/);
  assert.match(jobStatusNote({ job_id: 'tx-1', step: 'migrated', terminal: false }), /尚未到终态/);
  // 不认识的步骤有可表示的出口（类型上也允许），且明确不做猜测。
  const unknown = jobStatusNote({ job_id: 'tx-1', step: 'verifying', terminal: false });
  assert.match(unknown, /未知步骤/);
  assert.match(unknown, /不做猜测/);
});

test('回退边界是一句可断言的事实，不是空白', () => {
  // 执行/回滚当前具名拒绝（503，缺平台安装器）。界面 MUST 说出来，
  // 否则「能读出步骤」会被读成「能改变步骤」。
  assert.match(ROLLBACK_UNAVAILABLE_NOTE, /安装器/);
  assert.match(ROLLBACK_UNAVAILABLE_NOTE, /只读/);
});

test('客户端按 id 读 /jobs/{id}，不猜状态码含义', async () => {
  const calls = [];
  const client = new ControlClient('http://127.0.0.1:4318', TOKEN, async (url, options) => {
    calls.push({ url, options });
    return new Response(JSON.stringify(view()));
  });
  const result = await client.fetchJob('tx-1');
  assert.equal(result.job_id, 'tx-1');
  assert.equal(calls.length, 1);
  assert.match(calls[0].url, /\/api\/v1\/jobs\/tx-1$/);
  // 读状态是 GET，且不带请求体：状态由存储给出，MUST NOT 由调用方给。
  assert.equal(calls[0].options.method, 'GET');
  assert.equal(calls[0].options.body, undefined);
  // 空 id 连请求都不该发出。
  await assert.rejects(() => client.fetchJob(''), /非空字符串/);
  assert.equal(calls.length, 1);
});

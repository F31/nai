// §30.1 UI 层断言之一：**审批失效**，以及批准的两步确认与命令形状。
//
// 这一层测的是判据与消息，不是渲染：审批动作的编排被抽到 `console.ts`（不含 DOM），
// 因此「失效时不发请求」是一条**可断言**的性质，而不是只能靠眼睛在浏览器里看。
import test from 'node:test';
import assert from 'node:assert/strict';
import { ControlClient, parseApprovals } from '../../apps/desktop/src/control.ts';
import {
  APPROVE_CONFIRM_PROMPT, FINAL_TEXT_GAP, IDLE_APPROVAL_FLOW, approvalFreshness, approveConfirmPrompt,
  candidateText, describeCommandResult, runApprovalDecision,
} from '../../apps/desktop/src/console.ts';

const HASH_A = 'a'.repeat(64);
const HASH_B = 'b'.repeat(64);
const TOKEN = 'test_only_credential_01234567890123456789';
const NOW = 1_000_000;

const runtime = (epoch = '7') => ({
  resource_id: 's', revision: '9007199254740993', status: 'control_connected', safe_message: 'test-only',
  device_id: 'd', last_command_seq: '9007199254740993',
  session: {
    session_id: 's', generation_epoch: epoch, state_revision: '9007199254740993',
    lifecycle: 'running', mode: 'assisted', media_state: 'playing',
  },
});
const queue = (items, status = 'approvals_ok') => ({
  resource_id: 's', revision: 'approvals', status, safe_message: '当前有效的待审批候选元信息；不授予播放权限。', items,
});
const candidate = (id, expiresAt, evidence = '2') => ({
  id, revision: '', status: 'awaiting_approval',
  encoded_contract_ref: `approval:candidate_hash:${id}`,
  ...(expiresAt === undefined ? {} : { expires_at_utc_ms: String(expiresAt) }),
  evidence_count: evidence,
});
// 替身传输面：`queues[0]` 是动作前重读到的实时队列，`queues[1]`（若有）是提交后刷新的队列。
const transportStub = (queues, epoch = '7') => {
  const calls = { status: 0, approvals: 0, decideApproval: 0 };
  let index = 0;
  const taken = [];
  return {
    calls, taken,
    transport: {
      async status() { calls.status += 1; return runtime(epoch); },
      async approvals() { const q = queues[Math.min(index, queues.length - 1)]; index += 1; calls.approvals += 1; return q; },
      async decideApproval(view, item, action, id) {
        calls.decideApproval += 1; taken.push({ view, item, action, id });
        return { command_id: id, executed: true, generation_epoch: epoch, state_revision: '9007199254740994' };
      },
    },
  };
};

test('an expired candidate is refused and nothing is sent', async () => {
  const stale = candidate(HASH_A, NOW - 1);
  const { calls, transport } = transportStub([queue([stale])]);
  const outcome = await runApprovalDecision(
    transport, stale, 'approve', { epoch: '7', ids: [HASH_A] }, IDLE_APPROVAL_FLOW, 'cmd-1', NOW,
  );
  assert.equal(outcome.kind, 'refused');
  assert.match(outcome.message, /已过期/);
  assert.match(outcome.message, /未发送请求/);
  assert.equal(calls.decideApproval, 0);
  assert.deepEqual(outcome.flow, IDLE_APPROVAL_FLOW);
});

test('a candidate without an expiry is treated as unusable, not as valid', async () => {
  const noExpiry = candidate(HASH_A, undefined);
  const { calls, transport } = transportStub([queue([noExpiry])]);
  const outcome = await runApprovalDecision(
    transport, noExpiry, 'approve', { epoch: '7', ids: [HASH_A] }, IDLE_APPROVAL_FLOW, 'cmd-2', NOW,
  );
  assert.equal(outcome.kind, 'refused');
  assert.match(outcome.message, /未给出有效期/);
  assert.equal(calls.decideApproval, 0);
});

test('a session that advanced epoch since the panel listed the queue refuses the action', async () => {
  const live = candidate(HASH_A, NOW + 60_000);
  const { calls, transport } = transportStub([queue([live])], '8');
  const outcome = await runApprovalDecision(
    transport, live, 'approve', { epoch: '7', ids: [HASH_A] }, IDLE_APPROVAL_FLOW, 'cmd-3', NOW,
  );
  assert.equal(outcome.kind, 'refused');
  assert.match(outcome.message, /会话已换代/);
  assert.equal(calls.decideApproval, 0);
});

test('a candidate that is no longer listed refuses the action', async () => {
  const gone = candidate(HASH_A, NOW + 60_000);
  const { calls, transport } = transportStub([queue([candidate(HASH_B, NOW + 60_000)])]);
  const outcome = await runApprovalDecision(
    transport, gone, 'approve', { epoch: '7', ids: [HASH_A] }, IDLE_APPROVAL_FLOW, 'cmd-4', NOW,
  );
  assert.equal(outcome.kind, 'refused');
  assert.match(outcome.message, /不在最新队列/);
  assert.equal(calls.decideApproval, 0);
});

test('the first click only arms; the second click submits the same candidate', async () => {
  const fresh = candidate(HASH_A, NOW + 60_000);
  const listed = { epoch: '7', ids: [HASH_A] };
  const first = transportStub([queue([fresh])]);
  const armed = await runApprovalDecision(
    first.transport, fresh, 'approve', listed, IDLE_APPROVAL_FLOW, 'cmd-5', NOW,
  );
  assert.equal(armed.kind, 'armed');
  assert.equal(first.calls.decideApproval, 0);
  assert.equal(armed.flow.armed, HASH_A);
  assert.equal(armed.message, APPROVE_CONFIRM_PROMPT);
  // 确认面必须声明**看不见的东西**，而不是让操作员以为已经看到了将要播报的文本。
  assert.ok(armed.message.includes(FINAL_TEXT_GAP));

  const second = transportStub([queue([fresh]), queue([])]);
  const submitted = await runApprovalDecision(
    second.transport, fresh, 'approve', listed, armed.flow, 'cmd-6', NOW,
  );
  assert.equal(submitted.kind, 'submitted');
  assert.equal(second.calls.decideApproval, 1);
  assert.equal(second.taken[0].action, 'approve');
  assert.equal(second.taken[0].id, 'cmd-6');
  assert.deepEqual(submitted.flow, IDLE_APPROVAL_FLOW);
});

test('reject is the safe direction and needs no second confirmation', async () => {
  const fresh = candidate(HASH_A, NOW + 60_000);
  const { calls, taken, transport } = transportStub([queue([fresh]), queue([])]);
  const outcome = await runApprovalDecision(
    transport, fresh, 'reject', { epoch: '7', ids: [HASH_A] }, IDLE_APPROVAL_FLOW, 'cmd-7', NOW,
  );
  assert.equal(outcome.kind, 'submitted');
  assert.equal(calls.decideApproval, 1);
  assert.equal(taken[0].action, 'reject');
});

test('the freshness predicate reports the first applicable reason', () => {
  const fresh = candidate(HASH_A, NOW + 60_000);
  assert.deepEqual(approvalFreshness(fresh, { epoch: '7', ids: [HASH_A] }, { epoch: '7', ids: [HASH_A] }, NOW), { fresh: true });
  assert.deepEqual(
    approvalFreshness(fresh, { epoch: '7', ids: [HASH_A] }, { epoch: '8', ids: [HASH_A] }, NOW),
    { fresh: false, reason: 'session_changed' },
  );
  assert.deepEqual(
    approvalFreshness(fresh, { epoch: '7', ids: [HASH_A] }, { epoch: '7', ids: [] }, NOW),
    { fresh: false, reason: 'not_listed' },
  );
  assert.deepEqual(
    approvalFreshness(fresh, { epoch: '7', ids: [HASH_A] }, { epoch: '7', ids: [HASH_A] }, NOW + 60_000),
    { fresh: false, reason: 'expired' },
  );
});

test('the approval command carries the candidate hash and session context verbatim', async () => {
  const calls = [];
  const client = new ControlClient('http://127.0.0.1:4318', TOKEN, async (url, options) => {
    calls.push({ url, options });
    if (options.method === 'GET') {
      return new Response(JSON.stringify(url.endsWith('/approvals') ? queue([candidate(HASH_A, NOW + 60_000)]) : runtime('7')));
    }
    return new Response(JSON.stringify({ command_id: 'cmd-9', executed: true, state_revision: '9007199254740994', generation_epoch: '9007199254740994' }));
  });
  const view = await client.status();
  const list = await client.approvals();
  assert.equal(list.items.length, 1);
  const ack = await client.decideApproval(view, list.items[0], 'approve', 'cmd-9');
  assert.equal(ack.generation_epoch, '9007199254740994');

  const body = JSON.parse(calls.at(-1).options.body);
  assert.equal(calls.at(-1).url, `http://127.0.0.1:4318/api/v1/approvals/${HASH_A}/approve`);
  assert.equal(body.candidate_hash, HASH_A);
  assert.equal(body.hash_algorithm, 'sha256-canonical-v1');
  assert.equal(body.seq, '9007199254740994');
  assert.equal(body.expected_state_revision, '9007199254740993');
  assert.equal(body.generation_epoch, '7');
  assert.equal(body.session_id, 's');
  assert.equal(body.device_id, 'd');
  assert.equal(body.ttl_ms, 4000);
  assert.equal(calls.at(-1).options.headers.Authorization, `Bearer ${TOKEN}`);
  assert.equal(calls.at(-1).url.includes(TOKEN), false);
});

test('a false ACK never becomes an approved candidate and no retry happens', async () => {
  let posts = 0;
  const client = new ControlClient('http://127.0.0.1:4318', TOKEN, async (url, options) => {
    if (options.method === 'GET') return new Response(JSON.stringify(url.endsWith('/approvals') ? queue([]) : runtime('7')));
    posts += 1;
    return new Response(JSON.stringify({ command_id: 'cmd-10', executed: false, state_revision: '1', generation_epoch: '1' }));
  });
  const view = await client.status();
  await assert.rejects(client.decideApproval(view, candidate(HASH_A, NOW + 60_000), 'approve', 'cmd-10'), /ACK/);
  assert.equal(posts, 1);
});

test('the approvals payload is parsed strictly or refused', async () => {
  assert.equal(parseApprovals(queue([candidate(HASH_A, NOW)])).items[0].id, HASH_A);
  for (const bad of [
    { resource_id: 's', revision: 'r', status: 'ok', safe_message: 'm', items: 'not-an-array' },
    { resource_id: 's', revision: 'r', status: 'ok', safe_message: 'm' },
    queue([{ id: 'short', revision: '', status: 'awaiting_approval' }]),
    queue([{ id: HASH_A, revision: '', status: 'awaiting_approval', expires_at_utc_ms: '18446744073709551616' }]),
    queue([{ id: HASH_A, revision: '', status: 'awaiting_approval', evidence_count: 3 }]),
    queue(Array.from({ length: 1025 }, () => candidate(HASH_A, NOW))),
  ]) {
    assert.throws(() => parseApprovals(bad), /结构无效/);
  }
});

test('a command ACK is reported as a control ACK, never as a hardware mute', () => {
  for (const action of ['mute', 'takeover']) {
    const message = describeCommandResult(action);
    assert.match(message, /未取得硬件静音/);
    assert.ok(!/已静音|已停止|硬件已/.test(message), message);
  }
  assert.match(describeCommandResult('approve'), /未取得硬件静音/);
});

// 设计 §662（O02）与 §425（T-14）：审批按钮显示**最终文本**，不能只显示「同意模型回复」。
test('the approval surface shows the candidate text and never substitutes the hash', async () => {
  const text = '满 300 减 50，今晚 8 点上架';
  const withText = { ...candidate(HASH_A, NOW + 60_000), display_text: text };
  const shown = candidateText(withText);
  assert.equal(shown.present, true);
  assert.equal(shown.label, text);
  const prompt = approveConfirmPrompt(withText);
  assert.ok(prompt.includes(text), prompt);
  assert.ok(!prompt.includes(HASH_A), prompt);

  const { transport } = transportStub([queue([withText])]);
  const armed = await runApprovalDecision(
    transport, withText, 'approve', { epoch: '7', ids: [HASH_A] }, IDLE_APPROVAL_FLOW, 'cmd-11', NOW,
  );
  assert.equal(armed.kind, 'armed');
  assert.ok(armed.message.includes(text), armed.message);
  assert.ok(!armed.message.includes(HASH_A), armed.message);
});

test('a candidate with no stored text is named as missing, never as empty content', () => {
  const absent = candidate(HASH_B, NOW + 60_000);
  const missing = candidateText(absent);
  assert.equal(missing.present, false);
  assert.equal(missing.label, FINAL_TEXT_GAP);
  assert.ok(!missing.label.includes(HASH_B));
  assert.equal(approveConfirmPrompt(absent), APPROVE_CONFIRM_PROMPT);
  // 存储侧迁移 0006 的默认空串表示「该行早于该字段」，不是「内容为空」。
  const empty = { ...absent, display_text: '' };
  const blank = candidateText(empty);
  assert.equal(blank.present, false);
  assert.equal(blank.label, FINAL_TEXT_GAP);
  assert.notEqual(blank.label, '');
});

test('a display text of the wrong shape is refused by the strict parser', () => {
  for (const bad of [3, {}, [], true]) {
    assert.throws(() => parseApprovals(queue([{ ...candidate(HASH_A, NOW), display_text: bad }])), /结构无效/);
  }
  const parsed = parseApprovals(queue([{ ...candidate(HASH_A, NOW), display_text: '文本' }]));
  assert.equal(parsed.items[0].display_text, '文本');
});

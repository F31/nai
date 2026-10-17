// O05 的**来源**一侧（§1330「升级成本显示」；W137）：成本从控制面出来以后，
// 桌面端怎么把它接住。
//
// 与 `upgrade-cost.test.mjs` 的分工：那一份判**呈现口径**（未知≠0、部分和≠总额），
// 这一份判**信封**（形状、身份、失败方向）与「成本解析只有一处」。
// 两层都不构成 O05 的完整 UI 验收：真实渲染与生产接线仍未做。
import test from 'node:test';
import assert from 'node:assert/strict';
import { ControlClient, parseUpgradePlan } from '../../apps/desktop/src/control.ts';
import {
  COST_DECLARED_BASIS_NOTE, costRows, parseCostEstimate, totalPresentation,
} from '../../apps/desktop/src/upgrade.ts';

const TOKEN = 'test_only_credential_01234567890123456789';

const cost = (components) => ({ currency: 'CNY', components });

const KNOWN = cost({
  cloud: { known: 1000 },
  download: { known: 200 },
  storage: { known: 0 },
  runtime_delta: { known: 30 },
});

const PARTIAL = cost({
  cloud: { known: 1000 },
  download: { unknown: '包清单未声明该分项' },
  storage: { unknown: '包清单未声明该分项' },
  runtime_delta: { known: 30 },
});

const plan = (overrides = {}) => ({
  package_id: 'plugin.demo', package_version: '1.2.3', cost: KNOWN, ...overrides,
});

test('升级计划信封只认三个键', () => {
  const parsed = parseUpgradePlan(plan());
  assert.equal(parsed.package_id, 'plugin.demo');
  assert.equal(parsed.package_version, '1.2.3');
  assert.deepEqual(parsed.cost, parseCostEstimate(KNOWN));
});

test('多一个看起来方便的字段就被拒绝，而不是默默容忍', () => {
  // 契约是 additionalProperties:false。服务端会拒；客户端也必须拒，
  // 否则两边对「形状」的理解会悄悄分家——一侧加了 total，另一侧照收不误。
  for (const extra of [
    { total: { known: 1230 } },
    { currency: 'CNY' },
    { manifest: {} },
  ]) {
    assert.throws(() => parseUpgradePlan(plan(extra)), /含未知字段/);
  }
});

test('缺键与非法身份各具名拒绝', () => {
  const { cost: _cost, ...withoutCost } = plan();
  assert.throws(() => parseUpgradePlan(withoutCost), /缺少字段 cost/);
  const { package_id: _id, ...withoutId } = plan();
  assert.throws(() => parseUpgradePlan(withoutId), /缺少字段 package_id/);
  assert.throws(() => parseUpgradePlan(plan({ package_id: '' })), /package_id 必须是非空字符串/);
  assert.throws(() => parseUpgradePlan(plan({ package_version: '' })), /package_version 必须是非空字符串/);
  for (const bad of [null, [], 'CNY', 7]) {
    assert.throws(() => parseUpgradePlan(bad), /不是对象/);
  }
});

test('成本解析只有一处：信封不重判金额，坏成本由那一处拒绝', () => {
  // 一致性：信封里过的成本，与直接过 `parseCostEstimate` 的结果**同形**。
  assert.deepEqual(parseUpgradePlan(plan({ cost: PARTIAL })).cost, parseCostEstimate(PARTIAL));
  // 判别性：把成本解析退化成「透传」，这一条会红——信封自己不判金额。
  const broken = cost({
    cloud: { known: 1000 }, download: { known: 1 }, storage: { known: 1 },
  });
  assert.throws(() => parseUpgradePlan(plan({ cost: broken })), /缺少分项 runtime_delta/);
});

test('未知分项在计划里呈现为原因，绝不呈现为 0', () => {
  const rows = costRows(parseUpgradePlan(plan({ cost: PARTIAL })).cost);
  for (const row of rows.filter((candidate) => !candidate.known)) {
    assert.match(row.text, /未知（.+）/, row.text);
    assert.doesNotMatch(row.text, /：0 /, row.text);
  }
});

test('含未知分项时总额是下界加未知计数，不是总额', () => {
  const total = totalPresentation(parseUpgradePlan(plan({ cost: PARTIAL })).cost);
  assert.equal(total.complete, false);
  assert.equal(total.floor, 1030);
  assert.equal(total.unknown, 2);
});

test('全已知时总额是合计', () => {
  const total = totalPresentation(parseUpgradePlan(plan()).cost);
  assert.equal(total.complete, true);
  assert.equal(total.floor, 1230);
  assert.equal(total.unknown, 0);
});

test('金额来源的边界与金额一同呈现：它没说「要花多少钱」', () => {
  // 内容锚点，不是「非空即可」：这条说明的用处正是把「厂商声明」与「已核实收费」分开。
  assert.match(COST_DECLARED_BASIS_NOTE, /包清单/);
  assert.match(COST_DECLARED_BASIS_NOTE, /未验签/);
  assert.match(COST_DECLARED_BASIS_NOTE, /升级事务/);
});

test('客户端把清单交给 /upgrades/plan，不自己算成本', async () => {
  const calls = [];
  const client = new ControlClient('http://127.0.0.1:4318', TOKEN, async (url, options) => {
    calls.push({ url, options });
    return new Response(JSON.stringify(plan()));
  });
  const manifest = { package_id: 'plugin.demo', cost: { currency: 'CNY' } };
  const result = await client.planUpgrade(manifest);
  assert.equal(result.package_id, 'plugin.demo');
  assert.equal(calls.length, 1);
  assert.match(calls[0].url, /\/api\/v1\/upgrades\/plan$/);
  assert.equal(calls[0].options.method, 'POST');
  assert.deepEqual(JSON.parse(calls[0].options.body), { manifest });
  // 请求里只有清单：额度/单价由服务端从清单算，前端 MUST NOT 顺手递一个价钱过去。
  assert.equal(Object.keys(JSON.parse(calls[0].options.body)).length, 1);
});

test('不合规的计划响应被拒绝且不返回半成品', async () => {
  const client = new ControlClient('http://127.0.0.1:4318', TOKEN, async () =>
    new Response(JSON.stringify(plan({ total: { known: 1 } }))));
  await assert.rejects(client.planUpgrade({}), /含未知字段/);
});

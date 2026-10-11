// O05（§1330「升级成本显示：云资源、下载、存储与预估运行增量准确列示」）的呈现口径断言。
//
// 这一层测的是**呈现口径**，不是渲染：判据抽到 `apps/desktop/src/upgrade.ts`（不含 DOM），
// 于是「未知不等于零」「部分和不当作总额」成为**可断言**的性质。
// 它**不**构成 O05 的完整 UI 验收（真实渲染与生产接线未做）。
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  COST_COMPONENTS, COST_PARTIAL_NOTE, COST_UNITS_NOTE,
  costRows, parseCostEstimate, totalPresentation,
} from '../../apps/desktop/src/upgrade.ts';

const payload = (overrides = {}) => ({
  currency: 'CNY',
  components: {
    cloud: { known: 1200 },
    download: { known: 0 },
    storage: { unknown: '目标盘容量未探测' },
    runtime_delta: { known: 300 },
    ...overrides,
  },
});

const parse = (value) => parseCostEstimate(value);

test('成本逐项列示四类，顺序与报告一致', () => {
  const estimate = parse(payload());
  const rows = costRows(estimate);
  assert.deepEqual(rows.map((row) => row.id), COST_COMPONENTS.map((component) => component.id));
  assert.deepEqual(rows.map((row) => row.label), ['云资源', '下载', '存储', '运行增量']);
});

test('已知分项呈现为金额与币种', () => {
  const rows = costRows(parse(payload()));
  const cloud = rows.find((row) => row.id === 'cloud');
  assert.equal(cloud.text, '云资源：1200 CNY');
  assert.equal(cloud.known, true);
});

test('显式的 0 是一个断言，被如实呈现为 0', () => {
  const rows = costRows(parse(payload()));
  const download = rows.find((row) => row.id === 'download');
  assert.equal(download.known, true);
  assert.equal(download.text, '下载：0 CNY');
});

test('未知分项呈现为原因，绝不呈现为 0', () => {
  const rows = costRows(parse(payload()));
  const storage = rows.find((row) => row.id === 'storage');
  assert.equal(storage.known, false);
  assert.equal(storage.text, '存储：未知（目标盘容量未探测）');
  // 关键性质：这一行里不出现数字 0。
  assert.ok(!/0/.test(storage.text), '未知分项不得呈现为 0');
});

test('全已知时总额是合计', () => {
  const estimate = parse(payload({ storage: { known: 500 } }));
  const total = totalPresentation(estimate);
  assert.equal(total.complete, true);
  assert.equal(total.unknown, 0);
  assert.equal(total.text, '合计 2000 CNY');
});

test('含未知分项时总额是下界加未知计数，不是总额', () => {
  const total = totalPresentation(parse(payload()));
  assert.equal(total.complete, false);
  assert.equal(total.unknown, 1);
  assert.equal(total.floor, 1500);
  assert.equal(total.text, `下界 1500 CNY（1 项未知，${COST_PARTIAL_NOTE}）`);
  assert.ok(!total.text.includes('合计'), '部分和不得被说成合计');
});

test('未知计数随未知分项数增长', () => {
  const estimate = parse(payload({ cloud: { unknown: '未取得报价' }, runtime_delta: { unknown: '依赖闭包未解析' } }));
  const total = totalPresentation(estimate);
  assert.equal(total.unknown, 3);
  assert.equal(total.floor, 0);
  assert.ok(total.text.includes('3 项未知'));
});

// ---------------------------------------------------------------- 严格解析

const rejects = (value, pattern, label) => {
  assert.throws(() => parse(value), (error) => {
    assert.ok(error instanceof Error, `${label}: 必须是 Error`);
    assert.match(error.message, pattern, `${label}: 消息须点名问题`);
    return true;
  }, label);
};

test('缺分项被具名拒绝', () => {
  const missing = payload();
  delete missing.components.storage;
  rejects(missing, /缺少分项 storage/, '缺 storage');
});

test('未知分项被具名拒绝', () => {
  rejects(payload({ bandwidth: { known: 1 } }), /未知的分项 bandwidth/, '多 bandwidth');
});

test('顶层未知字段被拒绝', () => {
  rejects({ ...payload(), total: 1500 }, /含未知字段 total/, '多 total');
});

test('负金额被拒绝', () => {
  rejects(payload({ cloud: { known: -1 } }), /非负安全整数/, '负金额');
});

test('非整数或非数字金额被拒绝', () => {
  rejects(payload({ cloud: { known: 1.5 } }), /非负安全整数/, '小数');
  rejects(payload({ cloud: { known: '1200' } }), /非负安全整数/, '字符串金额');
});

test('未知但原因为空或全空白被拒绝', () => {
  rejects(payload({ storage: { unknown: '' } }), /未说明原因/, '空原因');
  rejects(payload({ storage: { unknown: '   ' } }), /未说明原因/, '全空白原因');
});

test('同时给出 known 与 unknown 被拒绝', () => {
  rejects(payload({ storage: { known: 1, unknown: '理由' } }), /恰好给出 known 或 unknown 之一/, '两者并存');
});

test('分项内未知字段被拒绝', () => {
  rejects(payload({ storage: { known: 1, currency: 'CNY' } }), /含未知字段 currency/, '分项多字段');
});

test('非法货币代码被拒绝', () => {
  rejects({ currency: 'cny', components: payload().components }, /三字母大写货币代码/, '小写');
  rejects({ currency: '', components: payload().components }, /三字母大写货币代码/, '空');
});

test('整体非对象被拒绝', () => {
  rejects([payload()], /不是对象/, '数组');
  rejects('{}', /不是对象/, '字符串');
  rejects(null, /不是对象/, 'null');
});

test('解析失败时不产出任何行', () => {
  let rows = [];
  try {
    const estimate = parse(payload({ cloud: { known: -1 } }));
    rows = costRows(estimate);
  } catch {
    rows = [];
  }
  assert.deepEqual(rows, []);
});

test('金额单位为最小货币单位这一点被说明', () => {
  assert.match(COST_UNITS_NOTE, /最小货币单位/);
});

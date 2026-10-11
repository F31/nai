import test from 'node:test';
import assert from 'node:assert/strict';
import { readRegistry, validateRegistry } from '../scripts/contracts/registry.mjs';

test('registry.yaml is machine-readable and internally consistent', async () => {
  const registry = await readRegistry();
  const errors = validateRegistry(registry);
  assert.deepEqual(errors, []);
  // Cover V3.7 §16 + §30.4 acceptance families and release lines.
  assert.ok(registry.acceptance.length >= 66, 'expected >=66 acceptance items');
  assert.ok(registry.mapping.d01_to_d14.D14, 'expected D14 mapping');
  const slice0 = registry.acceptance.filter(a => a.release_line === 'slice_0');
  assert.ok(['S01','S02','S03','S04','R01','R02','R03','R07','A01','A04'].every(id =>
    slice0.some(a => a.id === id)), 'Slice-0 exit acceptance missing');
  // The baseline's acceptance families (§39.5) must all be registered; the
  // extension/marketplace family M (§37.5, §38.10) is not optional.
  const byId = new Map(registry.acceptance.map(a => [a.id, a]));
  for (const n of Array.from({length: 18}, (_, i) => `M${String(i + 1).padStart(2, '0')}`)) {
    assert.ok(byId.has(n), `baseline acceptance ${n} is not registered`);
  }
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { readRegistry, readCatalogIds, validateRegistry } from '../scripts/contracts/registry.mjs';

test('registry.yaml is machine-readable and internally consistent', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  const errors = validateRegistry(registry, catalogIds);
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

test('contract references must resolve to the catalog or a named non-capability contract', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  // A dangling reference is what the gate exists for: before w52 the registry
  // pointed at six market.catalog@1 rows and a data.query@1 row while the
  // catalog contained none of them.
  const dangling = structuredClone(registry);
  const holder = dangling.acceptance.find(item => item.contract === 'data.query@1');
  assert.ok(holder, 'expected an acceptance referencing data.query@1');
  holder.contract = 'market.catalog@9';
  assert.ok(
    validateRegistry(dangling, catalogIds).some(error => error.includes('market.catalog@9')),
    'dangling contract reference must be reported',
  );
  // A draft satisfies the reference without changing any status.
  const draft = catalogIds.has('market.catalog@1');
  assert.ok(draft, 'market.catalog@1 must be declared in the catalog as a draft');
  const unchanged = structuredClone(registry);
  assert.deepEqual(validateRegistry(unchanged, catalogIds), []);
  assert.equal(
    unchanged.acceptance.find(item => item.contract === 'market.catalog@1').status,
    'pending',
    'a draft reference must not upgrade the referencing acceptance',
  );
  // The gate cannot be satisfied by simply not supplying the catalog.
  assert.ok(
    validateRegistry(registry, undefined).some(error => error.includes('required')),
    'omitting the catalog must itself be an error',
  );
});

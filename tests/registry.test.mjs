import test from 'node:test';
import assert from 'node:assert/strict';
import { readRegistry, readCatalogIds, validateRegistry } from '../scripts/contracts/registry.mjs';

test('registry.yaml is machine-readable and internally consistent', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  const errors = validateRegistry(registry, catalogIds);
  assert.deepEqual(errors, []);
  // Cover V3.8 §16 + §30.4 + §48.5 acceptance families and release lines.
  assert.ok(registry.acceptance.length >= 87, 'expected >=87 acceptance items');
  assert.ok(registry.mapping.d01_to_d14.D14, 'expected D14 mapping');
  const slice0 = registry.acceptance.filter(a => a.release_line === 'slice_0');
  assert.ok(['S01','S02','S03','S04','R01','R02','R03','R07','A01','A04'].every(id =>
    slice0.some(a => a.id === id)), 'Slice-0 exit acceptance missing');
  // Every family the baseline defines as a complete range must be registered in
  // full. The extension/marketplace family M (§37.5, §38.10) is not optional —
  // ADR 0042 decision 4 left it unregistered once and the omission was recorded
  // as a defect. The harness family H (V3.8 §40-§47, §48.5) starts from that
  // lesson rather than repeating it: "not started" reads as `pending`, never as
  // a missing row.
  const byId = new Map(registry.acceptance.map(a => [a.id, a]));
  for (const family of ['M', 'H']) {
    for (const n of Array.from({length: 18}, (_, i) => `${family}${String(i + 1).padStart(2, '0')}`)) {
      assert.ok(byId.has(n), `baseline acceptance ${n} is not registered`);
    }
  }
});

test('a family present with a hole, or absent, fails the gate and names the id', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  // The gate must not be satisfiable by "the family is non-empty": delete one
  // row (a hole) and the whole family (absent), and require both to be caught
  // by name.
  const holed = structuredClone(registry);
  holed.acceptance = holed.acceptance.filter(item => item.id !== 'H05');
  assert.ok(
    validateRegistry(holed, catalogIds).some(
      error => error.includes('H family incomplete') && error.includes('H05'),
    ),
    'a single missing id inside a registered family must be reported by name',
  );
  const absent = structuredClone(registry);
  absent.acceptance = absent.acceptance.filter(item => !/^H[0-9]{2}$/.test(item.id));
  const absentErrors = validateRegistry(absent, catalogIds);
  assert.ok(
    absentErrors.some(error => error.includes('H family incomplete') && error.includes('H01')),
    'an entirely absent family must be reported',
  );
  // Out-of-range ids inside a family are the other half of "complete range".
  const spurious = structuredClone(registry);
  spurious.acceptance.push({...spurious.acceptance[0], id: 'H19'});
  assert.ok(
    validateRegistry(spurious, catalogIds).some(error => error.includes('H19')),
    'an id outside the family range must be reported',
  );
  // M and H are counted separately: H's rows must not be able to cover an M hole.
  const mHoled = structuredClone(registry);
  mHoled.acceptance = mHoled.acceptance.filter(item => item.id !== 'M18');
  assert.ok(
    validateRegistry(mHoled, catalogIds).some(error => error.includes('M18')),
    'a hole in M must not be masked by a fully populated H family',
  );
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

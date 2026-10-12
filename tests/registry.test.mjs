import test from 'node:test';
import assert from 'node:assert/strict';
import { readRegistry, readCatalogIds, validateRegistry } from '../scripts/contracts/registry.mjs';

test('registry.yaml is machine-readable and internally consistent', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  const errors = validateRegistry(registry, catalogIds);
  assert.deepEqual(errors, []);
  // Cover V3.10 §16 + §30.4 + §48.5 + §51.3 + §57.3 acceptance families and
  // release lines.
  assert.ok(registry.acceptance.length >= 87, 'expected >=87 acceptance items');
  assert.ok(registry.mapping.d01_to_d14.D14, 'expected D14 mapping');
  const slice0 = registry.acceptance.filter(a => a.release_line === 'slice_0');
  assert.ok(['S01','S02','S03','S04','R01','R02','R03','R07','A01','A04'].every(id =>
    slice0.some(a => a.id === id)), 'Slice-0 exit acceptance missing');
  // Every family the baseline defines as a complete range must be registered in
  // full. The extension/marketplace family M (§37.5, §38.10) is not optional —
  // ADR 0042 decision 4 left it unregistered once and the omission was recorded
  // as a defect. The harness family H (V3.10 §40-§47, §48.5) starts from that
  // lesson rather than repeating it: "not started" reads as `pending`, never as
  // a missing row.
  const byId = new Map(registry.acceptance.map(a => [a.id, a]));
  for (const family of ['M', 'H']) {
    for (const n of Array.from({length: 18}, (_, i) => `${family}${String(i + 1).padStart(2, '0')}`)) {
      assert.ok(byId.has(n), `baseline acceptance ${n} is not registered`);
    }
  }
  // The V3.10 scope split has to be readable, not just asserted in prose: the
  // pack line holds exactly the §51.3 set, and none of those ids sits on a
  // first-release line (INV-33).
  const packLine = registry.acceptance.filter(a => a.release_line === 'capability_pack').map(a => a.id);
  assert.equal(packLine.length, 29, 'expected the V3.10 §51.3 capability-pack set');
  for (const id of ['M01', 'M18', 'H05', 'H14', 'H16']) {
    assert.ok(packLine.includes(id), `${id} must be on the capability_pack line`);
  }
  for (const id of ['H01', 'H02', 'H03', 'H04', 'H15', 'H17', 'H18', 'S05', 'B01']) {
    assert.ok(!packLine.includes(id), `${id} must stay on a first-release line`);
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

test('the capability-pack split cannot be emptied, promoted, or filled with stray ids', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  // Promoting a pack id back onto a first-release line is exactly the drift
  // V3.10 closed: before this round M01-M18 were counted as R1 acceptance, which
  // is where the "first release is too wide" reading came from.
  const promoted = structuredClone(registry);
  promoted.acceptance.find(item => item.id === 'M01').release_line = 'r1';
  assert.ok(
    validateRegistry(promoted, catalogIds).some(
      error => error.includes('INV-33') && error.includes('M01=r1'),
    ),
    'a capability-pack id back on r1 must be reported by name',
  );
  // The other direction: parking a first-release item on the pack line silently
  // shrinks the first release without any visible removal.
  const demoted = structuredClone(registry);
  demoted.acceptance.find(item => item.id === 'H17').release_line = 'capability_pack';
  assert.ok(
    validateRegistry(demoted, catalogIds).some(
      error => error.includes('outside the V3.10 §51.3 set') && error.includes('H17'),
    ),
    'a non-pack id on the capability_pack line must be reported',
  );
  // And the line must not be satisfiable by being empty: an emptied pack line
  // has to fail and name what went missing.
  const emptied = structuredClone(registry);
  emptied.acceptance = emptied.acceptance.filter(item => item.release_line !== 'capability_pack');
  assert.ok(
    validateRegistry(emptied, catalogIds).some(
      error => error.includes('capability pack set incomplete') && error.includes('M01'),
    ),
    'an emptied capability_pack line must fail and name a missing id',
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

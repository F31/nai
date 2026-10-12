import { readFile } from 'node:fs/promises';
import YAML from 'yaml';

const ID_RE = /^(S|R|A|B|O|P|M|H|X)[0-9]{2}$/;
const RELEASE_LINES = ['slice_0', 'r1', 'r2', 'r3', 'experimental', 'capability_pack'];
const STATUSES = ['implemented', 'tested', 'blocked', 'not_applicable', 'pending'];

/// Families whose baseline definition is a *complete* id range, with the count
/// the baseline fixes. "Defined but absent", and "present with a hole", are the
/// two failures this table exists for.
///
/// The reason it is a hard gate rather than a comment: ADR 0042 decision 4
/// deliberately left M01-M18 unregistered, and the consequence was recorded as
/// a defect — "not started" has to read as `pending`, because *registered* and
/// *measured* are two different statements, and an omitted family silently
/// drops the whole part from the "the registry is authoritative" view.
/// ADR 0047 decision 4 applies that lesson instead of repeating it.
const COMPLETE_FAMILIES = new Map([
  ['M', {count: 18, baseline: 'V3.10 §37.5、§38.10'}],
  ['H', {count: 18, baseline: 'V3.10 §40–§47、§48.5'}],
]);

/// The post-V3.9 scope split: ids that are *capability packs* — designed and
/// contract-bounded, but deliberately NOT part of the first release's gate.
///
/// V3.9 added the `capability_pack` release line because "designed" and
/// "committed" are two different statements and only the former had a place to
/// live. INV-33 ("the default composition must not depend on a capability
/// pack") is only checkable if the split itself is machine-readable, so the
/// split is asserted here rather than described in prose. Three distinct
/// failures are caught: a pack id sitting on a first-release line, a pack id
/// left off the pack line, and a non-pack id parked on the pack line.
const CAPABILITY_PACK_IDS = [
  ...Array.from({length: 18}, (_, index) => `M${String(index + 1).padStart(2, '0')}`),
  ...['H05', 'H06', 'H07', 'H08', 'H09', 'H10', 'H11', 'H12', 'H13', 'H14', 'H16'],
];
const FIRST_RELEASE_LINES = ['slice_0', 'r1', 'r2'];

/// Contract labels that name an architectural property instead of a callable
/// capability in `schemas/catalog/capabilities.json`. Kept deliberately tiny:
/// every entry has to say why it is not a capability, so an unreviewed label
/// cannot quietly become an exemption.
const NON_CAPABILITY_CONTRACTS = new Map([
  [
    'plugin-hosts-and-ipc',
    'A06 describes the host/IPC architecture and its trust boundary, not a callable wire contract; the catalog lists callable contracts only.',
  ],
]);

export async function readRegistry() {
  return YAML.parse(await readFile('tests/registry.yaml', 'utf8'));
}

/// Capability ids as declared in the catalog, in any stage.
///
/// Draft entries count. A registry row that references a draft contract names
/// something the catalog has actually recorded, which is what this gate is
/// about; it says nothing about the referenced acceptance passing, and must not
/// be read as one.
export async function readCatalogIds() {
  const catalog = JSON.parse(
    await readFile('schemas/catalog/capabilities.json', 'utf8'),
  );
  return new Set(catalog.capabilities.map(capability => capability.id));
}

export function validateRegistry(registry, catalogIds) {
  const errors = [];
  if (registry.schema_version !== 1) errors.push('schema_version must be 1');
  if (!(catalogIds instanceof Set)) {
    errors.push('catalog capability ids are required to validate contract references');
  }
  for (const line of RELEASE_LINES) {
    if (!registry.release_lines || !registry.release_lines[line]) {
      errors.push(`release_lines.${line} missing`);
    }
  }
  // A contract reference is a promise that the catalog knows the name. Without
  // this the registry can point at capabilities that do not exist at all, and
  // the "开放兼容" surface stays silently empty.
  const resolvable = value =>
    catalogIds instanceof Set &&
    (catalogIds.has(value) || NON_CAPABILITY_CONTRACTS.has(value));
  const seen = new Set();
  for (const item of registry.acceptance ?? []) {
    if (!ID_RE.test(item.id)) errors.push(`invalid acceptance id: ${item.id}`);
    if (seen.has(item.id)) errors.push(`duplicate acceptance id: ${item.id}`);
    seen.add(item.id);
    if (!RELEASE_LINES.includes(item.release_line)) {
      errors.push(`${item.id}: invalid release_line ${item.release_line}`);
    }
    if (!STATUSES.includes(item.status)) {
      errors.push(`${item.id}: invalid status ${item.status}`);
    }
    if (item.contract !== undefined && !resolvable(item.contract)) {
      errors.push(
        `${item.id}: contract ${item.contract} is neither a catalog capability nor a registered non-capability contract`,
      );
    }
  }
  // Mapping: every D target must resolve to an existing acceptance id.
  for (const [task, entry] of Object.entries(registry.mapping?.d01_to_d14 ?? {})) {
    if (!/^D[0-9]{2}$/.test(task)) errors.push(`invalid mapping key: ${task}`);
    if (!STATUSES.includes(entry.status)) {
      errors.push(`${task}: invalid status ${entry.status}`);
    }
    for (const target of entry.targets ?? []) {
      if (!seen.has(target)) errors.push(`${task}: target ${target} not in acceptance`);
    }
  }
  // New legacy ids (T/E/U) must not appear in acceptance ids.
  for (const item of registry.acceptance ?? []) {
    if (/^(T|E|U)[0-9]{2}$/.test(item.id)) errors.push(`legacy id ${item.id} must not be re-added`);
  }
  // Every family the baseline defines as a complete range must be registered in
  // full. Counted and compared per family: summing M and H into one "at least 36
  // rows" total would let one family's surplus hide the other's hole.
  for (const [family, {count, baseline}] of COMPLETE_FAMILIES) {
    const expected = Array.from(
      {length: count},
      (_, index) => `${family}${String(index + 1).padStart(2, '0')}`,
    );
    const present = [...seen].filter(id => id.startsWith(family));
    const missing = expected.filter(id => !present.includes(id));
    if (missing.length > 0) {
      errors.push(
        `${family} family incomplete (${baseline}, ${count} items): missing ${missing.join(', ')}`,
      );
    }
    const extra = present.filter(id => !expected.includes(id));
    if (extra.length > 0) {
      errors.push(`${family} family has ids outside the defined range: ${extra.join(', ')}`);
    }
  }
  // The capability-pack split (V3.9 §51.3, INV-33). Checked from both sides so
  // neither "quietly promote a pack into the first release" nor "quietly demote
  // a first-release item into a pack" can pass, and the check cannot be
  // satisfied by an empty pack line.
  const byId = new Map((registry.acceptance ?? []).map(item => [item.id, item]));
  const packSet = new Set(CAPABILITY_PACK_IDS);
  const onPackLine = [...byId.values()]
    .filter(item => item.release_line === 'capability_pack')
    .map(item => item.id);
  const notOnPackLine = CAPABILITY_PACK_IDS.filter(id => !onPackLine.includes(id));
  if (notOnPackLine.length > 0) {
    errors.push(
      `capability pack set incomplete (V3.10 §51.3): not on capability_pack line: ${notOnPackLine.join(', ')}`,
    );
  }
  const strayOnPackLine = onPackLine.filter(id => !packSet.has(id));
  if (strayOnPackLine.length > 0) {
    errors.push(
      `capability_pack line holds ids outside the V3.10 §51.3 set: ${strayOnPackLine.join(', ')}`,
    );
  }
  const leaked = [...byId.values()]
    .filter(item => packSet.has(item.id) && FIRST_RELEASE_LINES.includes(item.release_line))
    .map(item => `${item.id}=${item.release_line}`);
  if (leaked.length > 0) {
    errors.push(
      `capability pack ids must not sit on a first-release line (INV-33): ${leaked.join(', ')}`,
    );
  }
  return errors;
}

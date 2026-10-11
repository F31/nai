import { readFile } from 'node:fs/promises';
import YAML from 'yaml';

const ID_RE = /^(S|R|A|B|O|P|M|X)[0-9]{2}$/;
const RELEASE_LINES = ['slice_0', 'r1', 'r2', 'r3', 'experimental'];
const STATUSES = ['implemented', 'tested', 'blocked', 'not_applicable', 'pending'];

export async function readRegistry() {
  return YAML.parse(await readFile('tests/registry.yaml', 'utf8'));
}

export function validateRegistry(registry) {
  const errors = [];
  if (registry.schema_version !== 1) errors.push('schema_version must be 1');
  for (const line of RELEASE_LINES) {
    if (!registry.release_lines || !registry.release_lines[line]) {
      errors.push(`release_lines.${line} missing`);
    }
  }
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
  return errors;
}

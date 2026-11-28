import {readFile} from 'node:fs/promises';
import Ajv from 'ajv';
import YAML from 'yaml';

const registryPath = 'tests/v561/registry.yaml';
const schemaPath = 'schemas/tests/v561-registry.schema.json';
const text = await readFile(registryPath, 'utf8');
let registry;
try {
  registry = YAML.parse(text);
} catch (error) {
  console.error(`v561 registry: YAML parse failed: ${error.message}`);
  process.exit(1);
}

const schema = JSON.parse(await readFile(schemaPath, 'utf8'));
const ajv = new Ajv({allErrors: true});
const validate = ajv.compile(schema);
const errors = [];
if (!validate(registry)) {
  for (const error of validate.errors ?? []) {
    errors.push(`${error.instancePath || '/'} ${error.message}`);
  }
}

const acceptance = Array.isArray(registry?.acceptance) ? registry.acceptance : [];
const ids = new Set();
const groups = new Set();
const stages = new Set();
const statuses = new Map();
for (const [index, item] of acceptance.entries()) {
  const where = `acceptance[${index}]`;
  if (ids.has(item.id)) errors.push(`${where}: duplicate id ${item.id}`);
  ids.add(item.id);
  groups.add(item.group);
  stages.add(item.stage);
  statuses.set(item.status, (statuses.get(item.status) ?? 0) + 1);
  if (!item.id.includes(`-${item.group}-`)) {
    errors.push(`${where}: id ${item.id} does not match group ${item.group}`);
  }
  if (item.id.startsWith('ACC-V48-') || item.id.startsWith('ACC-V55-') || item.id.startsWith('ACC-V56-')) {
    errors.push(`${where}: inherited legacy ACC id ${item.id}`);
  }
  if (item.status === 'passed' && (!Array.isArray(item.evidence) || item.evidence.length === 0)) {
    errors.push(`${where}: passed ACC requires evidence`);
  }
  if (item.status === 'blocked' && !item.blocked_reason) {
    errors.push(`${where}: blocked ACC requires blocked_reason`);
  }
}
if (registry?.baseline !== 'V5.6.1') errors.push('baseline must be V5.6.1');
if (acceptance.length < 30) errors.push(`expected at least 30 ACC, got ${acceptance.length}`);
for (const stage of ['M0', 'M1', 'M2', 'M3']) {
  if (!stages.has(stage)) errors.push(`missing stage ${stage}`);
}
for (const group of ['SETUP', 'TTS', 'HEAD', 'PIPE', 'LIVE', 'OUT', 'REL', 'RES', 'UX']) {
  if (!groups.has(group)) errors.push(`missing group ${group}`);
}

if (errors.length > 0) {
  console.error('v561 registry: FAILED');
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}

const summary = [...statuses.entries()].map(([status, count]) => `${status}=${count}`).join(', ');
console.log(`v561 registry OK: ${acceptance.length} ACC, stages=${[...stages].sort().join('/')}, ${summary}`);

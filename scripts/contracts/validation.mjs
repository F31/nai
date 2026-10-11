import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020.js';
import SwaggerParser from '@apidevtools/swagger-parser';

export const readJson = async filename => JSON.parse(await readFile(filename, 'utf8'));
export async function validators() {
  const ajv = new Ajv2020({ allErrors: true, strict: true, strictRequired: false });
  const sources = {runtime:'config/runtime',provider:'config/provider',backup:'config/backup',binding:'config/binding-plan',avatar2d:'config/avatar-2d',plugin:'plugin/manifest',model:'plugin/model',catalog:'catalog/catalog',avatar:'plugin/avatar',ui:'plugin/ui',platforms:'platform-matrix/platforms'};
  return Object.fromEntries(await Promise.all(Object.entries(sources).map(async ([key, source]) => [key, ajv.compile(await readJson(`schemas/${source}.schema.json`))])));
}

export function validateSemantic(kind, value) {
  if (kind === 'runtime') {
    if (value.media.lease_expire_ms <= value.media.lease_renew_ms) throw new Error('lease expiry must exceed renewal interval');
    if (value.media.startup_buffer_ms > value.media.max_buffer_ms) throw new Error('startup buffer exceeds maximum');
    if (BigInt(value.budgets.per_session_limit_minor) > 18446744073709551615n) throw new Error('budget amount exceeds uint64');
  }
  if (kind === 'model') {
    const ids = new Set(value.graphs.map(graph => graph.id));
    if (ids.size !== value.graphs.length || value.required_graph_ids.some(id => !ids.has(id))) throw new Error('model graph closure is incomplete or duplicated');
  }
  if (kind === 'provider') {
    const endpoint = new URL(value.endpoint);
    if (endpoint.username || endpoint.password || [...endpoint.searchParams.keys()].some(key => /token|secret|api.?key/i.test(key))) throw new Error('endpoint must not contain credentials');
  }
  if (kind === 'catalog') {
    const ids = new Set(value.capabilities.map(capability => capability.id));
    const frozen = new Set(value.capabilities.filter(capability => capability.stage === 'frozen').map(capability => capability.id));
    // Legacy adapters point at what has been frozen: a draft has no frozen wire
    // representation to adapt to yet, so "target is in the catalog" is too weak.
    if (ids.size !== value.capabilities.length || value.adapters_required.some(adapter => !frozen.has(adapter.target))) throw new Error('duplicate capability or unresolved adapter target');
    // Mirrors the JSON Schema: a half-declared draft would make the stage field
    // unreadable, which is the one thing the field exists to prevent.
    if (value.capabilities.some(capability => capability.stage === 'draft' && ['request','response','stream','session_scoped'].some(key => capability[key] !== undefined))) throw new Error('draft capability declares consumer messages');
  }
}

export async function apiDocument() {
  const document = await SwaggerParser.validate('schemas/openapi/control.yaml');
  if (document['x-implementation-status'] !== 'partial-loopback-engineering') throw new Error('API contract must preserve partial engineering status');
  const implemented = new Set(['get /health','get /runtime/status','get /sessions/{id}','post /sessions/{id}/mute','post /sessions/{id}/takeover','get /approvals','post /approvals/{id}/approve','post /approvals/{id}/reject']);
  for (const [path,item] of Object.entries(document.paths)) {
    for (const method of ['get','post','put','delete']) {
      if (item[method] && !item[method]['x-required-permission']) throw new Error('API operation missing permission declaration');
      if (item[method]) {
        const expected = implemented.has(`${method} ${path}`) ? 'implemented-loopback' : undefined;
        if (item[method]['x-implementation-status'] !== expected) throw new Error(`API implementation claim does not match verified subset: ${method} ${path}`);
      }
    }
  }
  return document;
}

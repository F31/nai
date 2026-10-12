import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import descriptors from 'ts-proto-descriptors';
import { compileFromFile } from 'json-schema-to-typescript';
import openapiTS, { astToString } from 'openapi-typescript';
import SwaggerParser from '@apidevtools/swagger-parser';
import { validators, readJson, validateSemantic, TS_GENERATED_SOURCES } from './validation.mjs';

const require = createRequire(import.meta.url);
const stage = process.argv[2];
if (!stage) throw new Error('generation stage path required');
const ts = path.join(stage, 'ts');
const check = await validators();
const catalog = await readJson('schemas/catalog/capabilities.json');
if (!check.catalog(catalog)) throw new Error(JSON.stringify(check.catalog.errors));
validateSemantic('catalog', catalog);
const set = descriptors.FileDescriptorSet.decode(await readFile(path.join(stage, 'descriptor.bin')));
// The Rust encoder preserves proto2 scalar presence in plugin descriptors.
const request = await readFile(path.join(stage, 'request.bin'));
const tsProtoDir = path.dirname(require.resolve('ts-proto/package.json'));
const generated = spawnSync(process.execPath, [path.join(tsProtoDir, 'protoc-gen-ts_proto')], {
  input: request, maxBuffer: 32 * 1024 * 1024,
});
if (generated.error || generated.status !== 0) throw new Error(`ts-proto failed: ${generated.error ?? generated.stderr}`);
const response = descriptors.CodeGeneratorResponse.decode(generated.stdout);
if (response.error) throw new Error(response.error);
const exports = [];
for (const file of response.file) {
  const output = path.join(ts, file.name);
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, file.content);
  const descriptor = set.file.find(source => source.name === file.name.replace(/\.ts$/, '.proto'));
  const names = [...descriptor.enumType, ...descriptor.messageType].map(type => type.name).sort();
  exports.push(`export { ${names.join(', ')} } from './${file.name.replace(/\.ts$/, '.js')}';`);
}
// The list of schemas that get a TypeScript counterpart lives in
// `validation.mjs`, next to the list of schemas that get validated. A second
// copy here would be a list that can disagree with the first one in silence;
// the guard in `tests/contracts/schema.test.mjs` pins both ends (ADR 0052).
for (const source of TS_GENERATED_SOURCES) {
  const output = `config/${path.basename(source)}.ts`;
  await mkdir(path.join(ts, 'config'), { recursive: true });
  await writeFile(path.join(ts, output), await compileFromFile(`schemas/${source}.schema.json`, {
    bannerComment: '/* Generated from JSON Schema. Do not edit. */',
    style: { endOfLine: 'lf' },
  }));
  // Keep manifest helper names scoped to avoid collisions with protobuf names.
  const namespace = path.basename(source).replace(/-([a-z0-9])/g, (_, letter) => letter.toUpperCase());
  exports.push(`export type * as ${namespace}Config from './${output.replace(/\.ts$/, '.js')}';`);
}
await SwaggerParser.validate('schemas/openapi/control.yaml');
const api = await SwaggerParser.bundle('schemas/openapi/control.yaml');
await writeFile(path.join(ts, 'http.ts'), astToString(await openapiTS(api)));
exports.push("export type { paths, components, operations } from './http.js';");
// Keep the adapter contract independent of the desktop control API. Emit only
// the explicitly supported OpenAPI DTO subset; validators preserve all bounds.
await SwaggerParser.validate('schemas/openapi/remote-knowledge.yaml');
const remote = await SwaggerParser.bundle('schemas/openapi/remote-knowledge.yaml');
await writeFile(path.join(ts, 'remote-knowledge.ts'), astToString(await openapiTS(remote)));
exports.push("export type * as remoteKnowledge from './remote-knowledge.js';");
const schemas = remote.components.schemas;
const rustType = schema => {
  if (schema.$ref) {
    if (!schema.$ref.startsWith('#/components/schemas/')) throw new Error('nonlocal remote schema reference');
    return schema.$ref.split('/').at(-1);
  }
  if (schema.type === 'string') return 'String';
  if (schema.type === 'boolean') return 'bool';
  if (schema.type === 'integer' && schema.minimum >= 0) return 'u64';
  if (schema.type === 'array') return `Vec<${rustType(schema.items)}>`;
  if (schema.type === 'object' && typeof schema.additionalProperties === 'object') return `std::collections::BTreeMap<String, ${rustType(schema.additionalProperties)}>`;
  throw new Error(`unsupported remote DTO schema: ${JSON.stringify(schema)}`);
};
const pascal = value => value.split('_').map(part => part[0].toUpperCase() + part.slice(1)).join('');
const rust = ['// Generated from schemas/openapi/remote-knowledge.yaml. Do not edit.'];
for (const [name, schema] of Object.entries(schemas).sort()) {
  rust.push('#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]');
  if (schema.type === 'string' && schema.enum) {
    rust.push(`pub enum ${name} {`);
    for (const value of schema.enum) rust.push(`#[serde(rename = ${JSON.stringify(value)})] ${pascal(value)},`);
  } else if (schema.type === 'object' && schema.additionalProperties === false) {
    rust.push('#[serde(deny_unknown_fields)]', `pub struct ${name} {`);
    for (const [field, property] of Object.entries(schema.properties ?? {})) {
      const optional = !(schema.required ?? []).includes(field) || property.nullable;
      if (optional) rust.push('#[serde(default, skip_serializing_if = "Option::is_none")]');
      rust.push(`pub ${field}: ${optional ? `Option<${rustType(property)}>` : rustType(property)},`);
    }
  } else throw new Error(`unsupported remote component ${name}`);
  rust.push('}');
}
const jsonSchema = value => {
  if (Array.isArray(value)) return value.map(jsonSchema);
  if (!value || typeof value !== 'object') return value;
  const output = Object.fromEntries(Object.entries(value).filter(([key]) => key !== 'nullable').map(([key, item]) => [key, jsonSchema(item)]));
  if (output.$ref) output.$ref = output.$ref.replace('#/components/schemas/', '#/$defs/');
  return value.nullable ? {anyOf:[output,{type:'null'}]} : output;
};
// Control consumers validate from the same OpenAPI source. Flatten allOf
// inheritance before closing object fields: closing each base would reject
// the legitimate fields of its SessionCommand/ModeCommand descendants.
const controlSchema = schema => {
  if (schema.allOf) {
    const result = {type:'object', additionalProperties:false, properties:{}, required:[]};
    for (const part of schema.allOf) {
      const source = part.$ref ? api.components.schemas[part.$ref.split('/').at(-1)] : part;
      const flat = controlSchema(source);
      Object.assign(result.properties, flat.properties);
      result.required.push(...(flat.required ?? []));
    }
    result.required = [...new Set(result.required)];
    return result;
  }
  return schema.type === 'object' && schema.properties ? {...schema, additionalProperties:false} : schema;
};
const controlDefs = Object.fromEntries(Object.entries(api.components.schemas).map(([name,schema]) => [name,controlSchema(schema)]));
const controlRust = path.join(stage,'rust','control_http.rs');
await writeFile(controlRust, `// Generated from schemas/openapi/control.yaml. Do not edit.\npub const SCHEMAS_JSON: &str = r###"${JSON.stringify({$schema:'https://json-schema.org/draft/2020-12/schema',$defs:jsonSchema(controlDefs)})}"###;\n`);
const controlFormatted = spawnSync('rustfmt', ['--edition','2024',controlRust]);
if (controlFormatted.error || controlFormatted.status !== 0) throw new Error('control schema rustfmt failed');
rust.push(`pub const SCHEMAS_JSON: &str = r###"${JSON.stringify({$schema:'https://json-schema.org/draft/2020-12/schema',$defs:jsonSchema(schemas)})}"###;`);
for (const [route, methods] of Object.entries(remote.paths)) {
  for (const [method, operation] of Object.entries(methods)) {
    if (!operation.operationId || !operation['x-required-permission']) throw new Error('remote endpoint lacks identity or permission');
    rust.push(`pub const ${operation.operationId.toUpperCase()}_PATH: &str = ${JSON.stringify(route)};`);
    rust.push(`pub const ${operation.operationId.toUpperCase()}_METHOD: &str = ${JSON.stringify(method.toUpperCase())};`);
  }
}
const rustRemote = path.join(stage, 'rust', 'remote_knowledge.rs');
await writeFile(rustRemote, rust.join('\n') + '\n');
const formatted = spawnSync('rustfmt', ['--edition', '2024', rustRemote]);
if (formatted.error || formatted.status !== 0) throw new Error(`remote DTO rustfmt failed: ${formatted.error ?? formatted.stderr}`);
await writeFile(path.join(ts, 'index.ts'), `// Generated contract exports. Do not edit.\n${exports.sort().join('\n')}\n`);
console.log(`TS codecs generated for ${response.file.length} protobuf sources; JSON Schema/OpenAPI types generated`);

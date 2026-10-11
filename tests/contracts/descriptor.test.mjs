import test from 'node:test';
import assert from 'node:assert/strict';
import descriptors from 'ts-proto-descriptors';
import { readFile } from 'node:fs/promises';
import { readJson, apiDocument } from '../../scripts/contracts/validation.mjs';

// Checks that compare the contract sources against the **compiled descriptor**
// live here rather than in `schema.test.mjs`. The descriptor is a generation
// artifact (`target/contracts/descriptor.bin`, written by `cargo xtask schema`)
// and needs protoc, so these belong to the contracts suite. The static JSON
// Schema / OpenAPI validation stays in `schema.test.mjs`, which the engineering
// gate runs on every `check --profile portable|full` — that is what keeps a
// schema change and its sample from drifting apart unnoticed.
const catalog = await readJson('schemas/catalog/capabilities.json');
const descriptor = descriptors.FileDescriptorSet.decode(
  await readFile('target/contracts/descriptor.bin'),
);
const messages = new Set(
  descriptor.file.flatMap(file =>
    file.messageType.map(message => `${file.package}.${message.name}`),
  ),
);

test('frozen capabilities resolve to real messages and drafts resolve to none', () => {
  const frozen = catalog.capabilities.filter(capability => capability.stage === 'frozen');
  assert.ok(frozen.length > 0, 'expected frozen capabilities');
  for (const capability of frozen) {
    assert.ok(
      messages.has(capability.request),
      `${capability.id} references absent request ${capability.request}`,
    );
    assert.ok(
      messages.has(capability.response),
      `${capability.id} references absent response ${capability.response}`,
    );
    assert.equal(typeof capability.stream, 'boolean', `${capability.id} needs stream semantics`);
  }
  // The negotiation surface is built from frozen entries only, so the two sets
  // are disjoint by construction; this pins that the draft ids name no message
  // the wire could carry.
  for (const capability of catalog.capabilities.filter(entry => entry.stage === 'draft')) {
    for (const key of ['request', 'response', 'stream', 'session_scoped']) {
      assert.equal(capability[key], undefined, `${capability.id} is draft yet declares ${key}`);
    }
  }
});

test('OpenAPI error codes agree with the generated error enum', async () => {
  const api = await apiDocument();
  const error = descriptor.file
    .flatMap(file => file.enumType)
    .find(type => type.name === 'ErrorCode');
  assert.deepEqual(
    api.components.schemas.ErrorResponse.properties.error.properties.code.enum.sort(),
    error.value.filter(value => value.number !== 0).map(value => value.name).sort(),
  );
});

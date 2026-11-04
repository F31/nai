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

test('actual device progress preserves optional count and native backend identity', async()=>{
  const {LocalPlaybackStatus}=await import('../../packages/contracts-ts/dist/index.js');
  const message=descriptor.file.flatMap(f=>f.messageType).find(m=>m.name==='LocalPlaybackStatus');
  const fields=Object.fromEntries(message.field.map(f=>[f.name,f]));
  assert.equal(fields.device_played_samples.number,12);
  assert.equal(fields.device_played_samples.proto3Optional,true);
  assert.equal(fields.device_position_observed.number,13);assert.equal(fields.audio_backend.number,14);
  const value=LocalPlaybackStatus.create({devicePlayedSamples:'24000',devicePositionObserved:true,audioBackend:'wasapi-iaudioclock'});
  const decoded=LocalPlaybackStatus.decode(LocalPlaybackStatus.encode(value).finish());
  assert.equal(decoded.devicePlayedSamples,'24000');assert.equal(decoded.devicePositionObserved,true);assert.equal(decoded.audioBackend,'wasapi-iaudioclock');assert.equal(decoded.actualLocalPlaybackCompleted,false);
  assert.equal(LocalPlaybackStatus.decode(new Uint8Array()).devicePlayedSamples,undefined);
  assert.equal(LocalPlaybackStatus.decode(LocalPlaybackStatus.encode(LocalPlaybackStatus.create({devicePlayedSamples:'0'})).finish()).devicePlayedSamples,'0');
});

test('video tail diagnostics preserve optional zero and full audio position without AV completion',async()=>{
 const {LocalPlaybackStatus}=await import('../../packages/contracts-ts/dist/index.js');
 const message=descriptor.file.flatMap(f=>f.messageType).find(m=>m.name==='LocalPlaybackStatus');const fields=Object.fromEntries(message.field.map(f=>[f.name,f]));
 assert.equal(fields.video_frame_index.number,15);assert.equal(fields.video_frame_index.proto3Optional,true);
 assert.equal(fields.video_device_samples.number,16);assert.equal(fields.video_device_samples.proto3Optional,true);assert.equal(fields.video_tail_submitted.number,17);
 const value=LocalPlaybackStatus.decode(LocalPlaybackStatus.encode(LocalPlaybackStatus.create({videoFrameIndex:0,videoDeviceSamples:'24000',videoTailSubmitted:true})).finish());
 assert.equal(value.videoFrameIndex,0);assert.equal(value.videoDeviceSamples,'24000');assert.equal(value.videoTailSubmitted,true);assert.equal(value.actualLocalPlaybackCompleted,false);
 assert.equal(LocalPlaybackStatus.decode(new Uint8Array()).videoFrameIndex,undefined);
});

test('callback tail diagnostics preserve descriptor binding and optional count without hardware acknowledgement',async()=>{
  const {LocalPlaybackStatus}=await import('../../packages/contracts-ts/dist/index.js');
  const message=descriptor.file.flatMap(f=>f.messageType).find(m=>m.name==='LocalPlaybackStatus');
  const fields=Object.fromEntries(message.field.map(f=>[f.name,f]));
  assert.equal(fields.media_sha256.number,9);assert.equal(fields.callback_drained_samples.number,10);
  assert.equal(fields.callback_drained_samples.proto3Optional,true);assert.equal(fields.callback_drain_observed.number,11);
  const value=LocalPlaybackStatus.create({version:1,state:'submitted',mediaSha256:'a'.repeat(64),copiedSamples:'24000',callbackDrainedSamples:'24000',callbackDrainObserved:true});
  const decoded=LocalPlaybackStatus.decode(LocalPlaybackStatus.encode(value).finish());
  assert.equal(decoded.callbackDrainedSamples,'24000');assert.equal(decoded.callbackDrainObserved,true);assert.equal(decoded.actualLocalPlaybackCompleted,false);assert.equal(decoded.mediaSha256,value.mediaSha256);
  const missing=LocalPlaybackStatus.decode(new Uint8Array());assert.equal(missing.callbackDrainedSamples,undefined);
  const zero=LocalPlaybackStatus.decode(LocalPlaybackStatus.encode(LocalPlaybackStatus.create({callbackDrainedSamples:'0'})).finish());assert.equal(zero.callbackDrainedSamples,'0');
});

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

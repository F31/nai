import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import descriptors from 'ts-proto-descriptors';

test('compiled descriptor exposes signed Ready block geometry and version 6',async()=>{
 const set=descriptors.FileDescriptorSet.decode(await readFile(path.join(process.env.CARGO_TARGET_DIR,'contracts/descriptor.bin')));
 const file=set.file.find(f=>f.package==='nai.v561.avatar.v1');
 assert.equal(file.enumType.find(e=>e.name==='WireVersion').value.find(v=>v.name==='WIRE_VERSION_CURRENT').number,6);
 const event=file.messageType.find(m=>m.name==='Event');
 const field=event.field.find(f=>f.name==='warmup');
 assert.equal(field.number,20);
 assert.equal(field.typeName,'.nai.v561.avatar.v1.WarmupStats');
 assert.deepEqual(file.messageType.find(m=>m.name==='WarmupStats').field.map(f=>f.name),
  ['samples','frames_acked','blocks','unique_graphs','generated_kernels','elapsed_ms','block_frames','motion_frames']);
});

test('configuration separates bounded maintenance and business without old-config defaults',async()=>{
 const schema=JSON.parse(await readFile('schemas/config/avatar-worker.schema.json','utf8'));
 assert.equal(schema.additionalProperties,false);
 for(const name of ['maintenance_timeout_ms','warmup_samples'])assert.ok(schema.required.includes(name));
 assert.equal(schema.properties.maintenance_timeout_ms.maximum,240000);
 assert.equal(schema.properties.timeout_ms.maximum,45000);
 assert.equal(schema.properties.warmup_samples.maximum,144000);
});

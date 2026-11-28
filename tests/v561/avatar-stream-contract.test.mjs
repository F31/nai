import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import descriptors from 'ts-proto-descriptors';

test('incremental control references raw payload without embedding PCM', async()=>{
 const set=descriptors.FileDescriptorSet.decode(await readFile(path.join(process.env.CARGO_TARGET_DIR,'contracts/descriptor.bin')));
 const file=set.file.find(f=>f.package==='nai.v561.avatarstream.v1');
 const message=name=>file.messageType.find(m=>m.name===name);
 assert.deepEqual(message('Chunk').field.map(f=>f.name),['raw_payload_id','start_sample','samples','sha256']);
 assert.deepEqual(message('Request').field.map(f=>f.name),['version','sequence','task_id','epoch','mac','begin','chunk','eos','cancel','release']);
 assert.equal(message('Request').oneofDecl.length,1);
 assert.equal(file.enumType.find(e=>e.name==='WireVersion').value.find(v=>v.name==='WIRE_VERSION_CURRENT').number,2);
 for(const name of ['frame','mapping_name','warmup','stats','portrait_sha256','model_loads'])assert.ok(message('Event').field.some(f=>f.name===name));
 assert.deepEqual(message('Eos').field.map(f=>f.name),['samples','sha256','provider_eos_verified']);
 assert.ok(!file.messageType.some(m=>m.field.some(f=>['pcm','audio','payload','base64'].includes(f.name))));
 const existing=set.file.find(f=>f.package==='nai.v561.avatar.v1');
 assert.equal(existing.enumType.find(e=>e.name==='WireVersion').value.find(v=>v.name==='WIRE_VERSION_CURRENT').number,6);
 assert.ok(!existing.messageType.find(m=>m.name==='Request').field.some(f=>f.name==='begin'));
});

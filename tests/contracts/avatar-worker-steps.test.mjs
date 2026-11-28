import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020.js';

const schema=JSON.parse(await readFile(new URL('../../schemas/config/avatar-worker.schema.json',import.meta.url)));
const validate=new Ajv2020({strict:true}).compile(schema);
const base={python:'E:/python.exe',upstream_dir:'E:/upstream',dependency_dirs:['E:/deps'],
 model_dir:'E:/model',wav2vec_dir:'E:/wav',portrait:'E:/portrait.jpg',portrait_sha256:'a'.repeat(64),
 portrait_authorized:true,mapping_reader:'E:/reader.exe',temp_dir:'E:/tmp',width:512,height:512,
 frame_num:41,seed:42,face_ratio:2.8,timeout_ms:45000,maintenance_timeout_ms:240000,warmup_samples:76800};
test('absent step choice and each supported choice are accepted without mutation',()=>{
 assert.equal(validate(base),true);assert.equal(Object.hasOwn(base,'sample_steps'),false);
 for(const step of [2,3,4])assert.equal(validate({...base,sample_steps:step}),true);
});
test('unsupported and noninteger steps are rejected',()=>{
 for(const step of [0,1,5,8,2.5,'3',true,null])assert.equal(validate({...base,sample_steps:step}),false);
});
test('unassembled experimental controls stay outside configuration',()=>{
 for(const key of ['override_timesteps','execution_candidate'])assert.equal(validate({...base,[key]:{}}),false);
});

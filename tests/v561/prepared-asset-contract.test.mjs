import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import Ajv2020 from 'ajv/dist/2020.js';
const validate=new Ajv2020({strict:true}).compile(JSON.parse(readFileSync('schemas/media/prepared-utterance.schema.json','utf8')));
const asset={version:1,scope:'private_session_prepared_asset_not_current_model_cache',identity_sha256:'a'.repeat(64),pcm_sha256:'b'.repeat(64),
 frame_sha256:['c'.repeat(64)],width:768,height:768,samples:960,frames:1,ttl_ms:60000,model_api_calls:1};
test('manifest is content metadata and cannot carry old authority or pretend current-model cache',()=>{
 assert.equal(validate(asset),true);
 for(const change of [{epoch:1},{permit:'old'},{scope:'current_model_cache'},{pcm_sha256:'wrong'},{model_api_calls:0},{ttl_ms:3600001}])
  assert.equal(validate({...asset,...change}),false);
});
test('prepared plan manifest is bounded content metadata without execution authority',()=>{
 const check=new Ajv2020({strict:true}).compile(JSON.parse(readFileSync('schemas/media/prepared-plan.schema.json','utf8')));
 const plan={version:1,scope:'private_session_prepared_plan',input_sha256:'a'.repeat(64),asset_identities:['b'.repeat(64)],total_bytes:100,total_samples:960,model_api_calls:1};
 assert.equal(check(plan),true);
 for(const change of [{epoch:1},{permit:'old'},{scope:'shared_model_cache'},{asset_identities:[]},{asset_identities:Array(9).fill('b'.repeat(64))},{total_bytes:2147483649},{total_samples:480001},{model_api_calls:0}])
  assert.equal(check({...plan,...change}),false);
});

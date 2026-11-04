import {test} from 'node:test';
import assert from 'node:assert/strict';
import {validators,readJson} from '../../scripts/contracts/validation.mjs';
const validate=(await validators()).preparation;
const product=await readJson('tests/fixtures/contracts/preparation.json');
test('preparation contract closes fields, pins values and permits incomplete drafts',()=>{
 assert.equal(validate(product),true);
 assert.equal(validate({...product,facts:[],faq:[],talk_tracks:[]}),true);
 for(const bad of [{...product,published:true},{...product,schema_version:2},{...product,kind:'product'}, {...product,name:'  '}] )assert.equal(validate(bad),false);
 const fact=product.facts[0];
 for(const patch of [{value_type:'money',value:12,currency:'CNY'},{value_type:'boolean',value:'true'},{value:'1e3'},{value:'NaN'},{source:''},{authority:'model_approved'}])assert.equal(validate({...product,facts:[{...fact,...patch}]}),false);
 assert.equal(validate({...product,facts:[{...fact,value_type:'money',value:'129900',currency:'CNY'}]}),true);
});
test('host preferences have no credential or authority fields; shows pin complete object versions',()=>{
 const host={schema_version:1,kind:'host_draft',name:'助播',style:'friendly',preferences:'先说明限制'};
 assert.equal(validate(host),true);assert.equal(validate({...host,authority:'owner_confirmed'}),false);
 const pointer={object_id:'00000000-0000-0000-0000-000000000001',revision:1,sha256:'a'.repeat(64)};
 const show={schema_version:1,kind:'show_draft',name:'首场',product:pointer,host:pointer,series:pointer,cues:[]};
 assert.equal(validate(show),true);
 assert.equal(validate({...show,product:{...pointer,revision:0}}),false);
 assert.equal(validate({...show,product:{object_id:pointer.object_id}}),false);
});

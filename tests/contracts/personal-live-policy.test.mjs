import {test} from 'node:test';
import assert from 'node:assert/strict';
import {validators} from '../../scripts/contracts/validation.mjs';

const validate = (await validators()).personalLivePolicy;
const policy = {schema_version:1,interaction_mode:'topic_and_platform',answer_trigger:'automatic',knowledge_scope:'grounded_only',sampling_percent:30};

test('personal preference accepts bounded interaction settings without carrying authority', () => {
  assert.equal(validate(policy), true);
  for (const n of [0,100]) assert.equal(validate({...policy,sampling_percent:n}),true);
  for (const n of [-1,101,1.5]) assert.equal(validate({...policy,sampling_percent:n}),false);
  for (const bad of [{...policy,grant:true},{...policy,session_id:'claimed'},{...policy,answer_trigger:'always_allowed'},{...policy,knowledge_scope:'unverified'},{...policy,schema_version:2}]) assert.equal(validate(bad),false);
});

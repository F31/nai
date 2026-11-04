import test from 'node:test';
import assert from 'node:assert/strict';
import {adoptedTrack,stageAdoptedTrack} from '../../apps/workbench-desktop/ui/adoption.ts';
const old={talk_tracks:[{id:'old',text:'old',fact_ids:['f']}]};
const product={talk_tracks:[...old.talk_tracks,{id:'new',text:'new',fact_ids:['f','g']}]};
const show={product:{object_id:'p'},cues:[{id:'c',talk_track_id:'old',mandatory_fact_ids:['f']},{id:'other',talk_track_id:'old'}]};
test('adoption stages exactly the selected cue without mutating show or publishing',()=>{
 assert.equal(adoptedTrack(old,product),'new');const staged=stageAdoptedTrack(show,product,'p','c','new');
 assert.equal(staged.cues[0].talk_track_id,'new');assert.deepEqual(staged.cues[0].mandatory_fact_ids,['f','g']);assert.equal(show.cues[0].talk_track_id,'old');assert.equal(staged.cues[1],show.cues[1]);assert.equal(staged.product,show.product);
});
test('ambiguous adoption and changed product/cue/track fail closed',()=>{
 assert.equal(adoptedTrack(old,old),null);assert.equal(adoptedTrack(old,{talk_tracks:[...product.talk_tracks,{id:'extra'}]}),null);
 for(const args of [[show,old,'p','c','new'],[show,product,'wrong','c','new'],[show,product,'p','missing','new']])assert.throws(()=>stageAdoptedTrack(...args));
});


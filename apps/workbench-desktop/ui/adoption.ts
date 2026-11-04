import type {Product,Show} from './forms.ts';
export function adoptedTrack(before:Product,after:Product){
 const previous=new Set(before.talk_tracks.map(t=>t.id));
 const added=after.talk_tracks.filter(t=>!previous.has(t.id));
 return added.length===1?added[0].id:null;
}
export function stageAdoptedTrack(show:Show,product:Product,productId:string,cueId:string,trackId:string):Show{
 const track=product.talk_tracks.find(t=>t.id===trackId);
 if(show.product.object_id!==productId||!track||!show.cues.some(c=>c.id===cueId))throw Error('原场次或已采用话术已变化，请手工核对编排。');
 return {...show,cues:show.cues.map(c=>c.id===cueId?{...c,talk_track_id:track.id,mandatory_fact_ids:[...track.fact_ids]}:c)};
}

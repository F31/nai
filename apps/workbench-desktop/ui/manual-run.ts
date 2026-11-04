import type {preparationConfig} from '@nai/contracts';
export type ManualRun=preparationConfig.ManualRun;
export type ManualPorts={stop:()=>Promise<void>;save:(doc:ManualRun)=>Promise<void>;show:(cue:string)=>Promise<void>};
export async function moveManual(doc:ManualRun,cursor:number,ports:ManualPorts){
 if(doc.status!=='open'||!Number.isInteger(cursor)||cursor<0||cursor>=doc.cue_ids.length)throw Error('此记录已结束或单元不存在。');
 await ports.stop();
 await ports.save({...doc,cursor});
 await ports.show(doc.cue_ids[cursor]);
}
export async function endManual(doc:ManualRun,ports:Pick<ManualPorts,'stop'|'save'>){
 await ports.stop();
 if(doc.status==='ended')return;
 await ports.save({...doc,status:'ended'});
}
export function markManual(doc:ManualRun,note:string,id:string):ManualRun{
 const text=note.trim();if(!text||text.length>256||doc.marks.length>=8)throw Error('备注需填写1–256字，单场最多8条。');
 if(doc.marks.some(m=>m.id===id))throw Error('备注标识已存在，请重新操作。');
 return{...doc,marks:[...doc.marks,{id,cue_id:doc.cue_ids[doc.cursor],note:text}] as ManualRun['marks']};
}


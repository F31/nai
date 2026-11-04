import {useEffect,useState} from 'react';
import type {Workbench} from './workbench';
export function LivePlanPanel({v}:{v:Workbench}){
 const live=v.liveRun,c=live.context,p=live.speechPlan;
 const [rights,setRights]=useState(false);
 useEffect(()=>setRights(false),[p?.planSha256,p?.revision,p?.batchApproval?.approvalId]);
 if(!c||!p||p.revision!==c.revision)return null;
 const b=p.batchApproval,expired=!!b&&BigInt(b.expiresAtMs)<=BigInt(v.clock??Date.now());
 const title=(id:string)=>v.snapshotCues.find(x=>x.id===id)?.title??id;
 return <details aria-label="分句计划"><summary>分句计划：共 {p.sentences.length} 句 · 每句最多120字</summary>
 <p>原文完整保留；可一次核对并批准全部分句文本，有效5分钟。文本批准不授予模型费用、音画批准或输出租约；可在自动讲品区追加语音、费用与目标授权。</p>
 <ol>{p.sentences.map((s,index)=><li key={`${s.cueId}:${s.sentenceIndex}`} aria-current={index===p.currentStart?'step':undefined}><strong>{title(s.cueId)} · 第{s.sentenceIndex+1}句{index===p.currentStart?' · 当前':''}</strong><p style={{whiteSpace:'pre-wrap'}}>{s.text}</p></li>)}</ol>
 {c.sentenceMode&&<>{b&&<p role="status">{b.revoked?'批量文本批准已撤销':expired?'批量文本批准已过期':b.approved?'全部分句文本已批准（不自动合成或播放）':'原文本批准已失效，请重新核对。'}</p>}
 {!b?.approved||expired?<><label className="check"><input type="checkbox" checked={rights} disabled={v.busy||![1,4].includes(c.state)} onChange={e=>setRights(e.target.checked)}/>我已核对上方全部分句文本，并确认拥有本机公共窗口使用权</label><button disabled={v.busy||!rights||![1,4].includes(c.state)} onClick={()=>void v.run(live.approveTextPlan)}>批准全部分句文本（不播出）</button></>:null}
 {b&&!b.revoked&&<button disabled={v.busy} onClick={()=>void v.run(live.revokeTextPlan)}>撤销批量文本批准</button>}</>}
 </details>;
}

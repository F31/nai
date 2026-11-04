import {useEffect,useState} from 'react';
import {PublicMediaBundle,PublicMediaAudio,PublicMediaVisual,DeliveryScope,type PublicCandidateCheck,type FoundationPublicCandidate,type PublicApprovalView,type PublicWindowPrepared} from '@nai/contracts';
import type {Workbench} from './workbench';
import {validWindowPrepared,captionReason} from './window-prepared.ts';
import {explain} from './api';
const uuid=/^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;
const same=(a:Uint8Array,b:Uint8Array)=>a.length===b.length&&a.every((v,i)=>v===b[i]);
export function validApprovalDraft(c:PublicCandidateCheck,x:FoundationPublicCandidate):boolean{
 const d=c.approvalDraft;if(!d)return !c.approvalAvailable;
 const b=d.bundle;if(!b||!c.approvalAvailable||!c.preparedMediaReceipt||!c.guardReview?.policyPassed||!c.outputInstance||!c.audio||!c.visual)return false;
 return [b.bundleId,b.approvalId,b.guardPermitId].every(v=>uuid.test(v))&&b.protocolMajor===1&&b.scope===DeliveryScope.DELIVERY_SCOPE_PUBLIC&&b.leaseMs===500&&b.workspaceId===x.workspaceId&&b.sessionId===c.sessionId&&b.epoch===c.epoch&&b.outputId===c.outputInstance.outputId&&b.snapshotId===c.snapshotId&&b.snapshotSha256===c.snapshotSha256&&b.factsSha256===c.factsSha256&&b.captionSha256===c.captionSha256&&!!b.audio&&!!b.visual&&same(PublicMediaAudio.encode(b.audio).finish(),PublicMediaAudio.encode(c.audio).finish())&&same(PublicMediaVisual.encode(b.visual).finish(),PublicMediaVisual.encode(c.visual).finish())&&b.preparedMediaSha256===c.preparedMediaReceipt.descriptorSha256&&d.text===c.text&&/^\d+$/.test(d.expiresAtMs)&&BigInt(d.expiresAtMs)>BigInt(c.checkedAtMs)&&BigInt(d.expiresAtMs)<=BigInt(c.checkedAtMs)+30000n&&c.axes.filter(a=>a.axis!=='output').every(a=>a.state===1);
}
export async function verifyApprovalDraftHash(c:PublicCandidateCheck):Promise<boolean>{
 const b=c.approvalDraft?.bundle;if(!b)return !c.approvalAvailable;
 const h=await crypto.subtle.digest('SHA-256',new Uint8Array(PublicMediaBundle.encode({...b,approvedSha256:''}).finish()));
 return Array.from(new Uint8Array(h),v=>v.toString(16).padStart(2,'0')).join('')===b.approvedSha256;
}
export function PublicApprovalControls({v,check,request}:{v:Workbench;check:PublicCandidateCheck;request:FoundationPublicCandidate}){
 const [rights,setRights]=useState(false),[consumed,setConsumed]=useState(false),[view,setView]=useState<PublicApprovalView|null>(null),[prepared,setPrepared]=useState<PublicWindowPrepared|null>(null),[error,setError]=useState(''),[now,setNow]=useState(Date.now);
 useEffect(()=>{const t=setInterval(()=>setNow(Date.now()),500);return()=>clearInterval(t);},[]);
 const draft=check.approvalDraft,b=draft?.bundle;if(!draft||!b)return null;
 const expired=now>=Number(draft.expiresAtMs);
 async function confirm(){if(!rights||expired||consumed)return;setConsumed(true);setError('');try{
 const r=await v.api.call({$case:'confirmPublicApproval',confirmPublicApproval:{candidate:request,bundleSha256:b!.approvedSha256,acknowledgePublicRights:true}}),a=r.publicApproval;
 if(!a?.approved||a.revoked||a.playbackAvailable||!a.bundle||r.resourceId!==b!.approvalId||!same(PublicMediaBundle.encode(a.bundle).finish(),PublicMediaBundle.encode(b!).finish())||a.expiresAtMs!==draft!.expiresAtMs)throw Error('批准结果无法核对，请重新检查；不要重复确认。');setView(a);
 }catch(e){setError(explain(e)+' 请重新检查本句后再操作；此确认不会自动重试。');}}
 async function play(){setError('');setPrepared(null);try{const r=await v.api.call({$case:'startPublicExecution',startPublicExecution:{candidate:request,bundleSha256:b!.approvedSha256}});const p=r.publicWindowPrepared;if(!p||!validWindowPrepared(p,b!.outputId,b!.preparedMediaSha256))throw Error('公共输出回执无法核对，请重新检查。');if(!p.captionReady)throw Error(captionReason(p));if(!p.playing&&!p.submissionComplete)throw Error('本句未进入输出，请核对默认播放设备和当前批准，不会自动重试。');setPrepared(p);}catch(e){setError(explain(e));}}
 async function stop(){setError('');try{await v.api.call({$case:'stopPublicExecution',stopPublicExecution:{workspaceId:request.workspaceId,approvalId:b!.approvalId}});setPrepared(null);}catch(e){setError(explain(e));}}
 async function revoke(){setError('');setPrepared(null);try{const r=await v.api.call({$case:'revokePublicApproval',revokePublicApproval:{workspaceId:request.workspaceId,approvalId:b!.approvalId}});if(r.resourceId!==b!.approvalId||!r.publicApproval?.revoked||r.publicApproval.approved||r.publicApproval.playbackAvailable)throw Error('撤销结果无法核对，请重连检查。');setView(r.publicApproval);}catch(e){setError(explain(e));}}
 return <section aria-label="本句内容批准"><h4>确认本句内容</h4><p>目标：本机公共窗口。{Number(check.audio?.sampleCount??0)/24000} 秒语音。此确认不播放，也不授权任何直播平台。</p><p>确认有效至 {new Date(Number(draft.expiresAtMs)).toLocaleTimeString()}。</p>{!view&&<><label><input type="checkbox" checked={rights} disabled={consumed||expired||v.busy} onChange={e=>setRights(e.target.checked)}/>我已核对上方完整原文，并确认有权将本句、语音和形象用于本机公共窗口。</label><button disabled={!rights||consumed||expired||v.busy} onClick={()=>void v.run(confirm)}>确认本句（不播出）</button></>}{expired&&!view&&<p>确认已过期，请重新检查。</p>}{view&&<p role="status">{view.revoked?'本句批准已撤销':expired?'本句批准已过期':'本句内容已批准，可明确播出一次。'}</p>}{view?.approved&&!view.revoked&&!expired&&<button disabled={v.busy} onClick={()=>void v.run(play)}>播出本句</button>}{prepared&&<p role="status">已向公共窗口发送本句。设备：{prepared.deviceName}。实时进度见公共输出页。</p>}{view?.approved&&!view.revoked&&<button disabled={v.busy} onClick={()=>void v.run(stop)}>停止本句</button>}{view?.approved&&!view.revoked&&<button disabled={v.busy} onClick={()=>void v.run(revoke)}>撤销本句批准</button>}{error&&<p role="alert">{error}</p>}</section>;
}

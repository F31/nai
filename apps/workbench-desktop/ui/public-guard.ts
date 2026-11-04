import {PublicGuardPolicy,PublicGuardReason,PublicGuardObject,PublicMediaVisual,type PublicCandidateCheck,type FoundationPublicCandidate} from '@nai/contracts';
import {validBoundOutput} from './public-visual.ts';
export const guardPolicy=PublicGuardPolicy[PublicGuardPolicy.PUBLIC_GUARD_POLICY_EXACT_SPECS_AND_SOFTWARE_V5];
const reasons:Record<number,string>={1:'GUARD_EXACT_SPEC_PASSED',2:'GUARD_UNSUPPORTED_SPEECH',3:'GUARD_RISKY_CONTENT',4:'GUARD_UNSUPPORTED_RULE',5:'GUARD_PROHIBITED_PHRASE',6:'GUARD_MEDIA_NOT_READY',7:'GUARD_OUTPUT_NOT_BOUND',8:'GUARD_EXACT_FEATURE_PASSED',9:'GUARD_PLAYBACK_WINDOW_TOO_SHORT'};
export function validGuardReview(c:PublicCandidateCheck,x:FoundationPublicCandidate){
 const a=c.axes.find(a=>a.axis==='guard'),r=c.guardReview;
 if(!a)return false;
 if(!r)return a.state===2&&a.reason==='PUBLIC_GUARD_NOT_INSTALLED';
 if(a.reason!==reasons[r.reason]||r.policyPassed!==[PublicGuardReason.PUBLIC_GUARD_REASON_EXACT_SPEC_PASSED,PublicGuardReason.PUBLIC_GUARD_REASON_EXACT_FEATURE_PASSED].includes(r.reason))return false;
 if(r.reason===7)return a.state===2&&!r.object&&!r.objectSha256&&!c.outputInstance;
 if(r.reason===6)return a.state===2&&!r.object&&!r.objectSha256&&(!c.audio||!c.visual);
 if(!r.object||!/^[0-9a-f]{64}$/.test(r.objectSha256)||a.state!==(r.policyPassed?1:3)||!c.audio||!c.visual)return false;
 if(!/^\d+$/.test(r.checkedAtMs)||!/^\d+$/.test(r.validUntilMs)||BigInt(r.checkedAtMs)<1n||BigInt(r.validUntilMs)>8640000000000000n||BigInt(r.validUntilMs)<=BigInt(r.checkedAtMs)||BigInt(r.validUntilMs)-BigInt(r.checkedAtMs)>30000n)return false;
 if(!validBoundOutput(c.outputInstance))return false;
 const outputVisual=PublicMediaVisual.encode(c.outputInstance!.visual!).finish(),visual=PublicMediaVisual.encode(c.visual).finish();if(outputVisual.length!==visual.length||!outputVisual.every((b,i)=>b===visual[i]))return false;
 const expected=PublicGuardObject.fromPartial({protocolMajor:1,policyId:guardPolicy,workspaceId:x.workspaceId,sessionId:c.sessionId,epoch:c.epoch,previewId:c.previewId,previewSha256:c.previewSha256,taskId:c.taskId,taskSha256:c.taskSha256,snapshotId:c.snapshotId,snapshotSha256:c.snapshotSha256,factsSha256:c.factsSha256,captionSha256:c.captionSha256,outputTargetId:c.outputInstance!.outputId,audio:c.audio,visual:c.visual});
 const actual=PublicGuardObject.encode(r.object).finish(),bytes=PublicGuardObject.encode(expected).finish();
 return actual.length===bytes.length&&actual.every((b,i)=>b===bytes[i]);
}
export async function verifyGuardReviewHash(c:PublicCandidateCheck){
 const r=c.guardReview;if(!r?.object)return true;
 const bytes=Uint8Array.from(PublicGuardObject.encode(r.object).finish());
 const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
 return hash===r.objectSha256;
}


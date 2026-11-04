import {PublicOutputStatus,PublicOutputReason} from '@nai/contracts';
import {validWindowPrepared} from './window-prepared.ts';
import {ApiError} from './api.ts';
import {validPublicVisual,validOutputId} from './public-visual.ts';
export function decodePublicOutput(raw:unknown){
 if(!Array.isArray(raw)||!raw.length||raw.length>4096||raw.some(x=>!Number.isInteger(x)||x<0||x>255))throw new ApiError('PROTOCOL_ERROR');
 let s:PublicOutputStatus;try{s=PublicOutputStatus.decode(Uint8Array.from(raw));}catch{throw new ApiError('PROTOCOL_ERROR');}
 if(s.protocolMajor!==1||(s.reason===6)!==s.audioActive||!s.windowOpen||s.standby===s.audioActive||s.liveReady!==s.audioActive||s.audioActive!==!!s.windowPrepared?.playing||!/^\d+$/.test(s.seq)||!/^\d+$/.test(s.challenge)||!/^\d+$/.test(s.standbyFrames)||BigInt(s.standbyFrames)<1n||s.reason<1||s.reason>6||s.terminal!==(![PublicOutputReason.PUBLIC_OUTPUT_REASON_STANDBY_ONLY,PublicOutputReason.PUBLIC_OUTPUT_REASON_MEDIA_ACTIVE].includes(s.reason))||s.terminal!==(s.challenge==='0'))throw new ApiError('PROTOCOL_ERROR');
 if(!validPublicVisual(s.visual,s.visualName,s.visualLicense)||!validOutputId(s.outputId))throw new ApiError('PROTOCOL_ERROR');
 if(s.windowPrepared&&(!validWindowPrepared(s.windowPrepared,s.outputId)||s.terminal))throw new ApiError('PROTOCOL_ERROR');
 const p=s.presentation,w=s.windowPrepared,r=w?.receipt;
 if(p&&(!w?.playing||!r||p.preparationId!==w.preparationId||!p.submissionClock||!/^\d+$/.test(p.frameIndex)||BigInt(p.frameIndex)<1n||BigInt(p.frameIndex)>BigInt(r.expectedFrames)||!/^\d+$/.test(p.sampleStart)||BigInt(p.sampleStart)!==(BigInt(p.frameIndex)-1n)*800n||p.captionSha256!==w.captionSha256||!/^([a-f0-9]{64})$/.test(p.compositedSha256)||!/^\d+$/.test(p.presentationCount)||BigInt(p.presentationCount)<1n||BigInt(p.presentationCount)>BigInt(s.standbyFrames)))throw new ApiError('PROTOCOL_ERROR');
 return s;
}
const reasons:Record<number,string>={1:'待机窗口运行中',2:'控制心跳已到期，保持待机',3:'已停止，保持待机',4:'控制连接已断开，保持待机',5:'控制校验未通过，保持待机',6:'正在输出已批准本句'};
export const outputReason=(s:PublicOutputStatus)=>reasons[s.reason]??'状态未知';


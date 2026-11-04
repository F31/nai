import {AuditionStopNonce} from '@nai/contracts';
import {ApiError} from './api.ts';
import type {Invoke} from './client';
export function decodeAuditionNonce(raw:unknown){
 if(!Array.isArray(raw)||raw.length>128||raw.some(b=>!Number.isInteger(b)||b<0||b>255))throw new ApiError('PROTOCOL_ERROR',true);
 const v=AuditionStopNonce.decode(Uint8Array.from(raw));
 if(v.nonce&&!/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(v.nonce))throw new ApiError('PROTOCOL_ERROR',true);
 return v.nonce;
}
export async function verifyPrivateAudioHost(invoke:Invoke|undefined){
 if(!invoke)throw new ApiError('NATIVE_HOST_REQUIRED',true);
 try{await invoke('workbench_stop_audition');decodeAuditionNonce(await invoke('workbench_audition_nonce'));}
 catch(e){if(e instanceof ApiError)throw e;const text=String(e);if(/not allowed|not found|unknown command|permission/i.test(text))throw new ApiError('HOST_AUDIO_PERMISSION_MISSING',true);const code=text.replace(/^Error: /,'').replace(/^WB_/,'');throw new ApiError(/^[A-Z_]+$/.test(code)?code:'PROTOCOL_ERROR',true);}
}

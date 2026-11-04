export class TtsWaitError extends Error {
 readonly stage:'stop'|'submit'|'status';
 constructor(stage:'stop'|'submit'|'status'){super(stage==='stop'?'停止试听尚未确认，未发送合成请求。请重连后再操作。':stage==='submit'?'测试请求结果尚未确认。请核对原任务，勿重复合成；费用可能未知。':'读取任务状态超时，请稍后核对原任务，勿重复合成。');this.stage=stage;}
}
export class TtsActionGate {
 busy=false;
 async run<T>(action:()=>Promise<T>){if(this.busy)return;this.busy=true;try{return await action();}finally{this.busy=false;}}
}
export async function ttsDeadline<T>(request:Promise<T>,stage:'stop'|'submit'|'status',ms=6000):Promise<T>{
 let timer:ReturnType<typeof setTimeout>|undefined;
 try{return await Promise.race([request,new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new TtsWaitError(stage)),ms);})]);}
 finally{if(timer!==undefined)clearTimeout(timer);}
}
export async function submitTtsTest<T>(stop:()=>Promise<unknown>,active:()=>boolean,dispatch:()=>Promise<T>,onDispatch:()=>void,onResult:(value:T)=>void,ms=6000){
 await ttsDeadline(stop(),'stop',ms);
 if(!active())throw Error('已离开测试，未发送合成请求。');
 onDispatch();
 return ttsDeadline(dispatch().then(value=>{onResult(value);return value;}),'submit',ms);
}

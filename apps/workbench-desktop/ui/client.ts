import {Edition,Deployment,Behavior,DeliveryScope,Presentation,WorkbenchResponse,WorkbenchReadiness,WorkbenchImplementation} from '@nai/contracts';
export type Invoke=(name:string,args?:Record<string,unknown>)=>Promise<unknown>;
export async function inspect(invoke:Invoke,presentation:Presentation,prepare=false){
 const raw=await invoke('workbench_inspect',{presentation,prepare});
 if(!Array.isArray(raw)||raw.length>65536||raw.some(b=>!Number.isInteger(b)||b<0||b>255))throw Error('WB_PROTOCOL_ERROR');
 const r=WorkbenchResponse.decode(Uint8Array.from(raw));const c=r.outcome?.$case==='capabilities'?r.outcome.capabilities:undefined;
 if(!r.requestId||r.seq!=='1'||r.handshake?.protocolMajor!==2||r.handshake.maxFrameBytes!==65536||r.outcome?.$case==='problem'||!c||!c.config||c.config.presentation!==presentation||c.businessReady||c.providerCalls!==0||c.startedMediaProcesses!==0||c.config.edition!==Edition.EDITION_LITE||c.config.deployment!==Deployment.DEPLOYMENT_LOCAL||c.config.behavior!==Behavior.BEHAVIOR_ASSISTED||c.config.deliveryScope!==DeliveryScope.DELIVERY_SCOPE_PRIVATE||c.config.queueLimit!==2||c.config.taskDeadlineMs!==3000||c.config.shadow||c.capabilities.length===0||new Set(c.capabilities.map(i=>i.capabilityId)).size!==c.capabilities.length||c.capabilities.length>64||c.requiredCapabilities.length>64||c.status!==(prepare?WorkbenchReadiness.WORKBENCH_READINESS_NOT_READY:WorkbenchReadiness.WORKBENCH_READINESS_CONTROL_AVAILABLE)||c.capabilities.some(i=>!i.capabilityId||![WorkbenchImplementation.WORKBENCH_IMPLEMENTATION_IMPLEMENTED,WorkbenchImplementation.WORKBENCH_IMPLEMENTATION_NOT_STARTED].includes(i.implementation)||i.available&&i.capabilityId!=='workbench.inspect@1'))throw Error('WB_PROTOCOL_ERROR');
 return c;
}

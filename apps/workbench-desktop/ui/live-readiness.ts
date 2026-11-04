import type {Page} from './workbench';

export type ReadinessState='ready'|'action'|'confirm';
export type ReadinessItem={id:string;label:string;detail:string;state:ReadinessState;page?:Page;action?:string};
type Model={enabled:boolean;credentialSaved:boolean};
export type ObsLayerState='present'|'absent'|'unknown'|'binary'|'running';
export type ObsDetection={process:ObsLayerState;sceneSource:ObsLayerState;windowCapture:ObsLayerState;audioRouting:ObsLayerState;signal:ObsLayerState};
export type DestLayerState='present'|'absent'|'unknown';
export type DestReport={connection:DestLayerState;source:DestLayerState;signal:DestLayerState};
export type DestSurface={window:DestReport;virtualCamera:DestReport;tracks:{mix:DestReport;tts:DestReport;talent:DestReport};publicVia:'window'|'virtual-camera'|'independent-audio'};
export type LiveReadinessInput={publicationReady:boolean;llmCatalogReady:boolean;llmModels:Model[];ttsModels:Model[];customAvatarName?:string;outputOpen:boolean;outputTerminal:boolean;outputDevice:string;obs?:ObsDetection;dest?:DestSurface};

const LAYER_LABEL:{id:keyof ObsDetection;label:string}[]=[
 {id:'process',label:'进程'},
 {id:'sceneSource',label:'场景/来源'},
 {id:'windowCapture',label:'窗口捕获'},
 {id:'audioRouting',label:'声音路由'},
 {id:'signal',label:'实际信号'},
];

function layerText(state:ObsLayerState):string{
 if(state==='running'||state==='present')return '已观察';
 if(state==='binary')return '仅安装路径';
 if(state==='absent')return '未出现';
 return '未知';
}

export function obsDetectionDetail(obs:ObsDetection=unknownObs()):string{
 const layers=LAYER_LABEL.map(({id,label})=>`${label} ${layerText(obs[id])}`).join('；');
 const processSeen=obs.process==='running'||obs.process==='binary'||obs.process==='present';
 const othersPresent=[obs.sceneSource,obs.windowCapture,obs.audioRouting,obs.signal].every(state=>state==='present');
 return `${layers}。${processSeen&&!othersPresent?'进程存在或安装路径不是接入成功。':''}平台开播仍需在直播伴侣中人工确认。`;
}

export function unknownObs():ObsDetection{
 return {process:'unknown',sceneSource:'unknown',windowCapture:'unknown',audioRouting:'unknown',signal:'unknown'};
}

function destLayerText(state:DestLayerState):string{
 if(state==='present')return '已观察';
 if(state==='absent')return '未出现';
 return '未知';
}

function destReportText(label:string,report:DestReport):string{
 return `${label} 连接 ${destLayerText(report.connection)}、源 ${destLayerText(report.source)}、信号 ${destLayerText(report.signal)}`;
}

export function unknownDest():DestSurface{
 const unknown:DestReport={connection:'unknown',source:'unknown',signal:'unknown'};
 return {window:unknown,virtualCamera:unknown,tracks:{mix:unknown,tts:unknown,talent:unknown},publicVia:'window'};
}

export function destDetail(dest:DestSurface=unknownDest()):string{
 return `${destReportText('窗口',dest.window)}。${destReportText('虚拟摄像头',dest.virtualCamera)}。独立音轨：${destReportText('混合',dest.tracks.mix)}；${destReportText('讲解',dest.tracks.tts)}；${destReportText('真人',dest.tracks.talent)}。公共画面走窗口。虚拟摄像头只出画面。独立音轨只出声音。该报告不替代窗口输出，也不替代平台人工确认。`;
}

const OBS_LAYER:ObsLayerState[]=['present','absent','unknown','binary','running'];
const DEST_LAYER:DestLayerState[]=['present','absent','unknown'];

function isObsLayer(value:unknown):value is ObsLayerState{
 return typeof value==='string'&&OBS_LAYER.includes(value as ObsLayerState);
}
function isDestLayer(value:unknown):value is DestLayerState{
 return typeof value==='string'&&DEST_LAYER.includes(value as DestLayerState);
}
function destReport(value:unknown):DestReport|null{
 if(!value||typeof value!=='object')return null;
 const row=value as Record<string,unknown>;
 if(!isDestLayer(row.connection)||!isDestLayer(row.source)||!isDestLayer(row.signal))return null;
 return {connection:row.connection,source:row.source,signal:row.signal};
}

/** Closed-set parse of the native observe view. Fail closed to the caller. */
export function parseObserveView(raw:unknown):{obs:ObsDetection;dest:DestSurface}|null{
 let value:unknown=raw;
 if(typeof raw==='string'){
  try{value=JSON.parse(raw);}catch{return null;}
 }
 if(!value||typeof value!=='object')return null;
 const body=value as Record<string,unknown>;
 const obs=body.obs;
 const dest=body.dest;
 if(!obs||typeof obs!=='object'||!dest||typeof dest!=='object')return null;
 const o=obs as Record<string,unknown>;
 const d=dest as Record<string,unknown>;
 if(!isObsLayer(o.process)||!isObsLayer(o.sceneSource)||!isObsLayer(o.windowCapture)||!isObsLayer(o.audioRouting)||!isObsLayer(o.signal))return null;
 if(d.publicVia!=='window')return null;
 const window=destReport(d.window);
 const virtualCamera=destReport(d.virtualCamera);
 const tracks=d.tracks;
 if(!window||!virtualCamera||!tracks||typeof tracks!=='object')return null;
 const t=tracks as Record<string,unknown>;
 const mix=destReport(t.mix);
 const tts=destReport(t.tts);
 const talent=destReport(t.talent);
 if(!mix||!tts||!talent)return null;
 return {
  obs:{process:o.process,sceneSource:o.sceneSource,windowCapture:o.windowCapture,audioRouting:o.audioRouting,signal:o.signal},
  dest:{window,virtualCamera,tracks:{mix,tts,talent},publicVia:'window'},
 };
}

export function liveReadiness(input:LiveReadinessInput):ReadinessItem[]{
 const llmReady=input.llmCatalogReady&&input.llmModels.some(model=>model.enabled&&model.credentialSaved);
 const ttsReady=input.llmCatalogReady&&input.ttsModels.some(model=>model.enabled&&model.credentialSaved);
 const outputReady=input.outputOpen&&!input.outputTerminal;
 return [
  {id:'content',label:'直播内容',detail:input.publicationReady?'已加载当前发布版本':'需要复核并发布一个场次',state:input.publicationReady?'ready':'action',page:'release',action:input.publicationReady?'查看发布版本':'去发布'},
  {id:'llm',label:'文本模型',detail:llmReady?'已配置可用模型':'需要启用并保存一个文本模型凭据',state:llmReady?'ready':'action',page:'models',action:llmReady?'查看模型':'去配置'},
  {id:'tts',label:'语音模型',detail:ttsReady?'已配置可用语音模型':'需要启用并保存一个语音模型凭据',state:ttsReady?'ready':'action',page:'models',action:ttsReady?'查看模型':'去配置'},
  {id:'avatar',label:'数字人形象',detail:input.customAvatarName?`已选择 ${input.customAvatarName}；公共输出仍使用内置形象`:'使用内置基础二维形象',state:input.customAvatarName?'action':'ready',page:'avatar',action:'管理形象'},
  {id:'output',label:'画面与声音',detail:outputReady?`公共窗口待机中${input.outputDevice?` · ${input.outputDevice}`:''}`:'需要打开公共待机窗口并核对音频设备',state:outputReady?'ready':'action',page:'output',action:outputReady?'查看输出':'去准备'},
  {id:'platform',label:'OBS 与直播平台',detail:`${obsDetectionDetail(input.obs)} ${destDetail(input.dest)}`,state:'confirm',page:'output',action:'查看接入说明'},
 ];
}

import assert from 'node:assert/strict';
import test from 'node:test';
import {destDetail,liveReadiness,obsDetectionDetail,parseObserveView,unknownDest,unknownObs} from '../../apps/workbench-desktop/ui/live-readiness.ts';

test('readiness derives only from published, credential and native output facts',()=>{
 const items=liveReadiness({publicationReady:true,llmCatalogReady:true,llmModels:[{enabled:true,credentialSaved:true}],ttsModels:[{enabled:true,credentialSaved:true}],outputOpen:true,outputTerminal:false,outputDevice:'Speaker'});
 assert.deepEqual(items.slice(0,5).map(item=>item.state),['ready','ready','ready','ready','ready']);
 assert.equal(items[5].state,'confirm');
 assert.match(items[4].detail,/Speaker/);
 assert.match(items[5].detail,/进程 未知/);
 assert.match(items[5].detail,/实际信号 未知/);
});

test('missing or terminal facts stay actionable and platform never becomes ready',()=>{
 const items=liveReadiness({publicationReady:false,llmCatalogReady:false,llmModels:[{enabled:true,credentialSaved:true}],ttsModels:[],outputOpen:true,outputTerminal:true,outputDevice:''});
 assert.deepEqual(items.slice(0,5).map(item=>item.state),['action','action','action','ready','action']);
 assert.equal(items.find(item=>item.id==='platform')?.state,'confirm');
});

test('a running OBS process does not become connected or platform-ready',()=>{
 const items=liveReadiness({
  publicationReady:true,llmCatalogReady:true,
  llmModels:[{enabled:true,credentialSaved:true}],
  ttsModels:[{enabled:true,credentialSaved:true}],
  outputOpen:true,outputTerminal:false,outputDevice:'Speaker',
  obs:{process:'running',sceneSource:'unknown',windowCapture:'unknown',audioRouting:'unknown',signal:'unknown'},
 });
 const platform=items.find(item=>item.id==='platform');
 assert.equal(platform?.state,'confirm');
 assert.match(platform?.detail??'',/进程 已观察/);
 assert.match(platform?.detail??'',/窗口捕获 未知/);
 assert.match(platform?.detail??'',/进程存在或安装路径不是接入成功/);
 assert.match(platform?.detail??'',/平台开播仍需/);
});

test('an install path is reported separately from a running process',()=>{
 assert.match(obsDetectionDetail({process:'binary',sceneSource:'unknown',windowCapture:'unknown',audioRouting:'unknown',signal:'unknown'}),/仅安装路径/);
});

test('optional destinations stay unknown and public picture stays the window',()=>{
 const items=liveReadiness({publicationReady:true,llmCatalogReady:true,llmModels:[{enabled:true,credentialSaved:true}],ttsModels:[{enabled:true,credentialSaved:true}],outputOpen:true,outputTerminal:false,outputDevice:'Speaker'});
 const platform=items.find(item=>item.id==='platform');
 assert.equal(platform?.state,'confirm');
 assert.match(platform?.detail??'',/窗口 连接 未知/);
 assert.match(platform?.detail??'',/虚拟摄像头 连接 未知/);
 assert.match(platform?.detail??'',/混合 连接 未知/);
 assert.match(platform?.detail??'',/讲解 连接 未知/);
 assert.match(platform?.detail??'',/真人 连接 未知/);
 assert.match(platform?.detail??'',/公共画面走窗口/);
 assert.match(platform?.detail??'',/虚拟摄像头只出画面/);
 assert.match(platform?.detail??'',/独立音轨只出声音/);
 assert.equal(unknownDest().publicVia,'window');
});

test('an observed virtual camera still keeps the public picture on the window',()=>{
 const present={connection:'present',source:'present',signal:'present'};
 const detail=destDetail({window:unknownDest().window,virtualCamera:present,tracks:unknownDest().tracks,publicVia:'window'});
 assert.match(detail,/虚拟摄像头 连接 已观察、源 已观察、信号 已观察/);
 assert.match(detail,/窗口 连接 未知/);
 assert.match(detail,/公共画面走窗口/);
 assert.match(detail,/不替代窗口输出/);
});

test('custom avatar selection stays action and public remains builtin',()=>{
 const items=liveReadiness({publicationReady:true,llmCatalogReady:true,llmModels:[{enabled:true,credentialSaved:true}],ttsModels:[{enabled:true,credentialSaved:true}],customAvatarName:'演示形象',outputOpen:true,outputTerminal:false,outputDevice:'Speaker'});
 const avatar=items.find(item=>item.id==='avatar');
 assert.equal(avatar?.state,'action');
 assert.match(avatar?.detail??'',/演示形象/);
 assert.match(avatar?.detail??'',/内置形象/);
});

test('parseObserveView accepts camelCase and keeps publicVia window',()=>{
 const parsed=parseObserveView(JSON.stringify({
  obs:{process:'running',sceneSource:'present',windowCapture:'present',audioRouting:'present',signal:'unknown'},
  dest:{
   window:{connection:'present',source:'present',signal:'unknown'},
   virtualCamera:{connection:'present',source:'present',signal:'present'},
   tracks:{mix:{connection:'unknown',source:'unknown',signal:'unknown'},tts:{connection:'unknown',source:'unknown',signal:'unknown'},talent:{connection:'unknown',source:'unknown',signal:'unknown'}},
   publicVia:'window',
  },
  obsReadiness:'blocked',
  destReadiness:'window_public',
 }));
 assert.equal(parsed?.obs.process,'running');
 assert.equal(parsed?.dest.virtualCamera.signal,'present');
 assert.equal(parsed?.dest.publicVia,'window');
 const items=liveReadiness({publicationReady:true,llmCatalogReady:true,llmModels:[{enabled:true,credentialSaved:true}],ttsModels:[{enabled:true,credentialSaved:true}],outputOpen:true,outputTerminal:false,outputDevice:'Speaker',obs:parsed?.obs,dest:parsed?.dest});
 assert.equal(items.find(item=>item.id==='platform')?.state,'confirm');
 assert.match(items.find(item=>item.id==='platform')?.detail??'',/公共画面走窗口/);
});

test('parseObserveView fails closed for a non-window publicVia',()=>{
 assert.equal(parseObserveView({obs:unknownObs(),dest:{...unknownDest(),publicVia:'virtual-camera'}}),null);
 assert.equal(parseObserveView('{'),null);
 assert.equal(parseObserveView({obs:unknownObs()}),null);
});

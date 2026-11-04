import {useEffect,useState} from 'react';
import type {Workbench} from './workbench';
import {privateAvailable} from './api';
import type {PublicOutputStatus} from '@nai/contracts';
import {liveReadiness,unknownDest,unknownObs,type DestSurface,type ObsDetection} from './live-readiness';
export function PreparationHome({v,output=null,openOutput,closeOutput,outputBusy=false,obs,dest}:{v:Workbench;output?:PublicOutputStatus|null;openOutput:()=>Promise<void>;closeOutput:()=>Promise<void>;outputBusy?:boolean;obs?:ObsDetection;dest?:DestSurface}){
 const [avatarChoice,setAvatarChoice]=useState<string|null>(null),[avatarLoading,setAvatarLoading]=useState(false),[avatarError,setAvatarError]=useState(false);
 const avatarRecords=v.records.filter(r=>r.kind==='avatar_profile_draft');
 const avatarKey=JSON.stringify(avatarRecords.map(r=>[r.id,r.revision]));
 useEffect(()=>{let current=true;setAvatarChoice(null);setAvatarError(false);if(!v.connected||!v.ws){setAvatarLoading(false);return ()=>{current=false;};}setAvatarLoading(true);const items=v.records.filter(r=>r.kind==='avatar_profile_draft');Promise.all(items.map(r=>v.api.read(v.ws,r.id))).then(results=>{if(!current)return;const selected=results.map(r=>JSON.parse(r.body)).filter(d=>d.kind==='avatar_profile_draft'&&d.selected===true&&d.revoked===false);if(selected.length>1)throw Error('multiple selected avatars');if(selected[0]&&typeof selected[0].name!=='string')throw Error('invalid avatar name');setAvatarChoice(selected[0]?.name??null);}).catch(()=>{if(current)setAvatarError(true);}).finally(()=>{if(current)setAvatarLoading(false);});return ()=>{current=false;};},[v.ws,v.connected,avatarKey]);
 const items=liveReadiness({
  publicationReady:v.records.some(r=>r.kind==='publication_snapshot'),
  llmCatalogReady:v.llm.catalogState==='ready',
  llmModels:v.llm.models,
  ttsModels:v.llm.ttsModels,
  customAvatarName:avatarChoice??(v.selectedAvatarName||undefined),
  outputOpen:!!output,
  outputTerminal:!!output?.terminal,
  outputDevice:output?.windowPrepared?.deviceName??'',
  obs:obs??unknownObs(),
  dest:dest??unknownDest(),
 });
 const mark=items.map(item=>item.state==='ready'?'✓':item.state==='action'?'!':'i');
 return <><div className="page-title"><div><h1>直播工作台</h1><p>打开公共画面，准备直播内容。</p></div></div><article><p>公共画面形象：内置二维数字人</p><p>工作区形象选择：{!v.connected?'连接后查看':avatarLoading?'读取中':avatarError?'读取失败，请进入形象管理重试':avatarChoice?`${avatarChoice}（尚未用于公共画面）`:'内置二维数字人'}</p><button disabled={v.busy||v.dirty} onClick={()=>void v.changePage('avatar')}>管理自定义形象</button><p className="hint">目前公共画面使用内置形象，语音播出时驱动嘴型；自定义形象可导入并私有预览，选择不是播放许可。</p><div className="quick-actions"><button className="primary large" disabled={!v.connected||v.busy||v.dirty||outputBusy||!!output&&!output.terminal} onClick={()=>void openOutput()}>{output&&!output.terminal?'公共画面已打开':'打开画面与输出'}</button>{output&&<button disabled={outputBusy} onClick={()=>void closeOutput()}>关闭公共画面</button>}</div>{output?.terminal&&<p role="alert">公共画面已停止，请关闭后重新打开。</p>}<p className="hint">公共窗口可供 OBS 捕获；平台开播仍需在直播软件中操作。</p><ul className="readiness-list">{items.map((item,index)=><li key={item.id} className={`readiness-item ${item.state}`}><span className="readiness-state" aria-hidden="true">{mark[index]}</span><div><strong>{item.label}</strong><p>{item.detail}</p></div>{item.page&&<button disabled={v.busy||v.dirty} onClick={()=>void v.changePage(item.page!)}>{item.action}</button>}</li>)}</ul></article></>;
}
export function ManualPage({v}:{v:Workbench}){
 const [note,setNote]=useState(''),run=v.manualRun,doc=run?.doc;
 useEffect(()=>setNote(''),[v.manualInputVersion]);
 useEffect(()=>{const stop=(e:KeyboardEvent)=>{if(e.key==='Escape'){e.preventDefault();void v.stop();}};window.addEventListener('keydown',stop);return()=>window.removeEventListener('keydown',stop);},[v.stop]);
 if(!doc)return <article><h1>手动提词</h1><p>选择已发布场次开始，或继续已有记录。</p>{v.records.filter(r=>r.kind==='manual_run_draft').map(r=><div className="toolbar" key={r.id}><strong>{r.name}</strong><button disabled={v.busy||v.dirty} onClick={()=>void v.run(()=>v.openManual(r))}>打开记录</button></div>)}{v.shows.map(s=><div className="toolbar" key={s.id}><strong>{s.name}</strong><button disabled={v.busy||v.dirty||!v.records.some(r=>r.kind==='publication_snapshot'&&r.parentId===s.id)} onClick={()=>void v.run(()=>v.startManual(s))}>开始手动提词</button></div>)}{!v.shows.length&&<p>请先在“场次与节目”准备场次并发布。</p>}</article>;
 const open=doc.status==='open',ready=!!v.snapshot&&privateAvailable(v.snapshot)&&v.snapshotMeta?.id===doc.snapshot.object_id&&v.snapshot.sha256===doc.snapshot.sha256,disabled=v.busy||!open||!ready||v.stopPending;
 return <><div className="page-title"><div><h1>{doc.name}</h1><p>{open?'手动记录进行中':'本场已结束'} · 私有文字 · 无模型调用</p></div><div className="row-actions"><button disabled={v.busy||v.dirty} onClick={()=>void v.run(()=>v.openManual(run!.meta))}>重新读取记录</button><button disabled={v.busy||v.dirty} onClick={()=>void v.changePage('home')}>备播首页</button></div></div>
 <article><h2>单元 {doc.cursor+1} / {doc.cue_ids.length} · {v.snapshotCues.find(c=>c.id===doc.cue_ids[doc.cursor])?.title??doc.cue_ids[doc.cursor]}</h2>{!ready&&<p role="alert">绑定内容尚未加载或已失效，请从首页重新打开记录；仍可结束并保存人工标记。</p>}<div className="quick-actions"><button className="large" disabled={disabled||doc.cursor===0} onClick={()=>void v.run(()=>v.moveManual(doc.cursor-1))}>上一段</button><button className="primary large" disabled={disabled} onClick={()=>void v.run(()=>v.moveManual(doc.cursor))}>显示本段</button><button className="large" disabled={disabled||doc.cursor+1>=doc.cue_ids.length} onClick={()=>void v.run(()=>v.moveManual(doc.cursor+1))}>下一段</button><button className="danger large" disabled={v.busy||!open} onClick={()=>void v.run(v.endManual)}>结束本场</button></div>{v.card&&open?<div className="preview-card" aria-label="手动提词内容"><p>{v.card.value.text}</p><small>仅本工作台；停止、过期或失联会清空。</small></div>:<p className="empty">点击“显示本段”后重新核验内容。恢复记录不会自动重放。</p>}<button className="danger" disabled={!v.card&&!v.session.current&&!v.stopPending} onClick={()=>void v.stop()}>停止显示（Esc）</button><p className="hint">进度和标记是人工操作记录，不代表用户已读、音频已播放或直播已播出。</p></article>
 {!open&&<article><h2>准备下场</h2><p>把本场重点整理为FAQ修改，核对后保存独立下场草稿。</p><button disabled={v.busy||v.dirty} onClick={()=>void v.changePage('review')}>复盘并准备下场</button></article>}<article><h2>人工标记</h2><form onSubmit={e=>{e.preventDefault();void v.run(async()=>{await v.markManual(note);setNote('');v.setDirty(false);});}}><label>本段问题或下场改进<textarea maxLength={256} value={note} disabled={v.busy||doc.marks.length>=8} onChange={e=>{setNote(e.target.value);v.setDirty(!!e.target.value);}}/></label><button disabled={v.busy||!note.trim()||doc.marks.length>=8}>保存标记（{doc.marks.length}/8）</button><button type="button" disabled={v.busy||!note} onClick={()=>{setNote('');v.setDirty(false);}}>放弃本条输入</button></form>{doc.marks.map(m=><p key={m.id}><small>单元 {doc.cue_ids.indexOf(m.cue_id)+1}</small> · {m.note}</p>)}<p className="hint">标记仅保存为复盘输入，不自动改写FAQ或发布。</p></article></>;
}







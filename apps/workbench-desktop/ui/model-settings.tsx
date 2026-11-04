import {TtsTest} from './tts-test';
import {useState} from 'react';
import type {LlmModelView,TtsModelView} from '@nai/contracts';
import type {Workbench} from './workbench';
import {LlmEditor,ModelTest} from './llm-panel';
import {modelForm,yuan} from './llm';
import {ttsForm,ttsReady,qwenLanguages} from './tts';
import {TtsEditor} from './tts-editor';
type Props={v:Workbench};
type Tab='llm'|'tts';
export function LlmSettingsPage({v}:Props){
 const l=v.llm,[tab,setTab]=useState<Tab>('llm'),[testing,setTesting]=useState(''),[editing,setEditing]=useState({llm:false,tts:false}),[deleting,setDeleting]=useState<{model:LlmModelView|TtsModelView;tab:Tab}|null>(null);
 const speech=tab==='tts',models=l.shared?(speech?l.ttsModels:l.models):[],blocked=v.busy||l.probe.running||l.catalogState!=='ready'||!l.shared,editor=editing[tab];
 const close=()=>{l.clearKeys();setEditing(e=>({...e,[tab]:false}));};
 function selectTab(next:Tab){if(next===tab)return;void v.run(async()=>{await l.suspend();l.clearKeys();setDeleting(null);setTesting('');l.clearNotice();setTab(next);requestAnimationFrame(()=>document.getElementById(`models-tab-${next}`)?.focus());});}
 function edit(m?:LlmModelView|TtsModelView){setDeleting(null);setTesting('');if(speech)l.setTts(ttsForm(m as TtsModelView|undefined));else l.setForm(modelForm(m as LlmModelView|undefined));setEditing(e=>({...e,[tab]:true}));}
 return <><h1 className="model-heading">模型设置</h1><p className="hint">模型由所有工作区共用；费用与预算分别归属使用它的工作区。</p>
 <div className="model-tabs" role="tablist" aria-label="模型类型">{([['llm','LLM 文本模型'],['tts','TTS 语音模型']] as const).map(([id,label])=><button type="button" role="tab" id={`models-tab-${id}`} aria-selected={tab===id} aria-controls="model-catalog-panel" tabIndex={tab===id?0:-1} className={tab===id?'active':''} disabled={v.busy} key={id} onClick={()=>selectTab(id)} onKeyDown={e=>{if(['ArrowLeft','ArrowRight','Home','End'].includes(e.key)){e.preventDefault();selectTab(e.key==='Home'?'llm':e.key==='End'?'tts':tab==='llm'?'tts':'llm');}}}>{label}</button>)}</div>
 {l.notice&&<p className="model-notice" role="status">{l.notice}</p>}
 <div id="model-catalog-panel" role="tabpanel" aria-labelledby={`models-tab-${tab}`}><article className="model-catalog"><div className="row-head"><h2>{speech?'语音模型':'文本模型'} <span className="tag">{l.catalogState==='ready'?`已配置 ${models.length} 个`:'读取中'}</span></h2><div className="row-actions"><button disabled={v.busy||l.probe.running||l.catalogState==='loading'} onClick={()=>void v.run(l.load)}>刷新列表</button><button className="primary" disabled={blocked||editor||models.length>=8} onClick={()=>edit()}>＋ 新增{speech?'语音':'文本'}模型</button></div></div>
 {speech&&<p className="hint">点击即可合成固定短句；音频短期缓存，可本机试听、停止或释放。</p>}
 {deleting?.tab===tab&&<div className="banner" role="alert"><p>确认删除“{deleting.model.name}”？历史费用记录保留。</p><div className="actions"><button className="danger" disabled={blocked} onClick={()=>void v.run(async()=>{await l.removeModel(deleting.model,speech);setDeleting(null);setTesting('');})}>确认删除</button><button disabled={v.busy} onClick={()=>setDeleting(null)}>取消删除</button></div></div>}
 {l.catalogState==='loading'?<p role="status">正在加载模型配置，首次准备可能需要稍候…</p>:l.catalogState==='error'?<p role="alert">{l.catalogFailure}</p>:<>{models.length>=8&&<p className="hint">当前每类最多支持 8 个模型；请编辑或删除现有模型后再新增。</p>}{models.length?<div className="model-table-wrap"><table className="model-table"><thead><tr><th>名称 / 模型</th><th>{speech?'音色':'单价（元 / 百万 token）'}</th><th>状态</th><th>操作</th></tr></thead><tbody>{models.map(m=><tr key={m.id}><td><strong>{m.name}</strong><small>{m.model}</small></td><td>{'voice' in m?<><span>{m.voice}</span><small>{m.protocol==='qwen_local'?(qwenLanguages.find(([id])=>id===m.language)?.[1]??m.language):m.priceKnown?`¥${yuan(m.characterMinorPerMillion)} / 百万字符`:'单价未设置'}</small></>:m.pricingUnknown?<span>单价未设置</span>:<><span>输入 ¥{yuan(m.inputMinorPerMillion)}</span><small>输出 ¥{yuan(m.outputMinorPerMillion)}</small></>}</td><td><span className={m.enabled?'good':'hint'}>{m.enabled?'启用':'停用'}</span><small>{'voice' in m&&m.protocol==='qwen_local'?'本地服务 · 无需Key':m.credentialSaved?'Key 已保存':'未配置 Key'}</small></td><td><div className="row-actions"><button disabled={blocked||editor} onClick={()=>edit(m)} aria-label={`编辑 ${m.name}`}>编辑</button>{speech&&<button disabled={blocked||editor||!ttsReady(m as TtsModelView)} onClick={()=>{setTesting(m.id);setDeleting(null);}} aria-label={'测试合成 '+m.name}>测试合成</button>}{!speech&&<button disabled={blocked||editor||!m.enabled||!m.credentialSaved} onClick={()=>{l.setForm(modelForm(m as LlmModelView));setTesting(m.id);setDeleting(null);}} aria-label={'测试 '+m.name}>测试</button>}<button className="quiet" disabled={blocked||editor} onClick={()=>setDeleting({model:m,tab})} aria-label={`删除 ${m.name}`}>删除</button></div></td></tr>)}</tbody></table></div>:<p className="empty">尚未添加{speech?'语音':'文本'}模型，点击右上方新增即可。</p>}</>}
 {speech&&testing&&!editor&&l.ttsModels.find(m=>m.id===testing)&&<TtsTest key={testing+":"+l.ttsModels.find(m=>m.id===testing)!.revision} v={v} model={l.ttsModels.find(m=>m.id===testing)!}/>}
 {!speech&&testing&&!editor&&<section className="model-editor"><div className="row-head"><h2>测试 · {l.models.find(m=>m.id===testing)?.name}</h2><button disabled={v.busy} onClick={()=>void v.run(async()=>{await l.suspend();setTesting('');})}>关闭测试</button></div><ModelTest v={v}/></section>}
 {editor&&(speech?<TtsEditor v={v} onSaved={close} onCancel={close}/>:<LlmEditor v={v} onSaved={close} onCancel={close}/>)}
 </article></div>
 </>;
}

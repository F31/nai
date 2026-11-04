import {ReviewPage} from './review-page';
import {HumanSafety} from './human-safety';
import {usePublicOutput,PublicOutputPanel} from './public-output-panel';
import {useObsObserve} from './use-obs-observe';
import {PreparationHome,ManualPage} from './manual-pages';
import {WorkspaceBudgetFields} from './workspace-budget-fields';
import {useEffect,useRef} from 'react';
import {createRoot} from 'react-dom/client';
import {useWorkbench,type Page,type Workbench} from './workbench';
import {ProductPage,ShowPage,ReleasePage,ProfilePage} from './pages';
import {LlmSettingsPage} from './model-settings';
import './style.css';
import {WorkspaceManager} from './workspace-manager';
import {AvatarPage} from './avatar-page';
const workNavigation:[string,[Page,string,string][]][]=[['开始', [['home','⌂','直播工作台']]],['准备', [['product','◈','产品与话术'],['show','☷','场次与节目'],['release','▷','发布与预演']]],['直播', [['manual','▤','手动提词']]],['结束', [['review','↗','复盘与下场']]]];
const settingsNavigation:[Page,string,string][]=[['models','◇','模型设置'],['profile','♧','主持与系列'],['workspace','▣','工作区管理'],['diagnostics','⌁','能力检查']];
const isSettings=(page:Page)=>settingsNavigation.some(([id])=>id===page);
function WorkspaceForm({v}: {v:Workbench}){return <form className="workspace-create" onSubmit={e=>{e.preventDefault();void v.run(v.createWorkspace);}}><label>工作区名称<input required maxLength={128} disabled={v.busy||v.dirty||v.workspaceCreatePending} value={v.workspaceName} onChange={e=>v.setWorkspaceName(e.target.value)}/></label><WorkspaceBudgetFields mode={v.workspaceBudgetMode} limit={v.workspaceBudgetLimit} onMode={v.setWorkspaceBudgetMode} onLimit={v.setWorkspaceBudgetLimit} disabled={v.busy||v.workspaceCreatePending}/><button className="primary" disabled={v.busy||v.dirty||!v.connected||!v.workspaceName.trim()}>{v.workspaceCreatePending?'恢复切换':'创建工作区'}</button></form>;}
function App(){
 const publicOutput=usePublicOutput(),v=useWorkbench(),obsObserve=useObsObserve(v.connected&&(v.page==='home'||v.page==='output')),settings=isSettings(v.page),lastWork=useRef<Page>('home'),lastSettings=useRef<Page>('models');
 useEffect(()=>{if(isSettings(v.page))lastSettings.current=v.page;else lastWork.current=v.page;},[v.page]);
 const back=()=>v.changePage(lastWork.current),openSettings=()=>v.changePage(v.ws?lastSettings.current:'workspace');
 const navigation=settings?[["设置",settingsNavigation] as [string,[Page,string,string][]]]:workNavigation;
 return <main><header><div className="brand"><span className="brand-mark">n</span><div><strong>nai 直播工作台</strong><small>备播 · 提词 · 逐段讲品</small></div></div><div className="header-actions"><HumanSafety/>{publicOutput.status&&<button className="danger" onClick={()=>void publicOutput.stop()}>数字人静音</button>}<span className={v.connected?'connection connected':'connection'}>{v.connected?'● 已连接':'○ 未连接'}</span><button disabled={v.busy||v.dirty} onClick={()=>void v.run(v.connect)}>{v.connected?'重新连接':'连接'}</button><button className={settings?'settings-toggle nav-active':'settings-toggle'} aria-label={settings?'返回控制台':'设置'} aria-pressed={settings} disabled={v.busy||v.dirty} onClick={()=>void(settings?back():openSettings())}><span aria-hidden="true">{settings?'←':'⚙'}</span> {settings?'返回控制台':'设置'}</button></div></header>
 <div className="shell"><nav aria-label={settings?'设置菜单':'工作导航'} className={settings?'settings-navigation':'work-navigation'}><div className="nav-title">{settings?'设置':'工作台'}</div>{navigation.map(([group,items])=><div className="nav-group" key={group}><div className="nav-group-title">{group}</div>{items.map(([id,icon,label])=><button key={id} className={v.page===id?'nav-active':''} aria-current={v.page===id?'page':undefined} disabled={v.busy||v.dirty&&v.page!==id||!v.ws&&!['home','output','workspace','diagnostics'].includes(id)} onClick={()=>void v.changePage(id)}><span aria-hidden="true">{icon}</span>{label}</button>)}</div>)}<div className="nav-note">私有预演仅工作台可见<br/>公共播出需单独确认</div></nav>
 <section className="workspace">{settings&&<div className="settings-bar"><span>设置</span></div>}{v.ws&&!settings&&<div className="workspace-bar compact">{v.workspaces.length>1?<label>当前工作区<select value={v.ws} disabled={v.busy||v.dirty} onChange={e=>void v.run(()=>v.selectWorkspace(e.target.value))}>{v.workspaces.map(w=><option key={w.id} value={w.id}>{w.name}</option>)}</select></label>:<span className="workspace-name">工作区 · {v.workspaces.find(w=>w.id===v.ws)?.name}</span>}</div>}
 {v.error&&<div className="banner error" role="alert">{v.error}</div>}{v.notice&&<div className="banner" role="status">{v.notice}</div>}{v.dirty&&<div className="banner unsaved">有未保存修改，切换前请保存。<button disabled={v.busy} onClick={()=>void v.run(v.discard)}>放弃本次修改</button></div>}
 {!v.connected&&<article><h1>连接工作台</h1><p>连接后读取本机已保存的内容和设置。</p><button className="primary" onClick={()=>void v.run(v.connect)} disabled={v.busy}>连接</button></article>}
 {v.connected&&!v.ws&&!settings&&<article className="welcome"><h1>创建工作区</h1><p>为这间直播间保存产品、场次和预算，名称以后可区分不同工作区。</p><WorkspaceForm v={v}/></article>}
 {v.page==='output'&&<PublicOutputPanel v={publicOutput} connected={v.connected} obs={obsObserve.obs} dest={obsObserve.dest}/>}{v.connected&&v.ws&&v.page==='home'&&<><PreparationHome v={v} output={publicOutput.status} openOutput={publicOutput.open} closeOutput={publicOutput.close} outputBusy={publicOutput.busy} obs={obsObserve.obs} dest={obsObserve.dest}/>{publicOutput.error&&<p role="alert" className="banner error">{publicOutput.error}</p>}</>}{v.connected&&v.ws&&v.page==='manual'&&<ManualPage key={v.ws+':'+v.manualRun?.meta.id} v={v}/>}{v.connected&&v.ws&&v.page==='product'&&<ProductPage v={v}/>}{v.connected&&v.ws&&v.page==='show'&&<ShowPage v={v}/>}{v.connected&&v.ws&&v.page==='release'&&<ReleasePage v={v}/>}{v.connected&&v.ws&&v.page==='profile'&&<ProfilePage v={v}/>}{v.connected&&v.ws&&v.page==='models'&&<LlmSettingsPage v={v}/>} 
 {v.connected&&v.ws&&v.page==='avatar'&&<AvatarPage key={v.ws} v={v}/>} {v.connected&&v.ws&&v.page==='review'&&<ReviewPage key={v.ws} v={v}/>} {v.page==='workspace'&&v.connected&&<><div className="page-title"><div><h1>工作区管理</h1><p>一个直播间使用一个工作区；内容和预算分别保存，模型由所有工作区共用。</p></div></div><WorkspaceManager v={v}/></>}
 {v.page==='diagnostics'&&<><article><h1>能力检查</h1><p>查看本机引擎及能力状态，不会调用模型。</p><button disabled={v.busy} onClick={()=>void v.run(v.diagnose)}>运行能力检查</button>{v.diagnostic&&<p role="status">{v.diagnostic}</p>}</article></>}
 </section></div></main>;
}
createRoot(document.getElementById('root')!).render(<App/>);









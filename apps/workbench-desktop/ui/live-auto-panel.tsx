import {useEffect,useState} from 'react';
import {ttsReady} from './tts';
import {minor,yuan} from './llm';
import type {Workbench} from './workbench';
export function LiveAutoPanel({v}:{v:Workbench}){
 const [selected,setSelected]=useState(''),[budget,setBudget]=useState('1.00'),[rights,setRights]=useState(false);
 const c=v.liveRun.context,p=v.liveRun.speechPlan,a=v.liveAuto,s=a?.status,models=(v.llm?.ttsModels??[]).filter(ttsReady),model=models.find(m=>m.id===selected)??models.find(m=>m.protocol==='qwen_local')??models[0];
 useEffect(()=>setRights(false),[c?.runId,c?.epoch,p?.planSha256,model?.id,model?.revision,budget,s?.grantId]);
 if(!a||!c?.sentenceMode||c.state===7)return null;
 const reasons:Record<string,string>={RESOURCE_EXHAUSTED:'本次上限或工作区预算不足',TTS_FAILED:'语音合成失败或结果失效，未重试',POLICY_BLOCKED:'当前内容未通过公共播出守门',MEDIA_NOT_READY:'实际音画未在期限内就绪',AUTHORITY_UNAVAILABLE:'授权、资料或停止状态已变化',OPERATOR_STOP:'人工停止',COMPLETED:'全部句已提交',AUTHORIZED:'已建立本次授权',AUTO_FAILED:'执行失败，已停止后续消费'};
 const validText=!!p?.batchApproval?.approved&&BigInt(p.batchApproval.expiresAtMs)>BigInt(v.clock??Date.now());
 const labels:Record<string,string>={question_pausing:'已选择插问，等待本句结束',question_waiting:'本句已结束，正在准备插问回答',preparing:'正在准备或等待下一句，缺音频时保持待机',playing:'正在向公共窗口提交当前句',pausing:'本句结束后暂停，当前句仍在提交',paused:'已暂停后续消费，等待明确继续',completed:'本次自动讲品已结束',stopped:'本次自动讲品已停止',failed:'自动讲品失败，未自动重试',blocked:'授权或资料已失效，未自动恢复'};
 return <section className="model-editor" aria-label="自动讲品"><h3>自动讲品</h3><p>最多准备当前与下一句。逐句核验后自动衔接；短句生成跟不上时明确等待，不重复播报。仅本机公共窗口，不推流。</p>
 {s&&<><p role="status">{labels[s.state]??'状态不可用'}{s.reason&&` · ${reasons[s.reason]??s.reason}`}</p><p>授权截至 {new Date(Number(s.expiresAtMs)).toLocaleTimeString()} · 本次上限占用 ¥{yuan(s.reservedMinor)} / ¥{yuan(s.maxCostMinor)}，不是供应商实际账单。</p></>}
 {!a.active&&<>{!model?<p>请先在设置添加可用语音模型。</p>:<>
 {models.length>1?<label>语音模型<select disabled={v.busy} value={model.id} onChange={e=>setSelected(e.target.value)}>{models.map(m=><option key={m.id} value={m.id}>{m.name} · {m.voice}</option>)}</select></label>:<p>语音模型：{model.name} · {model.voice}</p>}
 <label>本次总调用上限（元）<input inputMode="decimal" disabled={v.busy} value={budget} onChange={e=>setBudget(e.target.value)}/></label>
 <p>每句最多占用 ¥0.10，累计受本次上限和工作区预算约束。有效期随当前文本批准到期；实际费用以供应商为准，无回执保持未知。</p>
 <label className="check"><input type="checkbox" disabled={v.busy||!validText} checked={rights} onChange={e=>setRights(e.target.checked)}/>我已核对全部文本和上述音色，确认语音及内置MIT形象的使用权，授权在上限内调用模型并向本机公共窗口自动播出</label>
 <button className="primary" disabled={v.busy||!rights||!validText||c.state!==1} onClick={()=>void v.run(()=>a.start(model,minor(budget)))}>开始自动讲品</button></>}{!validText&&<p>请先展开分句计划，核对并批准全部文本。</p>}</>}
 {a.status?.interruptionId&&<p role="status">插问：{a.status.question||'已处理问题'} · {({pending:'等待句边界',preparing:'准备回答',playing:'正在回答',completed:'已回答并归位',cancelled:'已取消，未自动恢复'} as Record<string,string>)[a.status.interruptionState]??a.status.interruptionState}{a.status.answer&&<span> · 回答：{a.status.answer}</span>}</p>}
 {a.active&&['pending','preparing','playing'].includes(a.status?.interruptionState??'')&&<button className="danger" disabled={v.busy} onClick={()=>void v.run(()=>a.interrupt('','',true))}>取消插问并停止自动讲品</button>}
 {a.active&&<button className="danger" disabled={v.busy} onClick={()=>void v.run(a.stop)}>停止本次自动讲品</button>}
 <p className="hint">暂停/继续使用队列按钮；立即停止声音可使用公共输出页的停止按钮。重启不会恢复自动播出。</p></section>;
}


import {readFile,access} from 'node:fs/promises';
import YAML from 'yaml';
export const BASELINE='docs/digital-host-agent-design-V4.8.md';
export const ID_RE=/^ACC-[A-Z]+-[0-9]{2}$/;
export const STATUSES=['pending','implemented','tested','blocked','not_applicable','not_selected','simulated'];
export async function readRegistry(){return YAML.parse(await readFile('tests/registry.yaml','utf8'));}
export function baselineRequirements(text){
 const s=text.split('## 39. 验收矩阵与交付证据')[1]?.split('## 40.')[0];if(!s)throw Error('acceptance section missing');
 return [...s.matchAll(/^\| (ACC-[A-Z]+-\d+) \| (.*?) \| (.*?) \|$/gm)].map(m=>({id:m[1],title:m[2],expected:m[3]}));
}
export function validateRegistry(r,text){
 const errors=[],required=baselineRequirements(text),seen=new Set();
 if(r.schema_version!==2||r.baseline!==BASELINE)errors.push('registry baseline/version mismatch');
 const tasks=new Set(Array.from({length:15},(_,i)=>`V48-${String(i+1).padStart(2,'0')}`));
 if(JSON.stringify([...(r.work_items??[])].sort())!==JSON.stringify([...tasks].sort()))errors.push('V48-01..15 incomplete');
 const invs=new Set(Array.from({length:62},(_,i)=>`INV-${String(i+1).padStart(2,'0')}`));
 const invRows=r.invariants??[];
 if(invRows.length!==62||new Set(invRows.map(i=>i.id)).size!==62||invRows.some(i=>!invs.has(i.id)))errors.push('INV-01..62 incomplete');
 for(const m of text.matchAll(/^\| (INV-\d{2}) \| (.*?) \|$/gm))if(!invRows.some(i=>i.id===m[1]&&i.requirement===m[2]))errors.push(`${m[1]} text drift`);
 for(const a of r.acceptance??[]){
  if(!ID_RE.test(a.id)||seen.has(a.id))errors.push(`invalid/duplicate ${a.id}`);seen.add(a.id);
  const s=required.find(v=>v.id===a.id);if(!s||s.title!==a.title||s.expected!==a.expected)errors.push(`${a.id} requirement drift`);
  if(!a.work_items?.length||a.work_items.some(t=>!tasks.has(t)))errors.push(`${a.id} unknown work item`);
  if(!a.invariants?.length||a.invariants.some(i=>!invs.has(i)))errors.push(`${a.id} unknown invariant`);
  if(!STATUSES.includes(a.status))errors.push(`${a.id} invalid status`);
  const es=a.evidence??[];
  if(['not_applicable','not_selected'].includes(a.status)&&(!a.condition||a.condition.result!=='false'||a.condition.claimed!==false||!a.condition.reason||!a.condition.expression))errors.push(`${a.id} exclusion requires false unclaimed condition`);
  if(a.status==='simulated'&&!es.some(e=>e.result==='simulated'))errors.push(`${a.id} simulated requires simulated evidence`);
  if(es.some(e=>e.kind==='prerequisite'&&e.complete))errors.push(`${a.id} prerequisite cannot be complete acceptance`);
  if(a.status==='tested'&&!es.some(e=>e.kind==='acceptance'&&e.complete&&e.result==='passed'&&typeof e.environment==='string'&&!/simulat|mock|fixture/i.test(e.environment)))errors.push(`${a.id} tested requires complete real acceptance evidence`);
  if(a.status!=='pending'&&!es.length)errors.push(`${a.id} status requires evidence/condition`);
  for(const e of es)if(!e.report||e.report.startsWith('/')||e.report.includes('..')||/[:\\]/.test(e.report))errors.push(`${a.id} unsafe evidence path`);
 }
 for(const a of required)if(!seen.has(a.id))errors.push(`${a.id} missing`);
 return errors;
}
export async function inspectRegistry(r=undefined){r??=await readRegistry();const errors=validateRegistry(r,await readFile(BASELINE,'utf8'));for(const a of r.acceptance??[])for(const e of a.evidence??[]){try{await access(e.report);}catch{errors.push(`${a.id} missing evidence ${e.report}`);}}return errors;}

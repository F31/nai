import type {preparationConfig} from '@nai/contracts';
import {key} from './api.ts';
export type Product=preparationConfig.Product;
export type Show=preparationConfig.Show;
export type Host=preparationConfig.Host;
export type Series=preparationConfig.Series;
export interface FactForm{id:string;label:string;value:string;type:'text'|'decimal'|'money'|'boolean';unit:string;source:string;conditions:string;confirmed:boolean;from:string;until:string;}
export interface ProductForm{name:string;facts:FactForm[];faq:Product['faq'];talk_tracks:Product['talk_tracks'];prohibitions:string;}
export const emptyFact=():FactForm=>({id:key('fact'),label:'',value:'',type:'text',unit:'',source:'',conditions:'',confirmed:false,from:'',until:''});
export function updateFact(fact:FactForm,patch:Partial<FactForm>):FactForm{return{...fact,...patch,...(Object.keys(patch).some(k=>k!=='confirmed')?{confirmed:false}:{})};}
export const emptyProduct=():ProductForm=>({name:'',facts:[emptyFact()],faq:[],talk_tracks:[{id:key('track'),text:'',fact_ids:[]}],prohibitions:''});
export function yuanToMinor(input:string){const m=/^(0|[1-9]\d{0,9})(?:\.(\d{1,2}))?$/.exec(input.trim());if(!m)throw Error('金额请填写非负元金额，最多两位小数。');return(BigInt(m[1])*100n+BigInt((m[2]??'').padEnd(2,'0'))).toString();}
export function minorToYuan(value:string){const n=BigInt(value);return(n/100n).toString()+'.'+(n%100n).toString().padStart(2,'0');}
const local=(n:number|null)=>n===null?'':new Date(n-new Date(n).getTimezoneOffset()*60000).toISOString().slice(0,23);
export function fromProduct(p:Product):ProductForm{return{name:p.name,facts:p.facts.map(f=>({id:f.id,label:f.label,value:f.value_type==='money'?minorToYuan(String(f.value)):String(f.value),type:f.value_type,unit:f.unit??'',source:f.source,conditions:f.conditions,confirmed:f.authority==='owner_confirmed',from:local(f.valid_from_ms),until:local(f.valid_until_ms)})),faq:p.faq,talk_tracks:p.talk_tracks,prohibitions:p.prohibitions.join('\n')};}
export function toProduct(p:ProductForm):Product{
 if(!p.name.trim())throw Error('请填写产品名称。');
 const facts=p.facts.filter(f=>f.label||f.value||f.source).map(f=>{if(!f.label.trim()||!f.value.trim()||!f.source.trim())throw Error('每条事实都需要名称、内容和来源。');if(f.type==='decimal'&&(!/^-?(0|[1-9]\d{0,11})(\.\d{1,6})?$/.test(f.value)||!f.unit.trim()))throw Error('数值事实需要十进制数字和单位。');const from=f.from?new Date(f.from).getTime():null,until=f.until?new Date(f.until).getTime():null;if(from!==null&&!Number.isFinite(from)||until!==null&&!Number.isFinite(until)||from!==null&&until!==null&&from>=until)throw Error('有效期起止不正确。');return{id:f.id,label:f.label.trim(),value_type:f.type,value:f.type==='boolean'?f.value==='true':f.type==='money'?yuanToMinor(f.value):f.value.trim(),...(f.type==='money'?{currency:'CNY' as const}:{}),...(f.unit?{unit:f.unit}:{}),source:f.source.trim(),conditions:f.conditions,authority:f.confirmed?'owner_confirmed' as const:'unverified' as const,valid_from_ms:from,valid_until_ms:until};});
 const ids=new Set(facts.map(f=>f.id));const binding=<T extends {fact_ids:string[];text:string}>(rows:T[])=>rows.filter(r=>r.text.trim()).map(r=>({...r,fact_ids:r.fact_ids.filter(id=>ids.has(id))}));
 const tracks=binding(p.talk_tracks);
 const faq=binding(p.faq);if(faq.some(f=>!f.question.trim()))throw Error('常见问题需要填写问题和回答。');
 return{schema_version:1,kind:'product_draft',name:p.name.trim(),facts,faq,talk_tracks:tracks,prohibitions:p.prohibitions.split('\n').map(s=>s.trim()).filter(Boolean)};
}

import test from 'node:test';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
const app=createRequire(new URL('../../apps/workbench-desktop/package.json',import.meta.url));
const vite=createRequire(new URL('../../apps/workbench-desktop/node_modules/vite/package.json',import.meta.url));
const renderer=createRequire(app.resolve('react-dom/server')),React=renderer('react'),{renderToStaticMarkup}=app('react-dom/server');
const build=vite('esbuild').buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/live-auto-panel.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','@nai/contracts']});
const m={exports:{}};new Function('require','module','exports',build.outputFiles[0].text)(n=>n==='react'||n==='react/jsx-runtime'?renderer(n):app(n),m,m.exports);
const local={id:'local',name:'本地Qwen',voice:'serena',protocol:'qwen_local',enabled:true,credentialSaved:false};
function html(patch={}){const v={clock:1000,busy:false,liveRun:{context:{runId:'run',epoch:'1',sentenceMode:true,state:1},speechPlan:{planSha256:'a'.repeat(64),batchApproval:{approved:true,expiresAtMs:'300000'}}},liveAuto:{active:false,status:null},llm:{ttsModels:[local]},...patch};return renderToStaticMarkup(React.createElement(m.exports.LiveAutoPanel,{v}));}
test('automatic playback requires explicit rights even with approved text and a ready model',()=>{const h=html();assert.ok(h.includes('开始自动讲品'));assert.match(h,/<button[^>]*disabled=""[^>]*>开始自动讲品/);assert.ok(!h.includes('checked=""'));assert.ok(h.includes('仅本机公共窗口，不推流'));assert.ok(!h.includes('<select'));assert.ok(!h.includes('baseURL'));});
test('missing and expired prerequisites do not expose an actionable automatic start',()=>{assert.ok(html({llm:{ttsModels:[]}}).includes('请先在设置添加可用语音模型'));const h=html({clock:400000});assert.match(h,/<input type="checkbox" disabled=""/);assert.ok(h.includes('核对并批准全部文本'));});
test('active automation exposes stop and distinguishes limit occupation from an actual invoice',()=>{const h=html({liveAuto:{active:true,status:{state:'paused',reason:'',grantId:'grant',expiresAtMs:'300000',reservedMinor:'20',maxCostMinor:'100'}}});assert.ok(h.includes('停止本次自动讲品'));assert.ok(h.includes('已暂停后续消费'));assert.ok(h.includes('不是供应商实际账单'));assert.ok(!h.includes('开始自动讲品'));});
test('a pending boundary pause is presented separately from a completed pause',()=>{const h=html({liveAuto:{active:true,status:{state:'pausing',reason:'',grantId:'grant',expiresAtMs:'300000',reservedMinor:'20',maxCostMinor:'100'}}});assert.ok(h.includes('本句结束后暂停，当前句仍在提交'));assert.ok(!h.includes('已暂停后续消费'));assert.ok(h.includes('停止本次自动讲品'));});

import test from 'node:test';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
const app=createRequire(new URL('../../apps/workbench-desktop/package.json',import.meta.url));
const vite=createRequire(new URL('../../apps/workbench-desktop/node_modules/vite/package.json',import.meta.url));
const renderer=createRequire(app.resolve('react-dom/server')),React=renderer('react'),{renderToStaticMarkup}=app('react-dom/server');
const build=vite('esbuild').buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/human-mic-panel.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','./use-human-mic']});
let mic={status:null};
const m={exports:{}};
new Function('require','module','exports',build.outputFiles[0].text)(n=>n==='./use-human-mic'?{useHumanMic:()=>mic}:n==='react'||n==='react/jsx-runtime'?renderer(n):app(n),m,m.exports);
function html(context={},status=null){mic={status};return renderToStaticMarkup(React.createElement(m.exports.HumanMicPanel,{v:{busy:false,liveRun:{context:{sentenceMode:true,state:1,sentenceCursor:2,...context}}}}));}
test('microphone remains off until an explicit safe route is chosen',()=>{
 const h=html({}, {active:false,reason:'OFF',inputs:[{deviceId:1,name:'Mic',isDefault:true}],outputs:[{deviceId:1,name:'Headphones'}],capturedFrames:'0',submittedFrames:'0',droppedFrames:'0',underrunFrames:'0'});
 assert.ok(h.includes('请选择安全输出'));assert.match(h,/<button[^>]*disabled=""[^>]*>真人接管/);
 assert.ok(!h.includes('checked=""'));assert.ok(h.includes('默认不监听、不录制、不上传'));assert.ok(h.includes('不代表OBS或硬件播放完成'));
});
test('partial sentence resume requires explicit confirmation and skips only that sentence',()=>{
 const h=html({humanTakeover:true,partialSentence:true});
 assert.ok(h.includes('仅跳过这一句'));assert.ok(h.includes('原模型及剩余额度'));assert.match(h,/<button[^>]*disabled=""[^>]*>结束接管并恢复讲品/);
 assert.ok(!h.includes('checked=""'));
});
test('active human route exposes independent mute and all mute controls',()=>{
 const h=html({humanTakeover:true},{active:true,reason:'ACTIVE',inputs:[],outputs:[]});
 assert.ok(h.includes('真人音轨静音'));assert.ok(h.includes('工作台全部静音'));assert.ok(h.includes('数字人静音不停止已开启的人声'));assert.ok(!h.includes('请选择安全输出'));
});
test('ended and non-sentence sessions do not expose human capture controls',()=>{
 assert.equal(html({state:7}),'');assert.equal(html({sentenceMode:false}),'');
});

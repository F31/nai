import test from 'node:test';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
const appRequire=createRequire(new URL('../../apps/workbench-desktop/package.json',import.meta.url));
const viteRequire=createRequire(new URL('../../apps/workbench-desktop/node_modules/vite/package.json',import.meta.url));
const {buildSync}=viteRequire('esbuild');
// Match Vite's React dedupe: this checkout contains separate physical copies in
// pnpm's renderer directory, so Node's ordinary require would split dispatchers.
const rendererRequire=createRequire(appRequire.resolve('react-dom/server'));
const React=rendererRequire('react'),{renderToStaticMarkup}=appRequire('react-dom/server');
const sharedRequire=name=>name==='react'||name==='react/jsx-runtime'?rendererRequire(name):appRequire(name);
const built=buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/published-speech.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','@nai/contracts']});
const m={exports:{}};new Function('require','module','exports',built.outputFiles[0].text)(sharedRequire,m,m.exports);
const {PublishedSpeech}=m.exports;
const audioBundle=buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/audition-controls.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','@nai/contracts']});
const audioModule={exports:{}};new Function('require','module','exports',audioBundle.outputFiles[0].text)(sharedRequire,audioModule,audioModule.exports);
const id='00000000-0000-4000-8000-000000000001';
const model={id,revision:'1',name:'默认语音',voice:'test:voice',enabled:true,credentialSaved:true,priceKnown:true};
function render(models){let calls=0;const v={ws:id,api:{pending:false,call(){calls++;throw Error('render must never dispatch');}},card:{result:{resourceId:id,sha256:'a'.repeat(64)},value:{text:'已发布话术'}},llm:{ttsModels:models},busy:false};const html=renderToStaticMarkup(React.createElement(PublishedSpeech,{v}));assert.equal(calls,0);return html;}
test('speech panel defaults to the sole configured model and never synthesizes while rendering',()=>{
 const html=render([model]);assert.match(html,/语音模型：默认语音/);assert.doesNotMatch(html,/<select/);assert.match(html,/<button disabled="">批准并合成本句/);assert.doesNotMatch(html,/播放本句/);
});
test('unavailable models direct the operator to settings and multiple usable models are selectable',()=>{
 const unavailable=render([{...model,credentialSaved:false}]);assert.match(unavailable,/设置语音模型/);assert.doesNotMatch(unavailable,/批准并合成本句/);
 const multiple=render([model,{...model,id:'00000000-0000-4000-8000-000000000002',name:'备用语音'}]);assert.match(multiple,/<select/);assert.match(multiple,/备用语音/);
});
test('audio controls keep the default one-click path and offer explicit output choice without auto-dispatch',()=>{
 let calls=0;const v={ws:id,api:{pending:false,call(){calls++;throw Error('unexpected call');}}};
 const html=renderToStaticMarkup(React.createElement(audioModule.exports.AuditionControls,{v,task:{resourceId:id,sha256:'a'.repeat(64)}}));
 assert.equal(calls,0);assert.match(html,/试听（本机）/);assert.match(html,/选择输出设备…/);assert.match(html,/<button class="danger" disabled="">停止试听/);assert.doesNotMatch(html,/使用此设备并播放/);
});


test('unknown-price speech remains selectable without dispatch or a hidden price requirement',()=>{
 const html=render([{...model,priceKnown:false}]);assert.match(html,/语音模型：默认语音/);assert.doesNotMatch(html,/设置语音模型|确认单价|填写.*单价/);assert.match(html,/<button disabled="">批准并合成本句/);
});

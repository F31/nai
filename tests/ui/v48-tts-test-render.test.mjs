import test from 'node:test';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
const appRequire=createRequire(new URL('../../apps/workbench-desktop/package.json',import.meta.url));
const viteRequire=createRequire(new URL('../../apps/workbench-desktop/node_modules/vite/package.json',import.meta.url));
const rendererRequire=createRequire(appRequire.resolve('react-dom/server'));
const React=rendererRequire('react'),{renderToStaticMarkup}=appRequire('react-dom/server');
const built=viteRequire('esbuild').buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/tts-test.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','@nai/contracts']});
const m={exports:{}};new Function('require','module','exports',built.outputFiles[0].text)(name=>name==='react'||name==='react/jsx-runtime'?rendererRequire(name):appRequire(name),m,m.exports);
test('the real test panel offers one-click synthesis without money or confirmation and never dispatches on render',()=>{
 let calls=0;const api={pending:false,call(){calls++;throw Error('unexpected synthesis');}};
 const v={ws:'00000000-0000-4000-8000-000000000001',busy:false,api};
 const html=renderToStaticMarkup(React.createElement(m.exports.TtsTest,{v,model:{id:'model',name:'语音模型',revision:'1',priceKnown:false}}));
 assert.equal(calls,0);assert.match(html,/<button>测试合成/);assert.doesNotMatch(html,/预留|费用确认|type="checkbox"/);assert.match(html,/核对任务状态/);assert.match(html,/不自动播放/);assert.doesNotMatch(html,/>试听（本机）<\/button>/);
});
test('the real text model test panel has no cap or confirmation control',()=>{
 const built=viteRequire('esbuild').buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/llm-panel.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','@nai/contracts']});
 const m={exports:{}};new Function('require','module','exports',built.outputFiles[0].text)(name=>name==='react'||name==='react/jsx-runtime'?rendererRequire(name):appRequire(name),m,m.exports);
 const html=renderToStaticMarkup(React.createElement(m.exports.ModelTest,{v:{busy:false,llm:{probe:{saved:true,running:false,result:null}}}}));
 assert.doesNotMatch(html,/预留|确认|<input/);assert.match(html,/<button type="button">测试连接与响应/);
 const result={taskState:5,taskReason:'API_RESPONSE_OK',body:JSON.stringify({kind:'publication_llm_test',target:'private_workbench',text:'你好，文本模型响应正常。'})};
 const success=renderToStaticMarkup(React.createElement(m.exports.ModelTest,{v:{busy:false,llm:{probe:{saved:true,running:false,result}}}}));
 assert.match(success,/模型响应内容/);assert.match(success,/你好，文本模型响应正常。/);
});

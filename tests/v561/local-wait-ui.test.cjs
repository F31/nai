const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),Module=require('node:module'),path=require('node:path'),ts=require('typescript');
test('prepared loop rejects duplicate and post-stop receipts',()=>{
 const oldWindow=global.window, oldTs=Module._extensions['.ts'];
 global.window={};
 Module._extensions['.ts']=(m,f)=>m._compile(ts.transpileModule(fs.readFileSync(f,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,f);
 try {
  const {repeatDecision}=load();
  const completed={kind:7,code:'COMPLETED',requestSequence:'12',contentPrepared:true,visualPrepared:true,ttsPrepared:true,busy:false};
  assert.equal(repeatDecision(11n,completed),'play');
  assert.equal(repeatDecision(12n,completed),'wait');
  assert.equal(repeatDecision(null,completed),'wait');
  assert.equal(repeatDecision(11n,{...completed,code:'REPLY_COMPLETED'}),'play');
  assert.equal(repeatDecision(12n,{...completed,kind:6}),'stop');
  assert.equal(repeatDecision(11n,{...completed,contentPrepared:false}),'stop');
 } finally {global.window=oldWindow;if(oldTs)Module._extensions['.ts']=oldTs;else delete Module._extensions['.ts'];}
});

test('Stop invalidates a pending native completion poll without issuing another play',async()=>{
 const oldWindow=global.window, oldTs=Module._extensions['.ts'];
 const calls=[];
 let release, refresh, stateIndex=0;
 global.window={__TAURI__:{core:{invoke(name){
  calls.push(name);
  if(name==='v561_live_status') return new Promise(resolve=>{release=resolve;});
  return Promise.resolve();
 }}}};
 const oldInterval=global.setInterval;
 global.setInterval=fn=>{refresh=fn;return 1;};
 Module._extensions['.ts']=(m,f)=>m._compile(ts.transpileModule(fs.readFileSync(f,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,f);
 try {
  const text='你好，这是数字人直播。';
  const api=load({useRef:value=>({current:value}),useEffect:fn=>fn(),useState(value){
   const index=stateIndex++;
   if(index===0)value={...value,kind:10,requestSequence:'2',visualPrepared:true,ttsPrepared:true,contentPrepared:true};
   if(index===5)value=text;
   if(index===7)value=true;
   return [value,()=>{}];
  }});
  const nodes=[];
  function walk(node){if(Array.isArray(node))return node.forEach(walk);if(node?.props){nodes.push(node);walk(node.props.children);}}
  walk(api.Voice());
  const {Event}=require(path.resolve('packages/contracts-ts/src/generated/v561/live_session.ts'));
  const encode=fields=>Event.encode(Event.create({version:1,visualPrepared:true,ttsPrepared:true,contentPrepared:true,...fields})).finish();
  release(encode({kind:10,requestSequence:'2'}));
  await new Promise(resolve=>setImmediate(resolve));
  nodes.find(n=>n.type==='button'&&n.props.children==='播放已备内容').props.onClick();
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(calls.filter(name=>name==='v561_live_status').length,2);
  nodes.find(n=>n.type==='button'&&n.props.children==='停止').props.onClick();
  release(encode({kind:7,code:'COMPLETED',requestSequence:'3'}));
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(typeof refresh,'function');
  assert.equal(calls.filter(name=>name==='v561_live_play_prepared').length,1);
  assert.ok(calls.includes('v561_live_stop'));
 } finally {global.window=oldWindow;global.setInterval=oldInterval;if(oldTs)Module._extensions['.ts']=oldTs;else delete Module._extensions['.ts'];}
});
function load(hooks={}) {
 const file=path.resolve('apps/workbench-desktop/ui/v561-voice.tsx'),m=new Module(file);
 m.filename=file;m.paths=Module._nodeModulePaths(path.dirname(file));
 const real=m.require.bind(m);m.require=id=>id==='react'?{...real(id),...hooks}:real(id);
 const source=fs.readFileSync(file,'utf8').replace("import './v561-voice.css';",'').replace(/createRoot\(document\.getElementById\('root'\)!\)\.render\(<Voice\s*\/>\);/,'');
 m._compile(ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.ReactJSX,target:ts.ScriptTarget.ES2022}}).outputText,file);
 return m.exports;
}
test('new local generation shows elapsed and Stop; cached playback never claims generation wait',()=>{
 const previous=global.window,oldTs=Module._extensions['.ts'];
 global.window={__TAURI__:{core:{invoke(){throw Error('render must not call models');}}}};
 Module._extensions['.ts']=(m,f)=>m._compile(ts.transpileModule(fs.readFileSync(f,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,f);
 try {
  for(const code of ['PLAYING','PLAYING_PREPARED','GENERATING_ANSWER']) {
   let hook=0;const api=load({useEffect(){},useState(initial){return [hook++===0?{...initial,kind:4,busy:true,code,requestSequence:3n,visualPrepared:true,ttsPrepared:true}:initial,()=>{}];}});
   const html=require('react-dom/server').renderToStaticMarkup(require('react').createElement(api.Voice));
   assert.equal(html.includes('本次已用'),code!=='PLAYING_PREPARED');
   const stop=html.match(/<button[^>]*class="stop"[^>]*>停止<\/button>/)[0];assert.ok(!stop.includes('disabled'));
   if(code==='PLAYING'){assert.ok(html.includes('首次声音需要等待数秒'));assert.ok(!html.includes('正在逐句讲解'));}
   else if(code==='PLAYING_PREPARED') assert.ok(html.includes('本次不会调用模型')); else assert.ok(html.includes('自动回到被打断的句子'));
  }
 } finally {global.window=previous;if(oldTs)Module._extensions['.ts']=oldTs;else delete Module._extensions['.ts'];}
});
test('elapsed uses monotonic difference and timer is removed when operation ends',()=>{
 const previous=global.window,oldSet=global.setInterval,oldClear=global.clearInterval;
 global.window={};let tick,cleanup,seconds=-1,clock=10000,cleared;
 global.setInterval=(fn)=>{tick=fn;return 77;};global.clearInterval=id=>cleared=id;
 try {
  const api=load({useRef(v){return {current:v};},useState(v){return [v,x=>seconds=x];},useEffect(fn){cleanup=fn();}});
  const element=api.LocalElapsed({now:()=>clock});assert.equal(element.props['aria-live'],'off');
  clock=23999;tick();assert.equal(seconds,13);cleanup();assert.equal(cleared,77);
  assert.equal(api.elapsedSeconds(10000,9999),0);assert.equal(api.elapsedSeconds(NaN,100),0);
  for(const state of [{kind:4,busy:false,code:'PLAYING'},{kind:6,busy:false,code:'STOPPED'},{kind:8,busy:false,code:'PLAY_FAILED'}])
   assert.equal(api.localGenerationActive(state),false);
 }finally{global.window=previous;global.setInterval=oldSet;global.clearInterval=oldClear;}
});

test('AI answer is enabled only during cached narration and never auto invoked',()=>{
 const previous=global.window,oldTs=Module._extensions['.ts'];
 global.window={__TAURI__:{core:{invoke(){throw Error('no automatic model call');}}}};
 Module._extensions['.ts']=(m,f)=>m._compile(ts.transpileModule(fs.readFileSync(f,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,f);
 try {
  for(const code of ['PLAYING_PREPARED','GENERATING_ANSWER','PLAYING','STOPPED']) {
   let hook=0; const api=load({useEffect(){},useState(initial){const i=hook++;return [i===0?{...initial,kind:code==='STOPPED'?6:4,busy:code!=='STOPPED',code,visualPrepared:true,ttsPrepared:true}:i===6?'介绍什么？':initial,()=>{}];}});
   const html=require('react-dom/server').renderToStaticMarkup(require('react').createElement(api.Voice));
   const button=html.match(/<button[^>]*>AI回答<\/button>/)[0];
   assert.equal(button.includes('disabled'),code!=='PLAYING_PREPARED');
  }
 } finally {global.window=previous;if(oldTs)Module._extensions['.ts']=oldTs;else delete Module._extensions['.ts'];}
});


const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),Module=require('node:module'),path=require('node:path'),ts=require('typescript');
test('AV workbench renders explicit actions and browser preview never invokes models',()=>{
 const previous=global.window,oldTs=Module._extensions['.ts'];
 global.window={};
 Module._extensions['.ts']=(module,file)=>module._compile(ts.transpileModule(fs.readFileSync(file,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,file);
 try{
  const file=path.resolve('apps/workbench-desktop/ui/v561-voice.tsx');
  const source=fs.readFileSync(file,'utf8').replace("import './v561-voice.css';",'').replace(/createRoot\(document\.getElementById\('root'\)!\)\.render\(<Voice\s*\/>\);/,'');
  const compiled=new Module(file);compiled.filename=file;compiled.paths=Module._nodeModulePaths(path.dirname(file));
  compiled._compile(ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.ReactJSX,target:ts.ScriptTarget.ES2022}}).outputText,file);
  const html=require('react-dom/server').renderToStaticMarkup(require('react').createElement(compiled.exports.Voice));
  for(const label of ['准备数字人','播放已备内容','生成并播放','停止','设置','网页预览不会调用模型'])assert.ok(html.includes(label),label);
  assert.match(html, /<button class="primary" disabled="">播放已备内容<\/button>/);
  assert.ok(html.includes('准备讲解内容'));assert.match(compiled.exports.message('CONTENT_NOT_PREPARED'),/未自动生成/);
  assert.ok(!html.includes('优先插播'));assert.ok(!html.includes('已就绪，点击试听'));
  assert.match(compiled.exports.message('CUDA_PREPARATION_FAILED'),/CUDA 模型预热失败/);
  assert.match(compiled.exports.message('HOST_MEMORY_EXHAUSTED'),/页面文件空间不足/);
  assert.match(compiled.exports.message('PREPARATION_TIMEOUT'),/准备超时/);
  assert.match(compiled.exports.message('PREPARATION_CANCELLED'),/已停止/);
  assert.match(compiled.exports.message('OUTPUT_WINDOW_UNAVAILABLE'),/公共输出窗口.*已停止.*恢复窗口.*不会自动恢复/);
  assert.match(compiled.exports.message('TTS_PREPARATION_FAILED'),/语音预热失败/);
  for(const visualPrepared of [false,true])for(const ttsPrepared of [false,true])
   assert.equal(compiled.exports.prepared({visualPrepared,ttsPrepared}),visualPrepared&&ttsPrepared);
  assert.ok(html.includes('语音：'));assert.ok(html.includes('画面：'));
  assert.ok(!compiled.exports.message('private exception /secret').includes('/secret'));
 }finally{global.window=previous;if(oldTs)Module._extensions['.ts']=oldTs;else delete Module._extensions['.ts'];}
});

const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),Module=require('node:module'),path=require('node:path'),ts=require('typescript');
test('native prepared standby exposes Stop while unprepared and browser previews do not',()=>{
 const before=global.window,oldTs=Module._extensions['.ts'];
 Module._extensions['.ts']=(m,f)=>m._compile(ts.transpileModule(fs.readFileSync(f,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,f);
 try {
  for(const [native,prepared,enabled] of [[true,true,true],[true,false,false],[false,true,false]]) {
   global.window=native?{__TAURI__:{core:{invoke(){throw Error('render must not call models');}}}}:{};
   const file=path.resolve('apps/workbench-desktop/ui/v561-voice.tsx'),m=new Module(file);m.filename=file;m.paths=Module._nodeModulePaths(path.dirname(file));
   const real=m.require.bind(m),react=require('react');let hook=0;
   m.require=id=>id==='react'?{...react,useEffect(){},useRef(v){return {current:v};},useState(initial){return [hook++===0?{...initial,contentPrepared:prepared,visualPrepared:prepared,ttsPrepared:prepared,busy:false}:initial,()=>{}];}}:real(id);
   const source=fs.readFileSync(file,'utf8').replace("import './v561-voice.css';",'').replace(/createRoot\(document\.getElementById\('root'\)!\)\.render\(<Voice\s*\/>\);/,'');
   m._compile(ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.ReactJSX,target:ts.ScriptTarget.ES2022}}).outputText,file);
   const html=require('react-dom/server').renderToStaticMarkup(react.createElement(m.exports.Voice));
   const stop=html.match(/<button[^>]*class="stop"[^>]*>停止<\/button>/)?.[0];assert.ok(stop);
   assert.equal(!stop.includes('disabled'),enabled);
   assert.ok(html.includes('公共输出窗口展开'));
  }
 }finally{global.window=before;if(oldTs)Module._extensions['.ts']=oldTs;else delete Module._extensions['.ts'];}
});

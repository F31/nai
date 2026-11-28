const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),Module=require('node:module'),path=require('node:path'),cp=require('node:child_process'),ts=require('typescript');
test('question task keeps uint64 identity and exact resume cursor across TS/Python codecs',()=>{
 const old=Module._extensions['.ts'];
 Module._extensions['.ts']=(m,f)=>m._compile(ts.transpileModule(fs.readFileSync(f,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,f);
 try {
  const {QuestionTask,QuestionResult}=require(path.resolve('packages/contracts-ts/src/generated/v561/live_session.ts'));
  const task=QuestionTask.create({taskId:'18446744073709551614',generation:'4',question:'支持哪些功能？',budgetMs:45000,
    resume:{planSha256:Uint8Array.from({length:32},()=>7),contentGeneration:'2',sentenceIndex:1,sentenceCount:3}});
  const bytes=QuestionTask.encode(task).finish();assert.deepEqual(QuestionTask.decode(bytes),task);
  const answer=QuestionResult.create({taskId:task.taskId,generation:'4',answer:'支持本机语音讲解。'});
  assert.deepEqual(QuestionResult.decode(QuestionResult.encode(answer).finish()),answer);
  const python=process.env.NAI_CONTRACT_PYTHON;
  assert.ok(python,'explicit existing Python required for cross-language verification');
  const script="import sys\nfrom v561.live_session_pb2 import QuestionTask,QuestionResult\nx=QuestionTask.FromString(sys.stdin.buffer.read())\nassert x.task_id==18446744073709551614 and x.resume.sentence_index==1 and x.resume.content_generation==2 and x.budget_ms==45000\ny=QuestionResult(task_id=x.task_id,generation=x.generation,answer='支持本机语音讲解。')\nsys.stdout.buffer.write(y.SerializeToString())\n";
  const result=cp.spawnSync(python,['-B','-c',script],{input:bytes,timeout:10000,env:{...process.env,PYTHONPATH:path.resolve('packages/contracts-python/src/generated'),PYTHONDONTWRITEBYTECODE:'1'}});
  assert.equal(result.status,0,result.stderr?.toString()||result.error?.message);
  assert.deepEqual(QuestionResult.decode(result.stdout),answer);
 }finally{if(old)Module._extensions['.ts']=old;else delete Module._extensions['.ts'];}
});

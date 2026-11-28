const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const Ajv=require('ajv/dist/2020.js').default;
test('live control TS/Python codecs preserve uint64 and completion fields',{skip:!process.env.NAI_CONTRACT_PYTHON},()=>{
 const ts=require('typescript'),Module=require('node:module'),path=require('node:path'),{spawnSync}=require('node:child_process');
 const file=path.resolve('packages/contracts-ts/src/generated/v561/live_session.ts');
 const compiled=new Module(file);compiled.filename=file;compiled.paths=Module._nodeModulePaths(path.dirname(file));
 compiled._compile(ts.transpileModule(fs.readFileSync(file,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,file);
 const {Request,Event,Command,Kind}=compiled.exports;
 for(const command of [Command.COMMAND_START,Command.COMMAND_PREPARE_CONTENT,Command.COMMAND_PLAY_PREPARED,Command.COMMAND_ASK]){
 const bytes=Request.encode(Request.create({version:1,sequence:'9007199254740993',command,text:'你好。'})).finish();
 const result=spawnSync(process.env.NAI_CONTRACT_PYTHON,['-B','tests/v561/live_session_roundtrip.py'],{input:bytes,env:{...process.env,PYTHONPATH:path.resolve('packages/contracts-python/src/generated')},timeout:5000});
 assert.equal(result.status,0,result.stderr.toString());
 const event=Event.decode(result.stdout);assert.equal(event.sequence,'9007199254740993');assert.equal(event.kind,command===Command.COMMAND_PREPARE_CONTENT?Kind.KIND_CONTENT_PREPARED:Kind.KIND_COMPLETED);assert.equal(event.samples,event.playedSamples);assert.equal(event.visualPrepared,true);assert.equal(event.ttsPrepared,true);assert.equal(event.contentPrepared,true);
 }
});
test('session assembly rejects extra fields and invalid buffer limits',()=>{
 const validate=new Ajv({strict:true}).compile(JSON.parse(fs.readFileSync('schemas/config/live-session.schema.json','utf8')));
 const config={avatar_config:'E:/data/avatar.json',tts_config:'E:/data/tts.json',worker:'E:/runtime/worker.py',prefill_samples:48000,tts_warmup_text:'语音准备测试。',prepared_cache:{max_bytes:268435456,ttl_ms:1800000,max_entries:4,max_total_bytes:1073741824,prefetch_max_bytes:268435456}};
 assert.equal(validate(config),true);
 for(const change of [{prefill_samples:0},{prefill_samples:96001},{apiKey:'unsupported'},{worker:''},{prepared_cache:{max_bytes:536870913,ttl_ms:1000}},{prepared_cache:{max_bytes:4194304,ttl_ms:0}},{tts_warmup_text:''},{tts_warmup_text:'  '},{tts_warmup_text:'字'.repeat(65)},{tts_warmup_text:null}])assert.equal(validate({...config,...change}),false);
 for(const prefetch_max_bytes of [0,536870913])assert.equal(validate({...config,prepared_cache:{...config.prepared_cache,prefetch_max_bytes}}),false);
 const llm={base_url:"https://example.invalid/v1",model:"model",api_key_ref:"keyring:nai/llm",deadline_ms:15000,max_answer_chars:32}; assert.equal(validate({...config,llm}),true); for(const change of [{api_key:"secret"},{base_url:"http://example.invalid"},{deadline_ms:15001},{max_answer_chars:0},{max_output_tokens:1025},{max_output_tokens:0}])assert.equal(validate({...config,llm:{...llm,...change}}),false);
 const missing={...config};delete missing.tts_warmup_text;assert.equal(validate(missing),false);
});



const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
test('continuous audio TS/Python identities preserve uint64; seal is not device completion', {skip:!process.env.NAI_CONTRACT_PYTHON},()=>{
 const ts=require('typescript'),Module=require('node:module'),path=require('node:path'),{spawnSync}=require('node:child_process');
 const file=path.resolve('packages/contracts-ts/src/generated/v561/continuous_audio.ts');
 const compiled=new Module(file);compiled.filename=file;compiled.paths=Module._nodeModulePaths(path.dirname(file));
 compiled._compile(ts.transpileModule(fs.readFileSync(file,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,file);
 const {SegmentSeal,SegmentProgress}=compiled.exports;
 const bytes=SegmentSeal.encode(SegmentSeal.create({identity:{scriptEpoch:'9007199254740993',sentenceId:2,generationEpoch:'4',sourceStartSample:'9007199254740992'},samples:'2',sha256:new Uint8Array(32).fill(1)})).finish();
 const result=spawnSync(process.env.NAI_CONTRACT_PYTHON,['-B','tests/v561/continuous_audio_roundtrip.py'],{input:bytes,env:{...process.env,PYTHONPATH:path.resolve('packages/contracts-python/src/generated')},timeout:5000});
 assert.equal(result.status,0,result.stderr.toString());
 const progress=SegmentProgress.decode(result.stdout);
 assert.equal(progress.identity.scriptEpoch,'9007199254740993');
 assert.equal(progress.identity.sourceStartSample,'9007199254740992');
 assert.equal(progress.playedSamples,'1');assert.equal(progress.sealed,true);assert.equal(progress.completed,false);
});

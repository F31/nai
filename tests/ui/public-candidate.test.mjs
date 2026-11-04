import test from 'node:test';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const app=createRequire(new URL('../../apps/workbench-desktop/package.json',import.meta.url));
const vite=createRequire(new URL('../../apps/workbench-desktop/node_modules/vite/package.json',import.meta.url));
const renderer=createRequire(app.resolve('react-dom/server'));
const React=renderer('react'),{renderToStaticMarkup}=app('react-dom/server');
const {buildSync}=vite('esbuild');
const built=buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/public-candidate.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','@nai/contracts']});
const m={exports:{}};new Function('require','module','exports',built.outputFiles[0].text)(n=>n==='react'||n==='react/jsx-runtime'?renderer(n):app(n),m,m.exports);
const {PublicCandidate,PublicCandidateResult,validPublicCandidate,verifyPreparedReceiptHash}=m.exports;
const guardBuilt=buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/public-guard.ts',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,external:['@nai/contracts']});
const guardModule={exports:{}};new Function('require','module','exports',guardBuilt.outputFiles[0].text)(app,guardModule,guardModule.exports);
const {verifyGuardReviewHash,guardPolicy}=guardModule.exports;
const guardTarget='00000000-0000-4000-8000-000000000020';
const {PublicMediaBundle,PreparedMediaDescriptor,PublicGuardObject}=app('@nai/contracts');
const id='00000000-0000-4000-8000-000000000001',hash='a'.repeat(64);
const request={workspaceId:id,previewId:id,previewSha256:hash,taskId:id,taskSha256:hash};
function check(){return {previewId:id,previewSha256:hash,taskId:id,taskSha256:hash,text:'已发布本句',captionSha256:hash,snapshotId:id,snapshotSha256:hash,factsSha256:hash,sessionId:id,epoch:'1',checkedAtMs:'1792800000000',approvalAvailable:false,audio:{artifactId:id,pcmSha256:hash,sampleCount:'24000',sampleRate:24000,channels:1},axes:[{axis:'source',state:1,reason:'SOURCE_AND_SENTENCE_BOUND'},{axis:'audio',state:1,reason:'ACTUAL_PCM_VERIFIED'},{axis:'visual',state:2,reason:'PUBLIC_VISUAL_LICENSE_NOT_CONFIGURED'},{axis:'guard',state:2,reason:'PUBLIC_GUARD_NOT_INSTALLED'},{axis:'output',state:2,reason:'PUBLIC_MEDIA_OUTPUT_NOT_INSTALLED'},{axis:'media',state:2,reason:'MEDIA_ASSET_NOT_PREPARED'}]};}
test('candidate inspection is explicit and rendering never calls APIs or grants approval',()=>{
 let calls=0;const v={busy:false,ws:id,card:{result:{resourceId:id,sha256:hash},value:{text:'已发布本句'}},api:{call(){calls++;throw Error('unexpected');}}};
 const html=renderToStaticMarkup(React.createElement(PublicCandidate,{v,task:{resourceId:id,sha256:hash}}));
 assert.equal(calls,0);assert.match(html,/检查公共播出准备/);assert.doesNotMatch(html,/批准并播出|已核验/);
});
test('real source and PCM observations still display missing public axes without a playback button',()=>{
 const c=check();assert.equal(validPublicCandidate(c,request,c.text),true);
 const html=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:c}));
 assert.match(html,/当前尚不能公共播出/);assert.match(html,/已重新核对实际 PCM/);assert.match(html,/公共形象与素材许可尚未接通/);assert.doesNotMatch(html,/<button/);
});
test('malformed, stale and fabricated readiness responses are rejected',()=>{
 for(const mutate of [c=>{c.approvalAvailable=true;},c=>{c.taskSha256='b'.repeat(64);},c=>{c.text='另一句';},c=>{c.axes[2].state=1;},c=>{c.axes[1].axis='source';},c=>{c.audio=undefined;},c=>{c.audio.sampleCount='480001';},c=>{c.axes[1].reason='UNKNOWN';}]){
  const c=check();mutate(c);assert.equal(validPublicCandidate(c,request,'已发布本句'),false);
 }
 const expired=check();expired.audio=undefined;expired.axes[1]={axis:'audio',state:2,reason:'PCM_EXPIRED_OR_RELEASED'};assert.equal(validPublicCandidate(expired,request,expired.text),true);
});
test('admitted actual visual and license can pass while public guard and output stay unavailable',()=>{
 const c=check();c.axes[2]={axis:'visual',state:1,reason:'BUILTIN_VISUAL_AND_LICENSE_VERIFIED'};
 c.visual={artifactId:'d074f2a8-a618-4b43-9e24-679bf43a0001',packageSha256:hash,licenseSha256:hash,width:512,height:288,fps:30};c.visualName='内置简洁2D形象';c.visualLicense='MIT';
 assert.equal(validPublicCandidate(c,request,c.text),true);
 const html=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:c}));
 assert.match(html,/内置简洁2D形象/);assert.match(html,/MIT 许可/);assert.match(html,/公共播出守门尚未接通/);assert.match(html,/当前尚不能公共播出/);
 for(const mutate of [v=>{v.visual.licenseSha256='';},v=>{v.visual.width=513;},v=>{v.visualLicense='unknown';}]){const bad=structuredClone(c);mutate(bad);assert.equal(validPublicCandidate(bad,request,c.text),false);}
});
function guarded(){const c=check();c.visual={artifactId:'d074f2a8-a618-4b43-9e24-679bf43a0001',packageSha256:hash,licenseSha256:hash,width:512,height:288,fps:30};c.visualName='内置简洁2D形象';c.visualLicense='MIT';c.axes[2]={axis:'visual',state:1,reason:'BUILTIN_VISUAL_AND_LICENSE_VERIFIED'};
 c.outputInstance={protocolMajor:1,seq:"1",challenge:"1",windowOpen:true,standby:true,liveReady:false,audioActive:false,standbyFrames:"1",reason:1,terminal:false,outputId:guardTarget,visual:c.visual,visualName:c.visualName,visualLicense:c.visualLicense};
 const object=PublicGuardObject.fromPartial({protocolMajor:1,policyId:guardPolicy,workspaceId:id,sessionId:c.sessionId,epoch:c.epoch,previewId:c.previewId,previewSha256:c.previewSha256,taskId:c.taskId,taskSha256:c.taskSha256,snapshotId:c.snapshotId,snapshotSha256:c.snapshotSha256,factsSha256:c.factsSha256,captionSha256:c.captionSha256,outputTargetId:guardTarget,audio:c.audio,visual:c.visual});
 const now=Date.now();c.guardReview={object,objectSha256:createHash('sha256').update(PublicGuardObject.encode(object).finish()).digest('hex'),policyPassed:true,reason:1,checkedAtMs:String(now),validUntilMs:String(now+30000)};c.axes[3]={axis:'guard',state:1,reason:'GUARD_EXACT_SPEC_PASSED'};return c;
}
test('bound restricted guard can pass without creating public approval or output readiness',async()=>{
 const c=guarded();assert.equal(validPublicCandidate(c,request,c.text),true);assert.equal(await verifyGuardReviewHash(c),true);
 const html=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:c}));assert.match(html,/内容原文核对/);assert.match(html,/本机公共窗口（已绑定实例）/);assert.match(html,/当前尚不能公共播出/);assert.doesNotMatch(html,/<button/);
 const expired=guarded();expired.guardReview.checkedAtMs=String(Date.now()-60000);expired.guardReview.validUntilMs=String(Date.now()-30000);const old=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:expired}));assert.match(old,/复核已过期，请重新检查/);assert.match(old,/已过期/);
});
test('guard source/resource/target changes, forged hash and false decision flags are rejected',async()=>{
 for(const mutate of [c=>{c.guardReview.object.epoch='2';},c=>{c.outputInstance.outputId=id;},c=>{c.outputInstance.terminal=true;},c=>{c.guardReview.object.audio.sampleCount='12000';},c=>{c.guardReview.object.outputTargetId=id;},c=>{c.guardReview.policyPassed=false;},c=>{c.guardReview.validUntilMs='0';},c=>{c.guardReview.reason=99;},c=>{c.axes[3].state=2;}]){const c=guarded();mutate(c);assert.equal(validPublicCandidate(c,request,c.text),false);}
 const corrupt=guarded();corrupt.guardReview.objectSha256='f'.repeat(64);assert.equal(await verifyGuardReviewHash(corrupt),false);
 const blocked=guarded();blocked.guardReview.reason=2;blocked.guardReview.policyPassed=false;blocked.axes[3]={axis:'guard',state:3,reason:'GUARD_UNSUPPORTED_SPEECH'};assert.equal(validPublicCandidate(blocked,request,blocked.text),true);const html=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:blocked}));assert.match(html,/已阻止/);assert.match(html,/不支持此改写/);
});
test('missing actual output stays unbound and cannot reuse a prior bound review',()=>{
 const c=guarded();c.outputInstance=undefined;assert.equal(validPublicCandidate(c,request,c.text),false);
 c.guardReview={policyPassed:false,reason:7,checkedAtMs:String(Date.now()),validUntilMs:String(Date.now()),objectSha256:''};c.axes[3]={axis:'guard',state:2,reason:'GUARD_OUTPUT_NOT_BOUND'};c.axes[4]={axis:'output',state:2,reason:'PUBLIC_OUTPUT_NOT_BOUND'};
 assert.equal(validPublicCandidate(c,request,c.text),true);const html=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:c}));assert.match(html,/请先在公共输出页打开待机窗口/);assert.doesNotMatch(html,/已绑定实例/);
});

test('prepared metadata binds PCM, source and sample timeline without granting playback',async()=>{
 const c=guarded();c.axes[5]={axis:'media',state:1,reason:'MEDIA_ASSET_PREPARED'};
 c.preparedMedia={protocolMajor:1,workspaceId:id,taskId:id,taskSha256:hash,audio:structuredClone(c.audio),visual:structuredClone(c.visual),expectedFrames:'30',samplesPerFrame:800,firstFrameSha256:hash,pcmFormat:'s16le-mono-24000',pixelFormat:'rgb32le',rendererId:'rms-sample-v1'};
 c.preparedMediaReceipt={protocolMajor:1,descriptorSha256:createHash('sha256').update(PreparedMediaDescriptor.encode(c.preparedMedia).finish()).digest('hex'),consumerInstanceId:id,pcmSha256:hash,sampleCount:'24000',expectedFrames:'30',firstFrameSha256:hash,tailFrame:{frameIndex:'30',sampleStart:'23200',sampleCount:800,sampleRate:24000,width:512,height:288,pixelsSha256:hash,opennessQ16:1}};
 assert.equal(validPublicCandidate(c,request,c.text),true);assert.equal(await verifyPreparedReceiptHash(c),true);
 const malformed=structuredClone(c);malformed.preparedMediaReceipt.tailFrame.sampleStart='invalid';assert.equal(validPublicCandidate(malformed,request,c.text),false);
 const forged=structuredClone(c);forged.preparedMediaReceipt.descriptorSha256='f'.repeat(64);assert.equal(await verifyPreparedReceiptHash(forged),false);
 assert.match(renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:c})),/未批准、未播放/);
 for(const mutate of [p=>p.expectedFrames='31',p=>p.rendererId='fake',p=>p.taskSha256='b'.repeat(64),p=>p.audio.pcmSha256='b'.repeat(64),p=>p.visual.width=1,p=>p.samplesPerFrame=0]){const bad=structuredClone(c);mutate(bad.preparedMedia);assert.equal(validPublicCandidate(bad,request,c.text),false);}
});

const approvalBuild=buildSync({entryPoints:[fileURLToPath(new URL('../../apps/workbench-desktop/ui/public-approval.tsx',import.meta.url))],bundle:true,platform:'node',format:'cjs',write:false,jsx:'automatic',external:['react','react/jsx-runtime','@nai/contracts']});
const approvalModule={exports:{}};new Function('require','module','exports',approvalBuild.outputFiles[0].text)(n=>n==='react'||n==='react/jsx-runtime'?renderer(n):app(n),approvalModule,approvalModule.exports);
const {PublicApprovalControls,validApprovalDraft,verifyApprovalDraftHash}=approvalModule.exports;
function approvalReady(){
 const c=guarded();c.checkedAtMs=String(Date.now());c.axes[5]={axis:'media',state:1,reason:'MEDIA_ASSET_PREPARED'};
 c.preparedMedia={protocolMajor:1,workspaceId:id,taskId:id,taskSha256:hash,audio:structuredClone(c.audio),visual:structuredClone(c.visual),expectedFrames:'30',samplesPerFrame:800,firstFrameSha256:hash,pcmFormat:'s16le-mono-24000',pixelFormat:'rgb32le',rendererId:'rms-sample-v1'};
 c.preparedMediaReceipt={protocolMajor:1,descriptorSha256:createHash('sha256').update(PreparedMediaDescriptor.encode(c.preparedMedia).finish()).digest('hex'),consumerInstanceId:id,pcmSha256:hash,sampleCount:'24000',expectedFrames:'30',firstFrameSha256:hash,tailFrame:{frameIndex:'30',sampleStart:'23200',sampleCount:800,sampleRate:24000,width:512,height:288,pixelsSha256:hash,opennessQ16:1}};
 const b=PublicMediaBundle.fromPartial({protocolMajor:1,bundleId:id,approvalId:id,guardPermitId:id,workspaceId:id,sessionId:c.sessionId,epoch:c.epoch,outputId:guardTarget,snapshotId:c.snapshotId,snapshotSha256:c.snapshotSha256,factsSha256:c.factsSha256,captionSha256:c.captionSha256,audio:c.audio,visual:c.visual,leaseMs:500,scope:2,preparedMediaSha256:c.preparedMediaReceipt.descriptorSha256});
 b.approvedSha256=createHash('sha256').update(PublicMediaBundle.encode(b).finish()).digest('hex');c.approvalAvailable=true;c.approvalDraft={bundle:b,text:c.text,expiresAtMs:String(Number(c.checkedAtMs)+15000)};return c;
}
test('approval requires explicit rights and never presents an executable playback button',async()=>{
 const c=approvalReady();assert.equal(validPublicCandidate(c,request,c.text),true);assert.equal(validApprovalDraft(c,request),true);assert.equal(await verifyApprovalDraftHash(c),true);
 let calls=0;const v={busy:false,api:{call(){calls++;throw Error('automatic call');}}};
 const html=renderToStaticMarkup(React.createElement(PublicApprovalControls,{v,check:c,request}));assert.equal(calls,0);assert.match(html,/确认本句（不播出）/);assert.match(html,/任何直播平台/);assert.match(html,/disabled=""/);assert.doesNotMatch(html,/checked=""|批准并播出/);
});
test('approval draft identity, resource, expiry, source, and hash forgery are rejected',async()=>{
 for(const mutate of [c=>c.approvalDraft.bundle.outputId=id,c=>c.approvalDraft.bundle.epoch='2',c=>c.approvalDraft.bundle.preparedMediaSha256=hash,c=>c.approvalDraft.text='other',c=>c.approvalDraft.expiresAtMs=String(Number(c.checkedAtMs)+30001),c=>c.approvalDraft.bundle.leaseMs=0,c=>c.approvalAvailable=false]){const c=approvalReady();mutate(c);assert.equal(validPublicCandidate(c,request,c.text),false);}
 const c=approvalReady();c.approvalDraft.bundle.approvedSha256=hash;assert.equal(await verifyApprovalDraftHash(c),false);
});

test('software guard uses generated policy identity and an explicit feature result',async()=>{const {PublicGuardPolicy}=app('@nai/contracts');assert.equal(guardPolicy,PublicGuardPolicy[PublicGuardPolicy.PUBLIC_GUARD_POLICY_EXACT_SPECS_AND_SOFTWARE_V5]);const c=guarded();c.guardReview.reason=8;c.axes[3].reason='GUARD_EXACT_FEATURE_PASSED';assert.equal(validPublicCandidate(c,request,c.text),true);assert.equal(await verifyGuardReviewHash(c),true);const html=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:c}));assert.match(html,/本机功能原文通过/);for(const old of ['nai-public-exact-spec-v3-sentences','nai-public-exact-spec-v4-faq']){const bad=structuredClone(c);bad.guardReview.object.policyId=old;assert.equal(validPublicCandidate(bad,request,bad.text),false);}const mismatch=structuredClone(c);mismatch.guardReview.policyPassed=false;assert.equal(validPublicCandidate(mismatch,request,mismatch.text),false);});

test('insufficient playback window is an explicit rejection without a playback action',()=>{const c=guarded();c.guardReview.reason=9;c.guardReview.policyPassed=false;c.axes[3]={axis:'guard',state:3,reason:'GUARD_PLAYBACK_WINDOW_TOO_SHORT'};assert.equal(validPublicCandidate(c,request,c.text),true);const html=renderToStaticMarkup(React.createElement(PublicCandidateResult,{check:c}));assert.match(html,/剩余有效期不足以完整播出/);assert.match(html,/未自动重试/);assert.doesNotMatch(html,/<button/);});

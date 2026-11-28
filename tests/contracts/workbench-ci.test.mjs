import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import YAML from 'yaml';
test('active V4.3 CI triggers both platforms and propagates engineering failures',()=>{
 const w=YAML.parse(fs.readFileSync('.github/workflows/foundation.yml','utf8'));
 assert.deepEqual(Object.keys(w.on).sort(),['pull_request','push']);
 assert.deepEqual(w.jobs.foundation.strategy.matrix.os,['ubuntu-24.04','windows-latest']);
 const steps=w.jobs.foundation.steps;const gate=steps.find(s=>s.name==='Workbench engineering scope');
 assert.match(gate.run,/set -o pipefail/);assert.match(gate.run,/cargo xtask check --profile workbench/);
 for(const s of steps)assert.notEqual(s['continue-on-error'],true);
 assert.ok(steps.some(s=>s.run==='cargo xtask schema --check'));
 assert.ok(steps.some(s=>s.run==='cargo xtask test --suite contracts'&&s.if==="runner.os == 'Linux'"));
 const native=steps.find(s=>s.name==='Windows native engine and host');assert.match(native.run,/nai-workbench-host/);assert.match(native.run,/NAI_WORKBENCH_ENGINE/);assert.match(native.run,/NAI_PUBLIC_OUTPUT_KEEPER/);assert.match(native.run,/nai-public-output-keeper/);assert.equal(native.if,"runner.os == 'Windows'");
 assert.equal(steps.find(s=>s.run==='cargo xtask check --secrets').if,'always()');
});
test('profile executes source tests, real new engine and dependency checks',()=>{
 const source=fs.readFileSync('xtask/src/lib.rs','utf8');const body=source.split('fn workbench_checks(')[1].split('/// 两种工程 profile')[0];
 for(const target of ['deps::check(root)?','scripts/check-v561-registry.mjs','tests/contracts/workbench-source.test.mjs','tests/contracts/workbench-ci.test.mjs','tests/ui/v43-bootstrap.test.mjs','nai-foundation-sqlite','tests/contracts/foundation.test.mjs','nai-runtime-core','nai-workbench-engine','nai-workbench-host','scripts/workbench-frontend.mjs'])assert.ok(body.includes(target),target);
 assert.match(source,/\["check", "--profile", "workbench"\] => workbench_checks\(root\)/);
 assert.doesNotMatch(body,/--workspace|continue-on-error/);
});



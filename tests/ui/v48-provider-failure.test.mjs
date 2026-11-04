import test from 'node:test';import assert from 'node:assert/strict';
import {providerFailureMessage} from '../../apps/workbench-desktop/ui/provider-failure.ts';
test('balance errors are distinct from permissions and no unknown raw code is echoed',()=>{
 assert.match(providerFailureMessage('BALANCE_INSUFFICIENT'),/余额不足|欠费/);assert.match(providerFailureMessage('HTTP_402'),/余额不足/);
 assert.match(providerFailureMessage('HTTP_403'),/拒绝访问/);assert.doesNotMatch(providerFailureMessage('HTTP_403'),/欠费/);
 assert.match(providerFailureMessage('HTTP_401'),/认证失败/);assert.match(providerFailureMessage('HTTP_429'),/限流|配额/);
 assert.match(providerFailureMessage('WORKBENCH_ERROR_DEADLINE_EXCEEDED'),/超时/);assert.match(providerFailureMessage('HTTP_503'),/服务异常/);
 assert.doesNotMatch(providerFailureMessage('raw secret-key'),/secret-key/);
});

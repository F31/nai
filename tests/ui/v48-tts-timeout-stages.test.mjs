import test from 'node:test';import assert from 'node:assert/strict';
import {providerFailureMessage} from '../../apps/workbench-desktop/ui/provider-failure.ts';
test('timeout stages identify the last waiting point without asserting an unproven proxy or billing fault',()=>{
 assert.match(providerFailureMessage('CONNECT_TIMEOUT'),/连接供应商超时/);
 assert.match(providerFailureMessage('RESPONSE_TIMEOUT'),/等待供应商响应/);
 assert.match(providerFailureMessage('AUDIO_DOWNLOAD_TIMEOUT'),/已响应.*音频下载/);
 assert.doesNotMatch(providerFailureMessage('RESPONSE_TIMEOUT'),/欠费|代理故障/);
});

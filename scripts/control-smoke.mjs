// Launch an owned, empty real control process on an ephemeral loopback port.
// The test credential is generated in memory and is never printed or saved.
import assert from 'node:assert/strict';
import { randomBytes, randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { ControlClient } from '../apps/desktop/src/control.ts';

const token=randomBytes(32).toString('hex');
const child=spawn(`target/debug/examples/local_control${process.platform==='win32' ? '.exe' : ''}`,[],{
  env:{...process.env,NAI_CONTROL_TOKEN:token,NAI_CONTROL_PORT:'0'},stdio:['ignore','pipe','pipe'],
});
let output='';let failure;
child.stdout.on('data',data=>{output+=data.toString();});
child.stderr.on('data',()=>{});
child.on('error',error=>{failure=error;});
const exited=once(child,'exit').catch(()=>[]);
const watchdog=setTimeout(()=>child.kill(),15000);
try {
  let match;
  for (let attempt=0;attempt<100;attempt++) {
    if (failure) throw failure;
    match=output.match(/http:\/\/127\.0\.0\.1:(\d+)\/api\/v1/);
    if (match) break;
    if (child.exitCode!==null) throw new Error('control example exited before listening');
    await delay(50);
  }
  assert.ok(match,'control example did not start');
  const address=`http://127.0.0.1:${match[1]}`;
  const client=new ControlClient(address,token);
  const before=await client.status();
  assert.equal(before.session.lifecycle,'created');
  assert.equal(before.session.media_state,'standby');
  assert.equal(before.session.generation_epoch,'0');
  assert.equal((await fetch(`${address}/api/v1/health`)).status,401);
  const mute=await client.command(before,'mute',randomUUID());
  const muted=await client.status();
  assert.equal(mute.executed,true);
  assert.equal(muted.session.generation_epoch,'1');
  assert.equal(muted.session.media_state,'muted');
  const takeover=await client.command(muted,'takeover',randomUUID());
  assert.equal(takeover.generation_epoch,'2');
  assert.equal((await client.status()).last_command_seq,'2');
  assert.equal(output.includes(token),false);
  console.log('control smoke passed: real desktop client → loopback HTTP → Session mute/takeover; no hardware ACK claimed');
} finally {
  clearTimeout(watchdog);child.kill();await exited;
}

import test from 'node:test';
import assert from 'node:assert/strict';
import { ControlClient, parseRuntime } from '../src/control.ts';

const token = 'test_only_credential_01234567890123456789';
const fixture = () => ({ resource_id:'s',revision:'9007199254740993',status:'control_connected',safe_message:'test-only',device_id:'d',last_command_seq:'9007199254740993',session:{session_id:'s',generation_epoch:'9007199254740993',state_revision:'9007199254740993',lifecycle:'created',mode:'assisted',media_state:'standby'} });

test('only explicit loopback URL and memory credential are accepted', () => {
  for (const url of ['https://example.invalid','http://localhost:4318','http://127.0.0.1:4318?token=x','http://user:pass@127.0.0.1:4318','http://127.0.0.1:4318/other']) {
    assert.throws(() => new ControlClient(url,token));
  }
  assert.throws(() => new ControlClient('http://127.0.0.1:4318','short'));
});

test('response parsing refuses unsafe integers and unknown/missing states', () => {
  assert.equal(parseRuntime(fixture()).session.generation_epoch,'9007199254740993');
  for (const change of [s => s.generation_epoch=9007199254740993,s => s.generation_epoch='18446744073709551616',s => s.lifecycle='unknown',s => delete s.media_state,s => s.session_id='other']) {
    const view=fixture();change(view.session);assert.throws(() => parseRuntime(view));
  }
  const view=fixture();delete view.device_id;assert.throws(() => parseRuntime(view));
});

test('client sends token only in headers and preserves exact command sequence', async () => {
  const calls=[];
  const client=new ControlClient('http://127.0.0.1:4318',token,async (url,options) => {
    calls.push({url,options});
    return new Response(JSON.stringify(options.method==='GET' ? fixture() : {command_id:'test-command',executed:true,state_revision:'9007199254740994',generation_epoch:'9007199254740994'}));
  });
  const view=await client.status();
  const ack=await client.command(view,'mute','test-command');
  assert.equal(ack.generation_epoch,'9007199254740994');
  assert.equal(calls.length,2);
  assert.equal(calls[0].options.headers.Authorization,`Bearer ${token}`);
  assert.equal(calls[0].options.credentials,'omit');
  assert.equal(calls[0].options.redirect,'error');
  assert.ok(calls[0].options.signal);
  assert.equal(calls[0].url.includes(token),false);
  const body=JSON.parse(calls[1].options.body);
  assert.equal(body.seq,'9007199254740994');
  assert.equal(body.expected_state_revision,'9007199254740993');
  assert.equal(body.device_id,'d');
});

test('failure/false ACK never becomes success and no automatic retry occurs', async () => {
  for (const response of [new Response(JSON.stringify({error:{code:'UNAUTHENTICATED'}}),{status:401}),new Response(JSON.stringify({command_id:'cmd',executed:false,state_revision:'1',generation_epoch:'1'})),new Response('not-json')]) {
    let calls=0;
    const client=new ControlClient('http://127.0.0.1:4318',token,async () => {calls++;return response;});
    await assert.rejects(client.command(parseRuntime(fixture()),'mute','cmd'));
    assert.equal(calls,1);
  }
  const client=new ControlClient('http://127.0.0.1:4318',token,async () => {throw new Error('network disconnected');});
  await assert.rejects(client.status(),/network disconnected/);
});

test('bounded responses and exhausted device sequences reject', async () => {
  let calls=0;
  const client=new ControlClient('http://127.0.0.1:4318',token,async () => {calls++;return new Response('x'.repeat(65537));});
  await assert.rejects(client.status(),/上限/);
  const view=fixture();view.last_command_seq='18446744073709551615';
  await assert.rejects(client.command(parseRuntime(view),'mute','cmd'),/耗尽/);
  assert.equal(calls,1);
});

import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { Envelope, PerformanceCue, AudioBlockMetadata, PermissionSnapshot, ContractError, ErrorCode, CapabilityResult, AuthoritativeFact, PrepareReport, UsageLedgerEntry, PlatformEvent, OutputWatchdogCommand, OutputWatchdogStatus, FrameSlotEvent } from '../../packages/contracts-ts/dist/index.js';

const [input,output] = process.argv.slice(2);
if (!input || !output) throw new Error('interop input/output directories required');
await mkdir(output,{recursive:true});
for (const [name,codec] of Object.entries({envelope:Envelope,cue:PerformanceCue,audio:AudioBlockMetadata,permissions:PermissionSnapshot,error:ContractError,stream:CapabilityResult,fact:AuthoritativeFact,prepare:PrepareReport,ledger:UsageLedgerEntry,platform:PlatformEvent,watchdog_command:OutputWatchdogCommand,watchdog_status:OutputWatchdogStatus,frame_slot:FrameSlotEvent})) {
  const message = codec.decode(await readFile(path.join(input,`${name}.bin`)));
  if (name==='envelope') { assert.equal(message.generationEpoch,'9007199254740993'); assert.equal(message.sentAtUtcMs,'18446744073709551615'); }
  if (name==='cue') { assert.equal(message.generationEpoch,'9007199254740993'); assert.equal(message.displayText,'测试：一千二百九十九元'); }
  if (name==='audio') { assert.equal(message.seq,'9007199254740993'); assert.equal(message.sampleStart,'18446744073709551615'); }
  if (name==='permissions') { assert.equal(message.capabilities['unknown-test'],77); assert.notEqual(message.capabilities['unknown-test'],1); }
  if (name==='stream') { assert.equal(message.end,3); assert.ok(message.error); }
  if (name==='fact') { assert.equal(message.value?.$case,'money'); assert.equal(message.value.money.minorUnits,'129900'); assert.equal(message.value.money.currency,'CNY'); }
  if (name==='prepare') { assert.equal(message.readiness,3); assert.equal(message.snapshot.revision,'9007199254740993'); assert.equal(message.snapshot.bindings[0].selected.pluginVersion,'0.1.0'); assert.equal(message.snapshot.bindings[0].selected.policyScopeRef,'test-scope'); }
  if (name==='ledger') { assert.equal(message.estimate.costMinor,'18446744073709551615'); assert.equal(message.estimate.units.tokens,'9007199254740993'); assert.equal(message.state,4); assert.equal(message.cancelled,true); assert.equal(message.actual,undefined); }
  if (name==='platform') { assert.equal(message.generationEpoch,'9007199254740993'); assert.equal(message.connectionGeneration,'18446744073709551615'); }
  if (name==='watchdog_command') { assert.equal(message.seq,'18446744073709551615'); assert.equal(message.action.$case,'arm'); assert.equal(message.action.arm.generationEpoch,'9007199254740993'); assert.equal(message.remainingBudgetMs,250); }
  if (name==='watchdog_status') { assert.equal(message.epochFloor,'9007199254740993'); assert.equal(message.skippedStatuses,'18446744073709551615'); assert.equal(message.outputAvailable,false); assert.equal(message.backendError.code,ErrorCode.CAPABILITY_NOT_READY); }
  if (name==='frame_slot') { assert.equal(message.event.$case,'committed');const ticket=message.event.committed.ticket;assert.equal(ticket.generationEpoch,'9007199254740993');assert.equal(ticket.slotGeneration,'18446744073709551615');assert.equal(ticket.frame.pts,'9223372036854775807');assert.equal(ticket.frame.frameSeq,'18446744073709551615');assert.equal(message.event.committed.byteLength,8);assert.equal(ticket.frame.bufferRef,'test-pool:15:18446744073709551615'); }
  await writeFile(path.join(output,`${name}.bin`),codec.encode(message).finish());
}
console.log('TS cross-language roundtrip passed (13 message types; large integers, Unicode, unknown field/enum, exact money, versioned binding, usage, platform, watchdog and nested frame-slot ticket/commit)');

// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { connect, createClient, type Request } from './index.ts';
test('inspection requires an explicit closed scope and current snapshot revision',async()=>{
 const calls:Request[]=[];const client=createClient(async request=>{calls.push(request);return null;});
 await client.inspection.snapshot('game','mods');await client.inspection.check('game','mods','revision');
 assert.deepEqual(calls,[{method:'inspection.snapshot',params:{gameId:'game',scope:'mods'}},{method:'inspection.check',params:{gameId:'game',scope:'mods',revision:'revision'}}]);
 assert.throws(()=>client.inspection.check('game','mods',''));assert.throws(()=>client.inspection.snapshot('game','accounts' as never));assert.equal(calls.length,2);
 assert.equal('write' in client.inspection,false);
});

test('diagnostics uses explicit game scope and immutable revision without paths',async()=>{
 const calls:Request[]=[];const client=createClient(async request=>{calls.push(request);return null;});
 await client.diagnostics.list('game');await client.diagnostics.read('game','opaque');await client.diagnostics.check('game','opaque','revision');await client.diagnostics.pick();
 assert.deepEqual(calls,[{method:'diagnostics.list',params:{gameId:'game'}},{method:'diagnostics.read',params:{gameId:'game',diagnosticId:'opaque'}},{method:'diagnostics.check',params:{gameId:'game',diagnosticId:'opaque',revision:'revision'}},{method:'diagnostics.pick',params:{}}]);
 assert.throws(()=>client.diagnostics.check('game','opaque',''));assert.throws(()=>client.diagnostics.list('bad\nidentity'));assert.equal(calls.length,4);
 assert.equal('write' in client.diagnostics,false);
});

test('keybinding methods expose only typed registered IDs, revisions and opaque backups',async()=>{
  const calls:Request[]=[];const client=createClient(async request=>{calls.push(request);return null;});
  await client.keybindings.preview('source','target');await client.keybindings.apply('source','target','before-source','before-target');await client.keybindings.restore('target','backup','after-target');
  assert.deepEqual(calls,[{method:'keybindings.preview',params:{sourceGameId:'source',targetGameId:'target'}},{method:'keybindings.apply',params:{sourceGameId:'source',targetGameId:'target',sourceRevision:'before-source',targetRevision:'before-target'}},{method:'keybindings.restore',params:{targetGameId:'target',backupId:'backup',targetRevision:'after-target'}}]);
  assert.throws(()=>client.keybindings.apply('source','target','','revision'));assert.throws(()=>client.keybindings.restore('target','bad\nbackup','revision'));assert.equal(calls.length,3);
});

test('exposes only the declared childview operations and exact context-bound arguments', async () => {
  const calls: Request[] = [];
  const client = createClient(async request => { calls.push(request); return null; });
  await client.context(); await client.configs.list('game'); await client.configs.read('game', 'config');
  await client.report.check('game', ['config']); await client.report.cancel();
  assert.deepEqual(calls, [
    { method: 'context', params: {} }, { method: 'configs.list', params: {gameId:'game'} },
    { method: 'configs.read', params: {gameId:'game',configId:'config'} },
    { method: 'report.check', params: {gameId:'game',configIds:['config']} }, { method: 'report.cancel', params: {} },
  ]);
  assert.equal('invoke' in client, false); assert.equal('write' in client.configs, false);
  assert.equal(Object.isFrozen(client), true);
});

test('rejects empty, duplicate, excessive or malformed selections before transport', () => {
  let calls = 0; const client = createClient(async () => { calls++; });
  for (const ids of [[], ['a','a'], Array.from({length:65},(_,i)=>String(i))]) assert.throws(()=>client.report.check('game',ids));
  assert.throws(()=>client.configs.read('game','bad\ncontext')); assert.throws(()=>client.configs.list(''));
  assert.equal(calls,0);
});

test('preserves native rejection and cancellation without inventing a report', async () => {
  const error = new Error('FRONTEND_SCOPE_DENIED');
  const client = createClient(async request => { if(request.method==='report.check')return null; throw error; });
  assert.equal(await client.report.check('game',['config']),null);
  await assert.rejects(client.configs.list('game'), value=>value===error);
});

test('connects only through the injected restricted bridge and fails closed elsewhere', async () => {
  assert.throws(()=>connect(), /PLUGIN_BRIDGE_UNAVAILABLE/);
  const calls: unknown[] = [];
  Object.defineProperty(globalThis,'__COBBLE_PLUGIN_BRIDGE__',{configurable:true,value:{request:async(method:string,params:unknown)=>{calls.push([method,params]);return null;}}});
  try { await connect().report.cancel();assert.deepEqual(calls,[['report.cancel',{}]]); }
  finally { Reflect.deleteProperty(globalThis,'__COBBLE_PLUGIN_BRIDGE__'); }
  assert.throws(()=>connect(), /PLUGIN_BRIDGE_UNAVAILABLE/);
});

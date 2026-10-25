// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { connect, createClient, type Request } from './index.ts';
test('world metadata uses a scoped direct world ID without accepting a path', async () => {
 const calls:Request[]=[];const client=createClient(async request=>{calls.push(request);return null;});
 await client.worlds.metadata('registered-game','世界 One');
 assert.deepEqual(calls,[{method:'worlds.metadata',params:{gameId:'registered-game',worldId:'世界 One'}}]);
 for(const world of ['../private','C:\\private','.',''])assert.throws(()=>client.worlds.metadata('registered-game',world));
 assert.equal(calls.length,1);
});
test('seed prediction preserves decimal seed text and cancels only its explicit request ID', async () => {
 const calls:Request[]=[];const client=createClient(async request=>{calls.push(request);return request.method==='seedMap.cancel'?true:null;});
 const map={seed:'-9223372036854775808',version:'1.21.1',dimension:'overworld' as const,preset:'default' as const,y:64,x:0,z:0,width:16,height:16,step:4};
 const search={seed:'9223372036854775807',version:'1.21.1',dimension:'overworld' as const,preset:'default' as const,y:64,x:0,z:0,radius:1024,target:{kind:'biome' as const,id:'plains'},limit:16};
 await client.seedMap.capabilities();await client.seedMap.generate({requestId:'map-1',map});await client.seedMap.search({requestId:'search-1',search});
 assert.equal(await client.seedMap.cancel({requestId:'search-1'}),true);
 assert.deepEqual(calls,[{method:'seedMap.capabilities',params:{}},{method:'seedMap.generate',params:{requestId:'map-1',map}},{method:'seedMap.search',params:{requestId:'search-1',search}},{method:'seedMap.cancel',params:{requestId:'search-1'}}]);
 assert.throws(()=>client.seedMap.generate({requestId:'bad/path',map}));
 assert.throws(()=>client.seedMap.generate({requestId:'numeric',map:{...map,seed:123 as never}}));
 assert.equal(calls.length,4);
});
test('selected-file report picker supplies no paths or implicit diagnostic scope', async () => {
 const calls:Request[]=[];const client=createClient(async request=>{calls.push(request);return null;});
 assert.equal(await client.report.pick(),null);
 assert.deepEqual(calls,[{method:'report.pick',params:{}}]);
 assert.equal('readFile' in client.report,false);
});
test('inspection requires an explicit closed scope and current snapshot revision',async()=>{
 const calls:Request[]=[];const client=createClient(async request=>{calls.push(request);return null;});
 await client.inspection.snapshot('game','mods');await client.inspection.check('game','mods','revision');
 assert.deepEqual(calls,[{method:'inspection.snapshot',params:{gameId:'game',scope:'mods'}},{method:'inspection.check',params:{gameId:'game',scope:'mods',revision:'revision'}}]);
 assert.throws(()=>client.inspection.check('game','mods',''));assert.throws(()=>client.inspection.snapshot('game','accounts' as never));assert.equal(calls.length,2);
 assert.equal('write' in client.inspection,false);
});
test('world snapshot API keeps game and world IDs scoped and pages bounded', async () => {
 const calls: Request[]=[];const client=createClient(async request=>{calls.push(request);return null;});
 await client.worldSnapshots.list('game','World One');
 await client.worldSnapshots.compare('game','World One','before','after',50,50,'region');
 await client.worldSnapshots.openRestore('game','World One','snapshot');
 assert.deepEqual(calls,[
  {method:'worldSnapshots.list',params:{gameId:'game',worldId:'World One'}},
  {method:'worldSnapshots.compare',params:{gameId:'game',worldId:'World One',beforeId:'before',afterId:'after',offset:50,limit:50,query:'region'}},
  {method:'worldSnapshots.openRestore',params:{gameId:'game',worldId:'World One',snapshotId:'snapshot'}},
 ]);
 assert.throws(()=>client.worldSnapshots.compare('game','world','a','b',0,101));
 assert.throws(()=>client.worldSnapshots.compare('game','world','a','b',-1,10));
 assert.throws(()=>client.worldSnapshots.compare('game','world','a','b',0,10,'x'.repeat(121)));
 assert.throws(()=>client.worldSnapshots.openRestore('game\nprivate','world','snapshot'));
 assert.equal(calls.length,3);
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

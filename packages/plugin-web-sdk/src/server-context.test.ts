// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { createClient } from './index.ts';
import { listServerContexts } from './server-context.ts';
test('server context listing accepts host opaque IDs and deduplicates endpoint aliases', async () => {
  const serverId = 'a'.repeat(64);
  const client = createClient(async request => {
    assert.deepEqual(request, {method:'inspection.snapshot',params:{gameId:'g',scope:'servers'}});
    return {schemaVersion:1,scope:'servers',game:{id:'g',name:'Game',version:'1.21.1'},revision:'r',data:{available:true,truncated:false,items:[{id:0,serverId,name:'Home'},{id:1,serverId,name:'Alias'},{id:2,name:'No endpoint'},{serverId:'example.org',name:'Address'}]}};
  });
  assert.deepEqual(await listServerContexts(client,'g'),{available:true,truncated:false,items:[{serverId,name:'Home'}]});
});
test('server context listing rejects mismatched snapshots and propagates unavailable transport', async () => {
  await assert.rejects(listServerContexts(createClient(async () => ({scope:'servers',game:{id:'other'},data:{items:[]}})), 'g'), /PLUGIN_SERVER_LIST_INVALID/);
  await assert.rejects(listServerContexts(createClient(async () => {throw new Error('UNAVAILABLE');}), 'g'), /UNAVAILABLE/);
});

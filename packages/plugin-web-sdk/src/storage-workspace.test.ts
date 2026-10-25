// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { createStorageWorkspace } from './storage-workspace.ts';
test('storage methods send scoped opaque identifiers and no filesystem paths', async () => {
    const calls: unknown[] = [];
    const api = createStorageWorkspace(async request => { calls.push(request); return null; });
    await api.overview(); await api.overview({gameId:'game-a'}); await api.pickDestination({gameId:'game-a'});
    await api.startMove({planId:'abc123'}); await api.task({taskId:'abc123'}); await api.cancel({taskId:'abc123'});
    await api.releaseSource({taskId:'abc123'}); await api.openSource({taskId:'abc123'}); await api.cleanCache();
    assert.equal(calls.length,9);
    assert.deepEqual(calls[0],{method:'storageWorkspace.overview',params:{}});
    assert.deepEqual(calls[3],{method:'storageWorkspace.startMove',params:{planId:'abc123'}});
    assert.throws(()=>api.pickDestination({gameId:'../game'}),/INVALID_ID/);
    assert.throws(()=>api.releaseSource({taskId:'C:\\game'}),/INVALID_ID/);
});

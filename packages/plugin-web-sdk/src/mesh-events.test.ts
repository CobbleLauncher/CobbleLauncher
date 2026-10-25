// SPDX-License-Identifier: MIT
import test, { type TestContext } from 'node:test';
import assert from 'node:assert/strict';
import { createClient, type JsonValue, type Request, type Transport } from './index.ts';
import { createMeshEvents, type MeshMessage, type MeshStatus } from './mesh-events.ts';
const ns = 'org.example.ready.v1', id = 'A'.repeat(32), peer = '12D3KooWFixturePeer';
const flush = () => new Promise<void>(resolve => setImmediate(resolve));
async function tick(t: TestContext, ms = 0) { t.mock.timers.tick(ms); await flush(); }
function message(sequence = 1): MeshMessage { return { sequence, messageId: id, peerId: peer, namespace: ns, event: 'ready', payload: { ready: true }, receivedAt: 1760000000 }; }
function fixture(t: TestContext, custom?: Transport) {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const calls: Request[] = []; let subscriptions = 0;
  const transport: Transport = async request => {
    calls.push(request);
    if (custom) return custom(request);
    if (request.method === 'mesh.subscribe') return { subscriptionId: `subscription-${++subscriptions}`, cursor: 0 };
    if (request.method === 'mesh.receive') return { messages: [], cursor: request.params.after, missed: false };
    if (request.method === 'mesh.send') return { messageId: id, status: 'received' };
    return null;
  };
  const mesh = createMeshEvents(transport), channel = mesh.namespace(ns);
  t.after(() => channel.close()); return { mesh, channel, calls, transport };
}
function visibility(t: TestContext) {
  const doc = new EventTarget() as EventTarget & { visibilityState: string }; doc.visibilityState = 'visible';
  const win = new EventTarget();
  for (const [key, value] of [['document', doc], ['window', win]] as const) {
    const previous = Object.getOwnPropertyDescriptor(globalThis, key);
    Object.defineProperty(globalThis, key, { value, configurable: true });
    t.after(() => { if (previous) Object.defineProperty(globalThis, key, previous); else Reflect.deleteProperty(globalThis, key); });
  }
  return { doc, win, hide: () => { doc.visibilityState = 'hidden'; doc.dispatchEvent(new Event('visibilitychange')); }, show: () => { doc.visibilityState = 'visible'; doc.dispatchEvent(new Event('visibilitychange')); } };
}

test('client/namespace/status creation is lazy, exact and cached', async t => {
  const { transport, calls, mesh, channel } = fixture(t);
  const client = createClient(transport), statuses: MeshStatus[] = [];
  const other = client.mesh.namespace(ns); other.onStatus(value => { statuses.push(value); });
  assert.equal(mesh.namespace(ns), channel); assert.deepEqual(statuses, [{ type: 'idle' }]);
  await tick(t, 10000); assert.equal(calls.length, 0); other.close();
});

test('validators share exact DNS/event/UTF8 JSON depth and byte bounds', async t => {
  const { mesh, channel, calls } = fixture(t);
  for (const name of ['org.*', 'Org.example', 'org.example\n', 'cobble.events', 'org.-bad', 'org.bad-', 'x', 'a'.repeat(64) + '.x', 'a'.repeat(63) + '.' + 'b'.repeat(33)]) assert.throws(() => mesh.namespace(name), /MESH_EVENT_/);
  const maximum = mesh.namespace('a'.repeat(63) + '.' + 'b'.repeat(32)); maximum.close();
  const reserved = ['connect','connect_error','disconnect','disconnecting','error','reconnect','reconnect_attempt','reconnect_error','reconnect_failed','newlistener','removelistener','companion_exited','network_state_changed','group_state_changed','app_events_changed'];
  for (const event of [...reserved, 'ready\n', 'Upper', '', 'x'.repeat(65)]) assert.throws(() => channel.on(event, () => {}), /MESH_EVENT_NAME_INVALID/);
  await channel.send(peer, 'ready', 'x'.repeat(1022)); await channel.send(peer, 'ready', '汉'.repeat(340));
  await assert.rejects(channel.send(peer, 'ready', 'x'.repeat(1023)), /PAYLOAD_TOO_LARGE/);
  await assert.rejects(channel.send(peer, 'ready', '汉'.repeat(341)), /PAYLOAD_TOO_LARGE/);
  let payload: JsonValue = 0; for (let depth = 0; depth < 8; depth++) payload = [payload];
  await channel.send(peer, 'ready', payload); await assert.rejects(channel.send(peer, 'ready', [payload]), /PAYLOAD_INVALID/);
  for (const value of [undefined, NaN, Infinity, { nested: undefined }, { nested: () => {} }]) await assert.rejects(channel.send(peer, 'ready', value as JsonValue), /PAYLOAD_INVALID/);
  const cycle: Record<string, unknown> = {}; cycle.self = cycle;
  await assert.rejects(channel.send(peer, 'ready', cycle as JsonValue), /PAYLOAD_INVALID/);
  assert.equal(calls.length, 3);
});

test('peers and send use the host facade, immutable JSON snapshots and opaque uppercase IDs', async t => {
  const { mesh, channel, calls } = fixture(t, async request => request.method === 'mesh.peers' ? { peers: [{ peerId: peer, displayName: 'Alex', connected: true }] } : { messageId: id, status: 'received' });
  assert.deepEqual(await mesh.peers(), { peers: [{ peerId: peer, displayName: 'Alex', connected: true }] });
  const payload = { ready: true }; const sent = channel.send(peer, 'ready', payload); payload.ready = false;
  assert.deepEqual(await sent, { messageId: id, status: 'received' });
  assert.deepEqual(calls[1], { method: 'mesh.send', params: { peerId: peer, namespace: ns, event: 'ready', payload: { ready: true } } });
});

test('send failures and unknown delivery are never retried or converted to received', async t => {
  const { channel, calls } = fixture(t, async () => { throw Error('MESH_EVENT_DELIVERY_UNKNOWN'); });
  await assert.rejects(channel.send(peer, 'ready', null), /DELIVERY_UNKNOWN/);
  await tick(t, 30000); assert.equal(calls.length, 1);
});

test('inbound host-admitted scientific floats are not rejected by JS reserialization size', async t => {
  const payload = Array.from({length:150}, () => 1e-6);
  assert(new TextEncoder().encode(JSON.stringify(payload)).byteLength > 1024);
  const { channel } = fixture(t, async request => request.method === 'mesh.subscribe' ? {subscriptionId:'owned',cursor:0} : request.method === 'mesh.receive' ? {messages:[{...message(),payload}],cursor:1,missed:false} : null);
  const observed: unknown[] = []; channel.on('ready', value => { observed.push(value); }); await tick(t);
  assert.deepEqual(observed, [payload]);
});

test('one namespace serializes reads, pages four records and forwards the acknowledgement cursor', async t => {
  let resolveRead!: (value: unknown) => void, reads = 0;
  const { mesh, channel, calls } = fixture(t, async request => {
    if (request.method === 'mesh.subscribe') return { subscriptionId: 'owned', cursor: 0 };
    if (request.method === 'mesh.receive' && ++reads === 1) return new Promise(resolve => { resolveRead = resolve; });
    if (request.method === 'mesh.receive') return { messages: [], cursor: 4, missed: false };
    return null;
  });
  const observed: number[] = []; const off = channel.on('ready', (_payload, value) => { observed.push(value.sequence); });
  const offSecond = mesh.namespace(ns).on('ready', () => {});
  await tick(t); await tick(t, 10000); assert.equal(reads, 1);
  resolveRead({ messages: [1,2,3,4].map(message), cursor: 4, missed: false }); await flush();
  assert.deepEqual(observed, [1,2,3,4]); off(); await tick(t, 500);
  assert.deepEqual(calls.filter(call => call.method === 'mesh.receive').map(call => call.params), [{ subscriptionId:'owned',after:0,limit:4 }, { subscriptionId:'owned',after:4,limit:4 }]);
  offSecond(); await flush(); assert.equal(calls.filter(call => call.method === 'mesh.unsubscribe').length, 1);
});

test('off is idempotent after another listener has been registered for the same event', async t => {
  const { channel } = fixture(t, async request => request.method === 'mesh.subscribe' ? { subscriptionId: 'owned', cursor: 0 } : request.method === 'mesh.receive' ? { messages: [message()], cursor: 1, missed: false } : null);
  const old = channel.on('ready', () => {}); old();
  const observed: unknown[] = []; channel.on('ready', value => { observed.push(value); }); old();
  await tick(t); assert.deepEqual(observed, [{ ready: true }]);
});

test('missed cursors surface separately and handler exceptions cannot suppress another handler', async t => {
  const { channel } = fixture(t, async request => request.method === 'mesh.subscribe' ? { subscriptionId: 'owned', cursor: 0 } : request.method === 'mesh.receive' ? { messages: [message()], cursor: 1, missed: true } : null);
  const statuses: MeshStatus[] = [], observed: unknown[] = [];
  channel.onStatus(value => { statuses.push(value); });
  channel.on('ready', () => { throw Error('local application error'); });
  channel.on('ready', async () => { throw Error('async local error'); });
  channel.on('ready', (payload, metadata) => { observed.push([payload, metadata.peerId]); });
  await tick(t); assert.deepEqual(observed, [[{ ready:true }, peer]]);
  assert(statuses.some(value => value.type === 'missed' && value.cursor === 1));
  assert.equal(statuses.filter(value => value.type === 'handler-error').length, 2);
});

test('late subscribe results release their host handle after the last listener leaves', async t => {
  let subscribed!: (value: unknown) => void;
  const { channel, calls } = fixture(t, async request => request.method === 'mesh.subscribe' ? new Promise(resolve => { subscribed = resolve; }) : null);
  const off = channel.on('ready', () => { throw Error('must not run'); }); await tick(t); off();
  subscribed({ subscriptionId: 'late', cursor: 0 }); await flush();
  assert(calls.some(call => call.method === 'mesh.unsubscribe' && call.params.subscriptionId === 'late'));
  assert.equal(calls.filter(call => call.method === 'mesh.receive').length, 0);
});

test('hidden pages unsubscribe, ignore late receive callbacks and resume without overlapping reads', async t => {
  const view = visibility(t); let resolveRead!: (value: unknown) => void, subscriptions = 0, reads = 0;
  const { channel, calls } = fixture(t, async request => {
    if (request.method === 'mesh.subscribe') return { subscriptionId: `owned-${++subscriptions}`, cursor: 0 };
    if (request.method === 'mesh.receive' && ++reads === 1) return new Promise(resolve => { resolveRead = resolve; });
    if (request.method === 'mesh.receive') return { messages: [], cursor: 0, missed: false };
    return null;
  });
  const observed: unknown[] = []; channel.on('ready', value => { observed.push(value); }); await tick(t);
  view.hide(); await flush(); view.show(); await tick(t, 1000); assert.equal(subscriptions, 1);
  resolveRead({ messages:[message()],cursor:1,missed:false }); await flush(); await tick(t);
  assert.deepEqual(observed, []); assert.equal(subscriptions, 2);
  assert(calls.some(call => call.method === 'mesh.unsubscribe' && call.params.subscriptionId === 'owned-1'));
  view.win.dispatchEvent(new Event('pagehide')); await flush(); await tick(t, 10000);
  assert.equal(reads, 2);
});

test('only temporary receive errors use a bounded backoff, never subscribe retries', async t => {
  const { channel, calls } = fixture(t, async request => request.method === 'mesh.subscribe' ? { subscriptionId:'owned',cursor:0 } : request.method === 'mesh.receive' ? Promise.reject(Error('MESH_APP_BUSY')) : null);
  const statuses: MeshStatus[] = []; channel.onStatus(value => { statuses.push(value); }); channel.on('ready', () => {});
  await tick(t); await tick(t, 500); await tick(t, 1000); await tick(t, 2000); await tick(t, 30000);
  assert.deepEqual(statuses.filter(value => value.type === 'retrying').map(value => value.retryInMs), [500,1000,2000]);
  assert.deepEqual(statuses.at(-1), { type:'error',code:'MESH_APP_BUSY' });
  assert.equal(calls.filter(call => call.method === 'mesh.receive').length, 4); assert.equal(calls.filter(call => call.method === 'mesh.subscribe').length, 1);
});

test('permission revocation is terminal and stays visible after hide/show', async t => {
  const view = visibility(t);
  const { channel, calls } = fixture(t, async request => request.method === 'mesh.subscribe' ? { subscriptionId:'owned',cursor:0 } : request.method === 'mesh.receive' ? Promise.reject(Error('PLUGIN_PERMISSION_DENIED')) : null);
  const statuses: MeshStatus[] = []; channel.onStatus(value => { statuses.push(value); }); channel.on('ready', () => {}); await tick(t);
  view.hide(); view.show(); await tick(t, 10000);
  assert.deepEqual(statuses.at(-1), { type:'error',code:'PLUGIN_PERMISSION_DENIED' }); assert.equal(calls.filter(call => call.method === 'mesh.receive').length, 1);
});

test('invalid subscribe cursors release the issued handle and invalid pages halt before dispatch', async t => {
  const { channel, calls } = fixture(t, async request => request.method === 'mesh.subscribe' ? { subscriptionId:'invalid',cursor:-1 } : null);
  channel.on('ready', () => { throw Error('must not run'); }); await tick(t);
  assert(calls.some(call => call.method === 'mesh.unsubscribe' && call.params.subscriptionId === 'invalid'));
  const other = createMeshEvents(async request => request.method === 'mesh.subscribe' ? { subscriptionId:'owned',cursor:0 } : request.method === 'mesh.receive' ? {messages:Array.from({length:5},()=>message()),cursor:1,missed:false} : null).namespace(ns);
  const statuses: MeshStatus[] = []; other.onStatus(value => { statuses.push(value); }); other.on('ready', () => { throw Error('must not run'); }); await tick(t);
  assert.deepEqual(statuses.at(-1), {type:'error',code:'MESH_EVENT_RESPONSE_INVALID'}); other.close();
});

test('close stops later dispatch and callback errors, and a new namespace handle can be opened explicitly', async t => {
  const { mesh, channel } = fixture(t, async request => request.method === 'mesh.subscribe' ? {subscriptionId:'owned',cursor:0} : request.method === 'mesh.receive' ? {messages:[message()],cursor:1,missed:false} : null);
  let rejected!: (cause: Error) => void; const statuses: MeshStatus[] = [], observed: unknown[] = [];
  channel.onStatus(value => { statuses.push(value); });
  channel.on('ready', () => new Promise<void>((_resolve,reject) => { rejected=reject; })); channel.on('ready', () => { channel.close(); }); channel.on('ready', () => { observed.push('late'); });
  await tick(t); rejected(Error('late handler')); await flush(); assert.deepEqual(observed, []); assert.deepEqual(statuses.at(-1), {type:'closed'});
  assert.throws(() => channel.on('ready', () => {}), /CLOSED/); await assert.rejects(channel.send(peer,'ready',null), /CLOSED/);
  const next = mesh.namespace(ns); assert.notEqual(next, channel); next.close();
});

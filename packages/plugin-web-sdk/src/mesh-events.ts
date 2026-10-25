// SPDX-License-Identifier: MIT
import type { JsonValue, Transport } from './index.ts';

export type MeshPeer = { peerId: string; displayName: string; connected: boolean };
export type MeshPeers = { peers: MeshPeer[] };
export type MeshDelivery = { messageId: string; status: 'received' };
export type MeshMessage<T extends JsonValue = JsonValue> = Readonly<{
  sequence: number; messageId: string; peerId: string; namespace: string;
  event: string; payload: T; /** Authenticated host receipt time, Unix seconds. */ receivedAt: number;
}>;
export type MeshStatus =
  | { type: 'idle' | 'subscribing' | 'subscribed' | 'closed' }
  | { type: 'paused'; reason: 'hidden' | 'pagehide' }
  | { type: 'missed'; cursor: number }
  | { type: 'retrying'; code: string; attempt: number; retryInMs: number }
  | { type: 'error'; code: string }
  | { type: 'handler-error'; event: string };
export type MeshEventHandler<T extends JsonValue = JsonValue> = (payload: T, message: MeshMessage<T>) => void | Promise<void>;
export interface MeshNamespace {
  on<T extends JsonValue = JsonValue>(event: string, handler: MeshEventHandler<T>): () => void;
  onStatus(handler: (status: MeshStatus) => void | Promise<void>): () => void;
  /** Acknowledges admission by the remote host, not application or user consumption. Never retried. */
  send(peerId: string, event: string, payload: JsonValue): Promise<MeshDelivery>;
  /** Stops local callbacks immediately and requests host release. The host lease is the cleanup backstop. */
  close(): void;
}
const reserved = new Set(['connect', 'connect_error', 'disconnect', 'disconnecting', 'error', 'reconnect', 'reconnect_attempt', 'reconnect_error', 'reconnect_failed', 'newlistener', 'removelistener', 'companion_exited', 'network_state_changed', 'group_state_changed', 'app_events_changed']);
const retryable = new Set(['MESH_EVENT_BUSY', 'MESH_APP_BUSY', 'MESH_EVENT_UNAVAILABLE']);
const retryDelays = [500, 1000, 2000];
const pollInterval = 500;
function namespaceName(value: string): string {
  if (typeof value !== 'string' || value.length > 96 || /[^a-z0-9.-]/.test(value) || !/^(?:[a-z](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(value)) throw Error('MESH_EVENT_NAMESPACE_INVALID');
  if (value.startsWith('cobble.')) throw Error('MESH_EVENT_RESERVED_NAMESPACE');
  return value;
}
function eventName(value: string): string {
  if (typeof value !== 'string' || /[^a-z0-9_.:-]/.test(value) || !/^[a-z][a-z0-9_.:-]{0,63}$/.test(value) || reserved.has(value)) throw Error('MESH_EVENT_NAME_INVALID');
  return value;
}
function peerId(value: string): string {
  if (typeof value !== 'string' || value.length > 128 || /[^a-zA-Z0-9]/.test(value) || !value) throw Error('MESH_EVENT_PEER_UNKNOWN');
  return value;
}
function jsonPayload(value: JsonValue, outgoing = true): JsonValue {
  function validate(item: unknown, depth: number, parents: Set<object>): void {
    if (item === null || typeof item === 'string' || typeof item === 'boolean') return;
    if (typeof item === 'number' && Number.isFinite(item)) return;
    if (typeof item !== 'object' || parents.has(item) || depth >= 8 || (!Array.isArray(item) && ![Object.prototype, null].includes(Object.getPrototypeOf(item))) || typeof (item as { toJSON?: unknown }).toJSON === 'function') throw Error('MESH_EVENT_PAYLOAD_INVALID');
    parents.add(item);
    for (const child of Array.isArray(item) ? item : Object.values(item)) validate(child, depth + 1, parents);
    parents.delete(item);
  }
  try { validate(value, 0, new Set()); } catch { throw Error('MESH_EVENT_PAYLOAD_INVALID'); }
  let encoded: string;
  try { encoded = JSON.stringify(value); } catch { throw Error('MESH_EVENT_PAYLOAD_INVALID'); }
  // The host already bounds inbound serde_json bytes. JS formats some valid floats more verbosely.
  if (outgoing && new TextEncoder().encode(encoded).byteLength > 1024) throw Error('MESH_EVENT_PAYLOAD_TOO_LARGE');
  return JSON.parse(encoded) as JsonValue;
}
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('MESH_EVENT_RESPONSE_INVALID');
  return value as Record<string, unknown>;
}
function cursor(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw Error('MESH_EVENT_RESPONSE_INVALID');
  return value;
}
function code(cause: unknown): string {
  const value = typeof cause === 'string' ? cause : cause instanceof Error ? cause.message : cause && typeof cause === 'object' && 'code' in cause ? cause.code : '';
  return typeof value === 'string' && /^[A-Z][A-Z0-9_]{1,79}$/.test(value) && !/[^A-Z0-9_]/.test(value) ? value : 'MESH_EVENT_READ_FAILED';
}
function page(value: unknown, namespace: string, after: number) {
  const response = record(value), next = cursor(response.cursor);
  if (!Array.isArray(response.messages) || response.messages.length > 4 || typeof response.missed !== 'boolean' || next < after) throw Error('MESH_EVENT_RESPONSE_INVALID');
  const sequences = new Set<number>();
  const messages = response.messages.map(raw => {
    const item = record(raw), sequence = cursor(item.sequence);
    if (sequence <= after || sequence > next || sequences.has(sequence) || item.namespace !== namespace || typeof item.messageId !== 'string' || !/^[a-fA-F0-9]{32}$/.test(item.messageId) || /[^a-fA-F0-9]/.test(item.messageId) || typeof item.receivedAt !== 'number' || !Number.isSafeInteger(item.receivedAt) || item.receivedAt < 0) throw Error('MESH_EVENT_RESPONSE_INVALID');
    sequences.add(sequence);
    return Object.freeze({ sequence, messageId: item.messageId, peerId: peerId(item.peerId as string), namespace, event: eventName(item.event as string), payload: jsonPayload(item.payload as JsonValue, false), receivedAt: item.receivedAt });
  });
  return { messages, cursor: next, missed: response.missed };
}

/** Lazy, namespace-scoped facade over the source-bound host bridge. No work starts at client creation. */
export function createMeshEvents(transport: Transport) {
  const namespaces = new Map<string, MeshNamespace>();
  function namespace(value: string): MeshNamespace {
    const name = namespaceName(value), existing = namespaces.get(name);
    if (existing) return existing;
    const handlers = new Map<string, Map<symbol, MeshEventHandler>>();
    const statuses = new Map<symbol, (status: MeshStatus) => void | Promise<void>>();
    let lastStatus: MeshStatus = { type: 'idle' };
    let subscription: { id: string; cursor: number } | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let running = false, closed = false, halted = false, pageHidden = false, listening = false, generation = 0, failures = 0;
    const documentTarget = typeof document === 'undefined' ? undefined : document;
    const windowTarget = typeof window === 'undefined' ? undefined : window;
    const count = () => [...handlers.values()].reduce((sum, entries) => sum + entries.size, 0);
    const visible = () => !pageHidden && documentTarget?.visibilityState !== 'hidden';
    const active = () => !closed && !halted && count() > 0 && visible();
    function notify(handler: (status: MeshStatus) => void | Promise<void>, status: MeshStatus) {
      try { void Promise.resolve(handler(status)).catch(() => {}); } catch { /* One status observer cannot stop the stream. */ }
    }
    function status(value: MeshStatus) {
      lastStatus = Object.freeze(value);
      for (const [token, handler] of [...statuses]) if (statuses.has(token)) notify(handler, lastStatus);
    }
    function release(id: string) { void Promise.resolve().then(() => transport({ method: 'mesh.unsubscribe', params: { subscriptionId: id } })).catch(() => {}); }
    function stop() {
      ++generation; clearTimeout(timer); timer = undefined;
      if (subscription) release(subscription.id);
      subscription = null; failures = 0;
    }
    function schedule(delay: number) {
      if (!active() || timer !== undefined || running) return;
      timer = setTimeout(() => { timer = undefined; void poll(); }, delay);
    }
    function handlerFailed(event: string, current: number) { if (current === generation && active()) status({ type: 'handler-error', event }); }
    async function poll() {
      if (!active() || running) return;
      const current = generation;
      let delay = pollInterval, reading = false;
      running = true;
      try {
        if (!subscription) {
          status({ type: 'subscribing' });
          if (!active() || current !== generation) return;
          const value = record(await transport({ method: 'mesh.subscribe', params: { namespace: name } }));
          if (typeof value.subscriptionId !== 'string' || !value.subscriptionId || value.subscriptionId.length > 128 || /[\u0000-\u0020\u007f]/.test(value.subscriptionId)) throw Error('MESH_EVENT_RESPONSE_INVALID');
          const id = value.subscriptionId;
          if (!active() || current !== generation) { release(id); return; }
          let initialCursor: number;
          try { initialCursor = cursor(value.cursor); if (value.namespace !== undefined && value.namespace !== name) throw Error('MESH_EVENT_RESPONSE_INVALID'); }
          catch (cause) { release(id); throw cause; }
          subscription = { id, cursor: initialCursor }; status({ type: 'subscribed' });
        }
        if (!active() || current !== generation || !subscription) return;
        reading = true;
        const value = await transport({ method: 'mesh.receive', params: { subscriptionId: subscription.id, after: subscription.cursor, limit: 4 } });
        if (!active() || current !== generation || !subscription) return;
        const batch = page(value, name, subscription.cursor);
        subscription.cursor = batch.cursor; failures = 0;
        if (batch.missed) status({ type: 'missed', cursor: batch.cursor });
        else if (lastStatus.type === 'retrying') status({ type: 'subscribed' });
        for (const message of batch.messages) {
          for (const [token, handler] of [...(handlers.get(message.event) ?? [])]) {
            if (!active() || current !== generation) return;
            if (!handlers.get(message.event)?.has(token)) continue;
            try { void Promise.resolve(handler(message.payload, message)).catch(() => handlerFailed(message.event, current)); }
            catch { handlerFailed(message.event, current); }
          }
        }
      } catch (cause) {
        if (!active() || current !== generation) return;
        const error = code(cause), retry = reading && retryable.has(error) ? retryDelays[failures] : undefined;
        if (retry !== undefined) { failures++; delay = retry; status({ type: 'retrying', code: error, attempt: failures, retryInMs: retry }); }
        else { halted = true; stop(); status({ type: 'error', code: error }); }
      } finally { running = false; schedule(current === generation ? delay : 0); }
    }
    function visibility() {
      if (halted || closed) return;
      if (!visible()) { stop(); status({ type: 'paused', reason: pageHidden ? 'pagehide' : 'hidden' }); }
      else schedule(0);
    }
    function hidden() { pageHidden = true; visibility(); }
    function shown() { pageHidden = false; visibility(); }
    function attach() {
      if (listening) return;
      listening = true; documentTarget?.addEventListener('visibilitychange', visibility);
      windowTarget?.addEventListener('pagehide', hidden); windowTarget?.addEventListener('pageshow', shown);
    }
    function detach() {
      listening = false; documentTarget?.removeEventListener('visibilitychange', visibility);
      windowTarget?.removeEventListener('pagehide', hidden); windowTarget?.removeEventListener('pageshow', shown);
    }
    const channel = Object.freeze<MeshNamespace>({
      on<T extends JsonValue>(event: string, handler: MeshEventHandler<T>) {
        if (closed) throw Error('MESH_EVENT_CLOSED');
        eventName(event); if (typeof handler !== 'function') throw Error('MESH_EVENT_HANDLER_INVALID');
        const token = Symbol(), entries = handlers.get(event) ?? new Map();
        entries.set(token, handler as MeshEventHandler); handlers.set(event, entries); attach();
        if (visible()) schedule(0); else status({ type: 'paused', reason: pageHidden ? 'pagehide' : 'hidden' });
        return () => { if (!entries.delete(token)) return; if (!entries.size && handlers.get(event) === entries) handlers.delete(event); if (!count() && !closed) { stop(); detach(); if (!halted) status({ type: 'idle' }); } };
      },
      onStatus(handler) {
        if (closed) throw Error('MESH_EVENT_CLOSED');
        if (typeof handler !== 'function') throw Error('MESH_EVENT_HANDLER_INVALID');
        const token = Symbol(); statuses.set(token, handler); notify(handler, lastStatus); return () => { statuses.delete(token); };
      },
      async send(peer, event, payload) {
        if (closed) throw Error('MESH_EVENT_CLOSED');
        const params = { peerId: peerId(peer), namespace: name, event: eventName(event), payload: jsonPayload(payload) };
        const result = record(await transport({ method: 'mesh.send', params }));
        if (result.status !== 'received' || typeof result.messageId !== 'string' || !/^[a-fA-F0-9]{32}$/.test(result.messageId) || /[^a-fA-F0-9]/.test(result.messageId)) throw Error('MESH_EVENT_RESPONSE_INVALID');
        return { messageId: result.messageId, status: 'received' };
      },
      close() {
        if (closed) return;
        closed = true; stop(); detach(); handlers.clear(); status({ type: 'closed' }); statuses.clear();
        if (namespaces.get(name) === channel) namespaces.delete(name);
      },
    });
    namespaces.set(name, channel); return channel;
  }
  return Object.freeze({
    namespace,
    peers: () => transport({ method: 'mesh.peers', params: {} }) as Promise<MeshPeers>,
  });
}

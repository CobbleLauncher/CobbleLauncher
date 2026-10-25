// SPDX-License-Identifier: MIT
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { rankUsedTools, recordToolOpen } from './tool-usage';
const key = 'cobble.tools.usage.v1';
const now = Date.UTC(2026, 9, 23);
const day = 24 * 60 * 60 * 1000;
const tool = (pluginId: string, commandId = 'open') => ({ pluginId, commandId, label: pluginId });
beforeEach(() => localStorage.clear());
afterEach(() => vi.restoreAllMocks());
describe('local tool use preferences', () => {
  it('coalesces repeated command uses while keeping separate commands and plugin identities', () => {
    recordToolOpen(tool('viewer'), now - 2); recordToolOpen(tool('viewer'), now - 1);
    recordToolOpen(tool('viewer', 'inspect'), now); recordToolOpen(tool('other'), now);
    const saved = JSON.parse(localStorage.getItem(key)!);
    expect(saved.entries).toHaveLength(3);
    expect(saved.entries.find((entry: { pluginId: string; commandId: string }) => entry.pluginId === 'viewer' && entry.commandId === 'open').count).toBe(2);
    expect(rankUsedTools([tool('other'), tool('viewer')], now)[0]!.pluginId).toBe('viewer');
  });
  it('uses both frequency and recency without preserving stale popularity forever', () => {
    for (let index = 0; index < 50; index++) recordToolOpen(tool('old-favorite'), now - 120 * day + index);
    recordToolOpen(tool('recent'), now);
    expect(rankUsedTools([tool('old-favorite'), tool('recent')], now)[0]!.pluginId).toBe('recent');
    for (let index = 0; index < 5; index++) recordToolOpen(tool('frequent'), now - day + index);
    expect(rankUsedTools([tool('recent'), tool('frequent')], now)[0]!.pluginId).toBe('frequent');
  });
  it('returns only eligible tools, deduplicates commands, and preserves stable unused order', () => {
    recordToolOpen(tool('disabled'), now); recordToolOpen(tool('removed'), now); recordToolOpen(tool('incompatible'), now);
    const first = tool('first'), second = tool('second'), input = [first, second, first];
    expect(rankUsedTools(input, now)).toEqual([first, second]);
    expect(input).toEqual([first, second, first]);
    recordToolOpen(first, now); recordToolOpen(second, now);
    expect(rankUsedTools([second, first], now)).toEqual([second, first]);
  });
  it('bounds saved history and persists no labels or game/profile context', () => {
    for (let index = 0; index < 100; index++) {
      const contextualCommand = { ...tool(`plugin-${index}`), label: 'private label', instanceId: 'game', accountId: 'account' };
      recordToolOpen(contextualCommand, now + index);
    }
    const raw = localStorage.getItem(key)!, entries = JSON.parse(raw).entries;
    expect(entries).toHaveLength(64); expect(entries[0].pluginId).toBe('plugin-99');
    expect(entries.some((entry: { pluginId: string }) => entry.pluginId === 'plugin-0')).toBe(false);
    expect(raw).not.toMatch(/private label|instanceId|accountId|account|game/);
    expect(Object.keys(entries[0]).sort()).toEqual(['commandId', 'count', 'lastOpenedAt', 'pluginId']);
  });
  it('ignores malformed storage, unknown schemas, invalid entries and far-future timestamps', () => {
    const eligible = [tool('first'), tool('second')];
    for (const malformed of ['broken json', 'null', '[]', '{"schemaVersion":2,"entries":[]}', 'x'.repeat(40_000)]) {
      localStorage.setItem(key, malformed); expect(rankUsedTools(eligible, now)).toEqual(eligible);
    }
    localStorage.setItem(key, JSON.stringify({ schemaVersion: 1, entries: [null, { pluginId: 'second', commandId: 'open', count: -1, lastOpenedAt: now }, { pluginId: 'second', commandId: 'open', count: 99, lastOpenedAt: now + day }] }));
    expect(rankUsedTools(eligible, now)).toEqual(eligible);
    recordToolOpen(eligible[0]!, now); expect(JSON.parse(localStorage.getItem(key)!).entries).toHaveLength(1);
  });
  it('normalizes duplicate persisted entries without summing forged counts and handles key delimiters', () => {
    localStorage.setItem(key, JSON.stringify({ schemaVersion: 1, entries: [
      { pluginId: 'a', commandId: 'open', count: 2, lastOpenedAt: now },
      { pluginId: 'a', commandId: 'open', count: 1, lastOpenedAt: now - day },
    ] }));
    recordToolOpen(tool('a'), now); expect(JSON.parse(localStorage.getItem(key)!).entries[0].count).toBe(3);
    recordToolOpen(tool('a:b', 'c'), now); recordToolOpen(tool('a', 'b:c'), now);
    expect(JSON.parse(localStorage.getItem(key)!).entries).toHaveLength(3);
  });
  it('caps frequency counters and rejects path-like identifiers without writing them', () => {
    localStorage.setItem(key, JSON.stringify({ schemaVersion: 1, entries: [{ pluginId: 'safe', commandId: 'open', count: 1_000_000, lastOpenedAt: now }] }));
    recordToolOpen(tool('safe'), now); expect(JSON.parse(localStorage.getItem(key)!).entries[0].count).toBe(10_000);
    const before = localStorage.getItem(key); recordToolOpen(tool('C:/private/account'), now); expect(localStorage.getItem(key)).toBe(before);
  });
  it('never blocks browsing or opening when local storage is unavailable', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('blocked'); });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('full'); });
    const eligible = [tool('one'), tool('two')];
    expect(() => recordToolOpen(eligible[0]!, now)).not.toThrow(); expect(rankUsedTools(eligible, now)).toEqual(eligible);
  });
});

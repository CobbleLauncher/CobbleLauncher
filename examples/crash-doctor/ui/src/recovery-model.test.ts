// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import type { RecoverySnapshot } from '../../../../packages/plugin-web-sdk/src/recovery-workspace';
import { recoveryEvents } from './recovery-model.ts';
const runtime = { javaMode: 'automatic' as const, selectedJavaMajor: 21, memoryMb: 2048, jvmArgumentCount: 0 };
const fixture = (): RecoverySnapshot => ({ schemaVersion: 2, gameId: 'game', revision: 'a'.repeat(64), baselineAvailable: true, lastCleanLaunch: { startedAt: 100, endedAt: 200, runtime }, runtime: { ...runtime, changedSinceClean: false }, launchState: 'idle', mods: { available: true, truncated: false, items: [{ id: 'new', createdAt: 300, status: 'applied', files: [{ name: 'Mod new', before: 'a.jar', after: 'b.jar' }] }, { id: 'old', createdAt: 50, status: 'undone', files: [{ name: 'Mod old', before: null, after: 'c.jar' }] }] }, configs: { available: true, truncated: false, files: [{ id: 'c'.repeat(64), name: 'example.toml', backups: [{ id: 'backup', createdAt: 150 }] }] }, comparison: { available: false, reason: 'noFailedAttempt', baselineEndedAt: 200, failureAt: null, runtime: { changed: null, before: null, after: null }, loader: { changed: null, before: null, after: null }, modOperations: [], configBackups: [], limited: false } });
test('change chronology uses recorded process start and never labels a change as the cause', () => {
  const result = recoveryEvents(fixture(), true, '', 'all');
  assert.deepEqual(result.items.map(item => item.key.startsWith('mod-') ? item.key : 'config'), ['mod-new', 'config']);
  assert.equal(result.items[0].at, 300); assert.equal(result.items[1].at, 150);
  assert.deepEqual(result.items[0].target, { kind: 'modUndo', operationId: 'new' });
  assert(!JSON.stringify(result).includes('cause'));
});
test('unknown baselines preserve history and filters/search do not invent restore actions', () => {
  const source = fixture(); source.lastCleanLaunch = null;
  assert.equal(recoveryEvents(source, true, '', 'all').total, 3);
  assert.equal(recoveryEvents(source, false, 'example', 'config').items[0].name, 'example.toml');
  assert.equal(recoveryEvents(source, false, 'old', 'mod').items[0].target, null);
  assert.equal(recoveryEvents(source, false, 'not found', 'all').total, 0);
});
test('visible event lists stay bounded with an explicit limit signal', () => {
  const source = fixture(); source.mods.items = Array.from({ length: 200 }, (_, index) => ({ ...source.mods.items[0], id: 'operation-' + index }));
  const result = recoveryEvents(source, false, '', 'all'); assert.equal(result.items.length, 100); assert.equal(result.total, 201); assert.equal(result.limited, true);
});
test('failed-attempt cutoff separates later records from comparison interval', () => {
  const source = fixture(); source.mods.items.push({ id: 'later', createdAt: 500, status: 'applied', files: [{ name: 'Later', before: null, after: 'later.jar' }] });
  assert.deepEqual(recoveryEvents(source, true, '', 'all', 400).items.map(item => item.key), ['mod-later']);
  assert.equal(recoveryEvents(source, false, '', 'all', 400).total, 4);
});

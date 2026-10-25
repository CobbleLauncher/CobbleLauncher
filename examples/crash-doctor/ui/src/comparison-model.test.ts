// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import type { RecoveryComparison } from '../../../../packages/plugin-web-sdk/src/recovery-workspace';
import { comparisonChanges, comparisonRecords } from './comparison-model.ts';

const fixture = (): RecoveryComparison => ({
  available: true, reason: null, baselineEndedAt: 100, failureAt: 400,
  runtime: { changed: true, before: { javaMode: 'automatic', selectedJavaMajor: 17, memoryMb: 2048, jvmArgumentCount: 0 }, after: { javaMode: 'custom', selectedJavaMajor: 21, memoryMb: 4096, jvmArgumentCount: 1 } },
  loader: { changed: true, before: { minecraftVersion: '1.20.1', kind: 'fabric', version: '0.15' }, after: { minecraftVersion: '1.20.1', kind: 'fabric', version: '0.16' } },
  modOperations: [{ id: 'operation', createdAt: 250, status: 'undone', statusAtFailure: 'unknown', files: [{ name: 'Example Mod', before: 'old.jar', after: 'new.jar' }] }],
  configBackups: [{ configId: 'a'.repeat(64), name: 'options.txt', backupId: 'backup', createdAt: 300 }], limited: false,
});
test('comparison uses captured launch settings and shows only changed fields', () => {
  const rows = comparisonChanges(fixture());
  assert.deepEqual(rows.map(row => row.field), ['loaderVersion', 'javaMode', 'javaMajor', 'memoryMb', 'jvmArgumentCount']);
  assert.equal(rows[0].before, '0.15'); assert.equal(rows[0].after, '0.16');
});
test('unavailable or uncaptured evidence never invents a settings delta', () => {
  const source = fixture(); source.available = false; assert.deepEqual(comparisonChanges(source), []); assert.deepEqual(comparisonRecords(source), []);
  source.available = true; source.runtime.before = null; source.loader.changed = null; assert.deepEqual(comparisonChanges(source), []);
});
test('managed records stay dated and undone mods have no undo action', () => {
  const records = comparisonRecords(fixture());
  assert.deepEqual(records.map(record => [record.kind, record.at]), [['mod', 250], ['config', 300]]);
  assert.equal(records[0].target, null);
  assert.deepEqual(records[1].target, { kind: 'configRestore', configId: 'a'.repeat(64), backupId: 'backup' });
});

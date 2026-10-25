// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createLangPackage } from './lang-package.ts';
test('language package calls use only native-issued session tokens and selected document IDs', async () => {
  const calls: unknown[] = []; const client = createLangPackage(async request => { calls.push(request); return null; });
  const selectionId = 'a'.repeat(64);
  await client.pick(); await client.check({ selectionId, namespace: 'example', baselineLocale: 'en_us', targetLocale: 'zh_cn' });
  await client.document({ selectionId, fileId: 'file-1' }); await client.cancel();
  assert.deepEqual(calls, [
    { method: 'langPackage.pick', params: {} },
    { method: 'langPackage.check', params: { selectionId, namespace: 'example', baselineLocale: 'en_us', targetLocale: 'zh_cn' } },
    { method: 'langPackage.document', params: { selectionId, fileId: 'file-1' } }, { method: 'langPackage.cancel', params: {} },
  ]);
  assert.throws(() => client.document({ selectionId, fileId: '../private' }));
  assert.throws(() => client.check({ selectionId: 'path', namespace: null, baselineLocale: null, targetLocale: 'en_us' }));
  assert.equal(calls.length, 4);
});

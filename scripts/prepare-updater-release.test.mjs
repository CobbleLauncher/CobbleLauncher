// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { installerConfiguration, updateManifest } from './prepare-updater-release.mjs';
test('updater preparation rejects incomplete staging and unsigned or non-HTTPS releases', () => {
  assert.throws(() => installerConfiguration('relative', 'public-key'));
  assert.throws(() => installerConfiguration(process.cwd(), 'public-key'), /Incomplete staging/);
  assert.throws(() => updateManifest('1.0.0', 'http://example.com/setup.exe', 'signature'));
  assert.throws(() => updateManifest('1.0.0', 'https://example.com/setup.exe', ''));
  assert.throws(() => updateManifest('latest', 'https://example.com/setup.exe', 'signature'));
  assert.deepEqual(updateManifest('1.2.3', 'https://example.com/setup.exe', 'signature').platforms['windows-x86_64'], { url: 'https://example.com/setup.exe', signature: 'signature' });
});

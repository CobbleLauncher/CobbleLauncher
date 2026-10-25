// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { redactLog } from '../../../../packages/plugin-web-sdk/src/redaction.ts';
import { logPreview, sharingLimits } from './redaction-model.ts';
test('changes and loaded toggles keep exact line numbers and search their actual displayed text', () => {
  const loaded = 'Useful line\r\nuserId=PrivateName\r\nUseful end', result = redactLog(loaded);
  assert.deepEqual(logPreview(loaded, result, 'changes', '').lines.map(line => line.number), [2]);
  assert.equal(logPreview(loaded, result, 'redacted', 'PrivateName').lines.length, 0);
  assert.equal(logPreview(loaded, result, 'loaded', 'PrivateName').lines[0].text, 'userId=PrivateName');
  const row = logPreview(loaded, result, 'changes', '').lines[0]; assert.equal(row.text.slice(row.marks[0].start, row.marks[0].end), '[ACCOUNT]');
});
test('preview and clipboard are explicitly bounded while exports retain full redacted output', () => {
  const loaded = 'userId=private\n'.repeat(1000), result = redactLog(loaded);
  assert.equal(logPreview(loaded, result, 'redacted', '').lines.length, 400); assert.equal(logPreview(loaded, result, 'redacted', '').limited, true);
  assert.deepEqual(sharingLimits(result.text), { copy: false, export: true }); assert.equal(result.text.split('\n').length, 1001);
  assert.equal(sharingLimits('界'.repeat(2731)).copy, false); assert.equal(sharingLimits('abc\0').copy, false);
  assert.equal(sharingLimits('x'.repeat(1_048_577)).export, false);
});

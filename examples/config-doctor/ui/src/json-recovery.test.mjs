// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { jsonFieldChanges, restoreJsonFields } from './json-recovery.ts';

test('compares nested JSON values with escaped field paths and restores selected fields only', () => {
  const before = '{\r\n  "video": {\r\n    "quality": "high",\r\n    "width": 1920\r\n  },\r\n  "old/key": true\r\n}\r\n';
  const current = '{\r\n  "video": {\r\n    "quality": "low",\r\n    "width": 1280\r\n  },\r\n  "new": true\r\n}\r\n';
  const changes = jsonFieldChanges(current, before);
  assert.deepEqual(changes?.map(change => change.path), ['/new', '/old~1key', '/video/quality', '/video/width']);
  const result = JSON.parse(restoreJsonFields(current, before, ['/video/quality', '/old~1key']));
  assert.equal(result.video.quality, 'high');
  assert.equal(result.video.width, 1280);
  assert.equal(result['old/key'], true);
  assert.equal(result.new, true);
});

test('handles added/removed keys and declines field-level recovery for invalid or nonobject JSON', () => {
  const changes = jsonFieldChanges('{"kept":1,"removed":2}', '{"kept":1,"added":3}');
  assert.deepEqual(changes?.map(change => [change.path, change.beforeExists, change.currentExists]), [['/added', true, false], ['/removed', false, true]]);
  assert.equal(jsonFieldChanges('{ invalid', '{}'), null);
  assert.equal(jsonFieldChanges('[1]', '[2]'), null);
});

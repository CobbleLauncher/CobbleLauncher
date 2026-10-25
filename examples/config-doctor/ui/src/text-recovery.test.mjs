// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { lineChanges } from './text-recovery.ts';

test('shows inserted and removed lines while retaining unchanged context',()=>{
  assert.deepEqual(lineChanges('a\nb\nc','a\nc\nd'),[
    {kind:'removed',line:2,text:'b'},
    {kind:'added',line:3,text:'d'},
  ]);
});

test('falls back to full preview for large files',()=>{
  assert.equal(lineChanges('x\n'.repeat(1001),'y'),null);
});

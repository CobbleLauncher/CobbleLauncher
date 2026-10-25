// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { bytes, percent, problem } from './model.ts';
test('disk numbers distinguish missing data and clamp task progress',()=>{
 assert.equal(bytes(null),'Unavailable');assert.equal(bytes(1024),'1 KiB');assert.equal(percent(11,10),100);assert.equal(percent(1,0),0);
});
test('failed migration never suggests deleting or switching the original',()=>{
 assert.match(problem('STORAGE_SOURCE_CHANGED',false),/original files changed/);
 assert.match(problem('STORAGE_INDEX_FAILED',false),/original game is kept/);
 assert.match(problem('STORAGE_RECYCLE_UNAVAILABLE',true),/仍保留/);
});

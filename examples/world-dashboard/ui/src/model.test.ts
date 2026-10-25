// SPDX-License-Identifier: MIT
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {orderedPair,toggleSnapshot,pageRange,size,worldRows,snapshotTime} from './model.ts';
test('comparison direction is chronological regardless of timeline selection order',()=>{const snapshots=[{id:'new',created_at:20},{id:'old',created_at:10}];assert.deepEqual(orderedPair(snapshots,['new','old'])?.map(row=>row.id),['old','new']);assert.equal(orderedPair(snapshots,['new']),null);});
test('selection never silently replaces either of two snapshots',()=>{assert.deepEqual(toggleSnapshot(['a','b'],'c'),['a','b']);assert.deepEqual(toggleSnapshot(['a','b'],'a'),['b']);});
test('filtered pagination and unknown sizes remain explicit',()=>{assert.equal(pageRange(50,3,53),'51–53 / 53');assert.equal(pageRange(0,0,0),'0');assert.equal(size(null),'—');assert.equal(size(1024),'1.0 KiB');assert.deepEqual(worldRows({items:[{id:'a',name:'World'},{name:'bad'}]}),[{id:'a',name:'World'}]);});
test('ISO timestamps compare actual instants across timezone offsets',()=>{assert.deepEqual(orderedPair([{id:'later',created_at:'2026-10-23T02:00:00Z'},{id:'earlier',created_at:'2026-10-23T09:00:00+08:00'}],['later','earlier'])?.map(row=>row.id),['earlier','later']);});
test('native jiff zone annotations retain chronological comparison direction',()=>{const before='2026-10-23T18:10:15.8851613+08:00[+08:00]',after='2026-10-23T18:11:15.8851613+08:00[+08:00]';assert.equal(snapshotTime(before),Date.parse('2026-10-23T10:10:15.885Z'));assert.deepEqual(orderedPair([{id:'a-after',created_at:after},{id:'z-before',created_at:before}],['a-after','z-before'])?.map(row=>row.id),['z-before','a-after']);});

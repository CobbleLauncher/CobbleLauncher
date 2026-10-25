// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { restoreSelection } from './recovery.ts';

const id='a'.repeat(64),revision='c'.repeat(64),backupId=`1761234567890-${'b'.repeat(64)}`;
const current={id,name:'options.json',revision,text:'{"video":{"quality":"low","width":1280},"keep":true}',backups:[]};
const saved={id:backupId,text:'{"video":{"quality":"high","width":1920},"old":true}'};

test('selected JSON fields use the current revision and preserve other fields',async()=>{
  let call;
  const client={write:async(...args)=>(call=args,{id,revision:'d'.repeat(64)}),restore:async()=>{throw new Error('unexpected restore');}};
  await restoreSelection(client,'game-a',current,saved,['/video/quality']);
  assert.equal(call[2],revision);
  const output=JSON.parse(call[3]);
  assert.equal(output.video.quality,'high');assert.equal(output.video.width,1280);assert.equal(output.keep,true);
});

test('whole-file restore is explicit, cancellation and external changes leave the request uncommitted',async()=>{
  let calls=0;
  const client={write:async()=>{calls++;},restore:async(...args)=>{calls++;assert.deepEqual(args,['game-a',id,backupId,revision]);return {};}};
  await restoreSelection(client,'game-a',current,saved,null);assert.equal(calls,1);
  for(const code of ['PLUGIN_ACTION_CANCELLED','CONFIG_CHANGED']){
    const rejecting={write:async()=>{throw new Error(code);},restore:async()=>{throw new Error(code);}};
    await assert.rejects(restoreSelection(rejecting,'game-a',current,saved,['/video/quality']),new RegExp(code));
  }
});

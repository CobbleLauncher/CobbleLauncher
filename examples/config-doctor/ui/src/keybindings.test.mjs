// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bindingLabel, keyLabel, defaultSource } from './keybindings.ts';
test('binding display preserves unknown bindings while translating known actions',()=>{
  assert.equal(bindingLabel('key_key.forward'),'Forward');
  assert.equal(bindingLabel('key_key.jump',true),'跳跃');
  assert.equal(bindingLabel('key_mod.customAction'),'mod custom Action');
  assert.equal(keyLabel('key.keyboard.left.shift'),'LEFT SHIFT');
  assert.equal(keyLabel('key.mouse.left'),'Mouse 1');
  assert.equal(keyLabel('key.keyboard.unknown'),'—');
});
test('source follows context then recent launch and excludes explicit target',()=>{
  const games=[{id:'a',name:'A',version:'1.21'},{id:'b',name:'B',version:'1.20'},{id:'new',name:'New',version:'1.21'}];
  const context={locale:'en',theme:'light',instanceId:null,games,keybindingTargetId:'new',recentGames:[{...games[0],lastLaunchedAt:2},{...games[1],lastLaunchedAt:8},{...games[2],lastLaunchedAt:10}]};
  assert.equal(defaultSource(context),'b');
  assert.equal(defaultSource({...context,instanceId:'a'}),'a');
  assert.equal(defaultSource({...context,keybindingGames:[games[2]]}),'');
});

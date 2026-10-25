// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { displayName } from './names.ts';

test('uses localized labels without changing namespaced engine identifiers', () => {
  assert.equal(displayName('plains', 'zh-CN', 'biome'), '平原');
  assert.equal(displayName('minecraft:trial_chambers', 'zh-CN', 'structure'), '试炼密室');
  assert.equal(displayName('minecraft:trial_chambers', 'en', 'structure'), 'Trial Chambers');
});

test('humanizes unknown identifiers without inventing translations', () => {
  assert.equal(displayName('minecraft:future_biome', 'zh-CN', 'biome'), 'Future Biome');
});

test('keeps historical names distinct from modern renamed biomes',()=>{
  assert.equal(displayName('mountains','en','biome'),'Mountains');
  assert.equal(displayName('mountains','zh-CN','biome'),'山地');
  assert.equal(displayName('windswept_hills','zh-CN','biome'),'风袭丘陵');
  assert.equal(displayName('snowy_mountains','zh-CN','biome'),'雪山');
  assert.equal(displayName('desert_hills','zh-CN','biome'),'沙漠丘陵');
  assert.equal(displayName('modified_jungle','zh-CN','biome'),'变种丛林');
});

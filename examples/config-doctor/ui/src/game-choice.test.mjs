// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { JSDOM } from '../../../../apps/desktop/node_modules/jsdom/lib/api.js';
import { populateGames } from './game-choice.ts';

test('standalone selector opens an available game while bound context auto-selects only itself', () => {
  const dom = new JSDOM('<select></select>');
  try {
    const select = dom.window.document.querySelector('select');
    const games = [{id:'first',name:'First',version:'1.21.1'},{id:'bound',name:'Bound',version:'26.3'}];
    populateGames(select, {games,instanceId:null}, 'Choose game');
    assert.equal(select.value, 'first');
    populateGames(select, {games,instanceId:null,recentGames:[{id:'missing'},{id:'bound'}]}, 'Choose game');
    assert.equal(select.value, 'bound');
    select.value = 'first';
    assert.equal(select.value, 'first');
    populateGames(select, {games,instanceId:'bound'}, '选择游戏');
    assert.equal(select.value, 'bound');
    assert.equal(select.options.length, 1);
    assert.equal(select.options[0].textContent, 'Bound · 26.3');
    populateGames(select, {games,instanceId:'missing'}, 'Choose game');
    assert.equal(select.value, '');
  } finally { dom.window.close(); }
});

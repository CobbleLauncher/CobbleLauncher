// SPDX-License-Identifier: MIT
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { isMarketSaved, savedMarketItems, savedMarketUndo, toggleMarketSaved, undoMarketRemoval } from './market-saved';
import type { ModSearchHit } from './games';

const hit: ModSearchHit = { project_id: 'same-id', slug: 'one', title: 'One', description: '', author: '', downloads: 0 };
beforeEach(() => { localStorage.clear(); savedMarketItems.value = []; savedMarketUndo.value = null; });
it('undoes only the removed collection entry and persists the result', () => {
  toggleMarketSaved('modrinth', 'mod', hit, 'favorite');
  toggleMarketSaved('modrinth', 'mod', hit, 'later');
  toggleMarketSaved('modrinth', 'mod', hit, 'favorite');
  expect(savedMarketUndo.value?.item.collection).toBe('favorite');
  undoMarketRemoval();
  expect(isMarketSaved('modrinth', 'mod', hit.project_id, 'favorite')).toBe(true);
  expect(isMarketSaved('modrinth', 'mod', hit.project_id, 'later')).toBe(true);
  expect(savedMarketUndo.value).toBeNull();
  expect(JSON.parse(localStorage.getItem('cobble.market.saved.v2')!)).toHaveLength(2);
});
describe('market saved items', () => {
  it('scopes identical project IDs by provider and kind and never starts an install', () => {
    toggleMarketSaved('modrinth', 'mod', hit);
    toggleMarketSaved('modrinth', 'mod', hit, 'favorite');
    toggleMarketSaved('modrinth', 'modpack', hit);
    toggleMarketSaved('curseforge', 'mod', hit);
    expect(savedMarketItems.value).toHaveLength(4);
    expect(isMarketSaved('modrinth', 'mod', hit.project_id)).toBe(true);
    toggleMarketSaved('modrinth', 'mod', hit);
    expect(savedMarketItems.value).toHaveLength(3);
    expect(isMarketSaved('modrinth', 'mod', hit.project_id, 'favorite')).toBe(true);
    expect(isMarketSaved('modrinth', 'modpack', hit.project_id)).toBe(true);
    expect(isMarketSaved('curseforge', 'mod', hit.project_id)).toBe(true);
    expect(JSON.parse(localStorage.getItem('cobble.market.saved.v2') ?? '[]')).toHaveLength(3);
  });
  it('restores separate favorite and later collections after module reload', async () => {
    toggleMarketSaved('modrinth', 'mod', hit, 'favorite');
    toggleMarketSaved('modrinth', 'mod', hit, 'later');
    vi.resetModules();
    const restored = await import('./market-saved');
    expect(restored.isMarketSaved('modrinth', 'mod', hit.project_id, 'favorite')).toBe(true);
    expect(restored.isMarketSaved('modrinth', 'mod', hit.project_id, 'later')).toBe(true);
  });
});

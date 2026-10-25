// SPDX-License-Identifier: MIT
import { ref } from 'vue';
import type { MarketKind } from './market';
import type { ModSearchHit } from './games';
import type { CurseForgeHit } from './curseforge';

export type SavedMarketItem = {
  collection: 'favorite' | 'later';
  provider: 'modrinth' | 'curseforge';
  kind: Exclude<MarketKind, 'blueprint'>;
  project_id: string;
  title: string;
  hit: ModSearchHit | CurseForgeHit;
};
const key = 'cobble.market.saved.v2';
function read(): SavedMarketItem[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? '[]');
    if (!Array.isArray(value)) return [];
    return value.filter((item): item is SavedMarketItem =>
      !!item && typeof item === 'object' &&
      (item.provider === 'modrinth' || item.provider === 'curseforge') &&
      (item.collection === 'favorite' || item.collection === 'later') &&
      ['mod', 'modpack', 'resource_pack', 'shader_pack'].includes(item.kind) &&
      typeof item.project_id === 'string' && item.project_id.length > 0 && item.project_id.length < 128 &&
      typeof item.title === 'string' && item.hit?.project_id === item.project_id
    );
  } catch { return []; }
}
export const savedMarketItems = ref<SavedMarketItem[]>(read());
export function isMarketSaved(provider: SavedMarketItem['provider'], kind: SavedMarketItem['kind'], projectId: string, collection: SavedMarketItem['collection'] = 'later') {
  return savedMarketItems.value.some(item => item.provider === provider && item.kind === kind && item.project_id === projectId && item.collection === collection);
}
export function toggleMarketSaved(provider: SavedMarketItem['provider'], kind: SavedMarketItem['kind'], hit: ModSearchHit | CurseForgeHit, collection: SavedMarketItem['collection'] = 'later') {
  const previous = savedMarketItems.value;
  const retained = previous.filter(item => !(item.provider === provider && item.kind === kind && item.project_id === hit.project_id && item.collection === collection));
  savedMarketItems.value = retained.length === previous.length
    ? [{ collection, provider, kind, project_id: hit.project_id, title: hit.title, hit }, ...retained]
    : retained;
  try { localStorage.setItem(key, JSON.stringify(savedMarketItems.value)); }
  catch { savedMarketItems.value = previous; }
}

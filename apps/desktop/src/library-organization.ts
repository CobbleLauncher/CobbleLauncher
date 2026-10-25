// SPDX-License-Identifier: MIT
import type { HomeGame } from './library-home';

export type GameOrganization = { group?: string; tags: string[]; archived: boolean };
export type OrganizationMap = Record<string, GameOrganization>;
const key = 'cobble.library.organization.v1';
const label = (value: unknown) => typeof value === 'string' ? value.trim().slice(0, 40) : '';
const gameId = (value: unknown) => typeof value === 'string' && /^[a-zA-Z0-9_-]{1,100}$/.test(value);

export function readLibraryOrganization(): OrganizationMap {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(key) ?? '{}');
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).slice(0, 1000).filter(([id]) => gameId(id)).map(([id, raw]) => {
      const value = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw as Record<string, unknown> : {};
      const group = label(value.group);
      const tags = Array.isArray(value.tags) ? [...new Set(value.tags.map(label).filter(Boolean))].slice(0, 12) : [];
      return [id, { ...(group ? { group } : {}), tags, archived: value.archived === true }];
    }));
  } catch { return {}; }
}

export function writeLibraryOrganization(value: OrganizationMap): void {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* Changes remain in this session. */ }
}

export function updateGameOrganization(map: OrganizationMap, id: string, changes: Partial<GameOrganization>): OrganizationMap {
  if (!gameId(id)) return map;
  const previous = map[id] ?? { tags: [], archived: false };
  const group = label(changes.group === undefined ? previous.group : changes.group);
  const tags = Array.isArray(changes.tags) ? [...new Set(changes.tags.map(label).filter(Boolean))].slice(0, 12) : previous.tags;
  return { ...map, [id]: { ...(group ? { group } : {}), tags, archived: changes.archived === undefined ? previous.archived : changes.archived } };
}

export function organizationOptions(games: readonly HomeGame[], map: OrganizationMap) {
  const entries = games.map(game => map[game.id]).filter(Boolean);
  return {
    groups: [...new Set(entries.map(entry => entry?.group).filter((value): value is string => !!value))].sort(),
    tags: [...new Set(entries.flatMap(entry => entry?.tags ?? []))].sort(),
  };
}

export function filterOrganizedGames(games: readonly HomeGame[], map: OrganizationMap, options: { archived?: boolean; group?: string; tags?: string[] } = {}): HomeGame[] {
  return games.filter(game => {
    const value = map[game.id];
    return Boolean(value?.archived) === Boolean(options.archived) &&
      (!options.group || value?.group === options.group) &&
      (!options.tags?.length || options.tags.every(tag => value?.tags.includes(tag)));
  });
}

export function searchOrganization(game: HomeGame, map: OrganizationMap, query: string): boolean {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return true;
  const value = map[game.id];
  return [value?.group, ...(value?.tags ?? [])].some(label => label?.toLocaleLowerCase().includes(needle));
}

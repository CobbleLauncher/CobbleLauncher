// SPDX-License-Identifier: MIT
import { beforeEach, expect, it } from 'vitest';
import { filterOrganizedGames, readLibraryOrganization, searchOrganization, updateGameOrganization, writeLibraryOrganization } from './library-organization';
import { defaultHomeLayout, moveHomeSection, readHomeLayout, writeHomeLayout } from './home-layout';
import type { HomeGame } from './library-home';

const games = ['one', 'two'].map(id => ({ id, name: id, status: 'installed', version: '1.21.1' } as HomeGame));
beforeEach(() => localStorage.clear());

it('keeps archived games out of the normal library and requires every selected tag', () => {
  let map = updateGameOrganization({}, 'one', { group: 'Survival', tags: ['Friends', 'Modded'] });
  map = updateGameOrganization(map, 'two', { group: 'Survival', tags: ['Friends'], archived: true });
  writeLibraryOrganization(map);
  const saved = readLibraryOrganization();
  expect(filterOrganizedGames(games, saved).map(game => game.id)).toEqual(['one']);
  expect(filterOrganizedGames(games, saved, { archived: true, group: 'Survival', tags: ['Friends'] }).map(game => game.id)).toEqual(['two']);
  expect(filterOrganizedGames(games, saved, { tags: ['Friends', 'Modded'] }).map(game => game.id)).toEqual(['one']);
  expect(searchOrganization(games[0]!, saved, 'modded')).toBe(true);
});

it('limits stored labels and ignores malformed metadata', () => {
  localStorage.setItem('cobble.library.organization.v1', JSON.stringify({ one: { tags: ['A', 'A', 42], archived: 'yes' }, 'invalid path': { archived: true } }));
  expect(readLibraryOrganization()).toEqual({ one: { tags: ['A'], archived: false } });
  const changed = updateGameOrganization({}, 'one', { tags: Array.from({ length: 20 }, (_, index) => `Tag ${index}`) });
  expect(changed.one?.tags).toHaveLength(12);
});

it('persists section order and preserves unavailable quick action IDs', () => {
  const layout = { ...moveHomeSection(defaultHomeLayout, 'favorites', -1), quickIds: ['navigation.library', 'plugin.missing'] };
  writeHomeLayout(layout);
  expect(readHomeLayout()).toEqual(layout);
  localStorage.setItem('cobble.home.layout.v1', JSON.stringify({ order: ['quick', 'quick', 'other'], hidden: ['favorites', 'bogus'], quickIds: ['valid', 'valid', 'bad/path'] }));
  expect(readHomeLayout()).toEqual({ order: ['quick', 'other', 'favorites'], hidden: ['favorites'], quickIds: ['valid'], libraryView: 'list' });
});

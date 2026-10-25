// SPDX-License-Identifier: MIT
import { describe, expect, it } from 'vitest';
import { marketWorkstate, rememberMarketWorkstate } from './market-workstate';

describe('market workstate', () => {
  it('keeps query, filters and page scoped to the current game in this session', () => {
    rememberMarketWorkstate('modrinth:mod:game-a', { query: 'forest', filter: 'fabric', page: 2 });
    rememberMarketWorkstate('modrinth:mod:game-b', { query: 'stone', filter: 'forge', page: 1 });
    expect(marketWorkstate('modrinth:mod:game-a')).toEqual({ query: 'forest', filter: 'fabric', page: 2 });
    expect(marketWorkstate('modrinth:mod:game-b')).toEqual({ query: 'stone', filter: 'forge', page: 1 });
  });
});

// SPDX-License-Identifier: MIT
import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import MarketPage from './MarketPage.vue';
import { marketGameLocked, marketGames } from './market';
import type { Game } from './games';
const vanilla: Game = { id: 'vanilla', name: 'Vanilla', version: '1.21.1', directory: 'test', status: 'installed', error: null, progress: null };
const fabric: Game = { ...vanilla, id: 'fabric', name: 'Fabric', loader_kind: 'fabric', loader_version: '0.16' };
const stubs = {
  DiscoverMods: { template: '<div data-test="mods" />' },
  CurseForgeDiscover: { template: '<div data-test="curseforge" />' },
  DiscoverPacks: { template: '<div data-test="packs" />' },
  DiscoverPacksPage: true,
};
describe('market', () => {
  it('offers installed compatible targets and blocks mutations while games run', () => {
    const games = [vanilla, fabric, { ...fabric, id: 'pending', status: 'installing' as const }];
    expect(marketGames(games, 'mod')).toEqual([fabric]);
    expect(marketGames(games, 'resource_pack')).toEqual([vanilla, fabric]);
    expect(marketGameLocked({ ...fabric, launch_state: 'running' })).toBe(true);
    expect(marketGameLocked({ ...fabric, content_installing: true })).toBe(true);
    expect(marketGameLocked(fabric)).toBe(false);
  });
  it('carries a game target into the correct content workflow', async () => {
    const wrapper = mount(MarketPage, { props: { games: [vanilla, fabric], initialKind: 'mod', initialGameId: 'fabric' }, global: { stubs } });
    expect((wrapper.get('#market-game-desktop').element as HTMLSelectElement).value).toBe('fabric');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(true);
    await wrapper.setProps({ initialKind: 'resource_pack', initialGameId: 'vanilla' });
    expect((wrapper.get('#market-game-desktop').element as HTMLSelectElement).value).toBe('vanilla');
    expect(wrapper.find('[data-test="packs"]').exists()).toBe(true);
    wrapper.unmount();
  });
  it('keeps provider and target controls in the filter area and switches providers', async () => {
    const wrapper = mount(MarketPage, { props: { games: [vanilla, fabric], initialKind: 'mod', initialGameId: 'fabric' }, global: { stubs } });
    expect((wrapper.get('#market-provider-desktop').element as HTMLSelectElement).value).toBe('modrinth');
    expect((wrapper.get('#market-game-desktop').element as HTMLSelectElement).value).toBe('fabric');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(true);
    await wrapper.get('.market-mobile-filters summary').trigger('click');
    expect((wrapper.get('#market-provider-mobile').element as HTMLSelectElement).value).toBe('modrinth');
    expect((wrapper.get('#market-game-mobile').element as HTMLSelectElement).value).toBe('fabric');
    await wrapper.get('#market-provider-desktop').setValue('curseforge');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(true);
    expect(wrapper.find('.market-mobile-filters').exists()).toBe(true);
    wrapper.unmount();
  });
});

// SPDX-License-Identifier: MIT
import { beforeEach, expect, it } from 'vitest';
import { defineComponent, ref } from 'vue';
import { mount } from '@vue/test-utils';
import MarketPage from './MarketPage.vue';
import DiscoverMods from './DiscoverMods.vue';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import { marketTarget, rememberMarketTarget } from './market-preferences';
import type { Game } from './games';

const vanilla: Game = { id: 'vanilla', name: 'Vanilla', version: '1.21.1', directory: 'test', status: 'installed', error: null, progress: null };
const fabric: Game = { ...vanilla, id: 'fabric', name: 'Fabric', loader_kind: 'fabric', loader_version: '0.16' };
const games = [vanilla, fabric];
const stubs = { DiscoverMods: true, CurseForgeDiscover: true, DiscoverPacks: true, DiscoverPacksPage: true };
beforeEach(() => localStorage.clear());

it('offers the completed installation target without clearing the active browse state on refresh', async () => {
  const wrapper = mount(MarketPage, { props: { games, initialKind: 'mod', initialGameId: 'fabric', initialQuery: 'Litematica' }, global: { stubs } });
  expect(wrapper.findComponent(DiscoverMods).props('initialQuery')).toBe('Litematica');
  wrapper.findComponent(DiscoverMods).vm.$emit('installed');
  await wrapper.setProps({ games: [{ ...fabric, name: 'Updated Fabric' }] });
  await wrapper.get('.setting-hint button').trigger('click');
  expect(wrapper.emitted('locate')).toEqual([['fabric', 'mod']]);
  expect(wrapper.emitted('installed')).toEqual([['fabric']]);
  expect(wrapper.findComponent(DiscoverMods).props('initialQuery')).toBe('Litematica');
  wrapper.unmount();
});

it('remembers explicit targets per content kind and restores live version and loader for both providers', async () => {
  const first = mount(MarketPage, { props: { games, initialKind: 'mod' }, global: { stubs } });
  expect(first.findComponent(DiscoverMods).exists()).toBe(false);
  await first.get('#market-game-desktop').setValue('fabric');
  first.unmount();
  const updated = { ...fabric, version: '1.21.4', loader_version: '0.17' };
  const restored = mount(MarketPage, { props: { games: [vanilla, updated], initialKind: 'mod' }, global: { stubs } });
  expect(restored.findComponent(DiscoverMods).props('game')).toEqual(updated);
  await restored.get('#market-provider-desktop').setValue('curseforge');
  expect(restored.findAllComponents(CurseForgeDiscover).find(component => component.props('kind') === 'mod')?.props('game')).toEqual(updated);
  expect(marketTarget(games, 'mod')).toBe('fabric');
  expect(marketTarget(games, 'resource_pack')).toBe('');
  restored.unmount();
});

it('honors an explicit target and discards removed, incompatible and uninstalled remembered targets', () => {
  rememberMarketTarget('resource_pack', 'fabric');
  expect(marketTarget(games, 'resource_pack', 'vanilla')).toBe('vanilla');
  expect(marketTarget(games, 'resource_pack', '')).toBe('');
  expect(marketTarget([vanilla], 'resource_pack')).toBe('');
  rememberMarketTarget('mod', 'vanilla');
  expect(marketTarget(games, 'mod')).toBe('');
  rememberMarketTarget('mod', 'fabric');
  expect(marketTarget([{ ...fabric, status: 'installing' }], 'mod')).toBe('');
  localStorage.setItem('cobble.market.targets.v1', 'invalid json');
  expect(marketTarget(games, 'mod')).toBe('');
  localStorage.setItem('cobble.market.targets.v1', '{"mod": {"id": "fabric"}}');
  expect(marketTarget(games, 'mod')).toBe('');
});

it('clears a deleted target and never silently switches installation to another game', async () => {
  const wrapper = mount(MarketPage, { props: { games, initialKind: 'mod', initialGameId: 'fabric' }, global: { stubs } });
  await wrapper.setProps({ games: [{ ...fabric, id: 'other', name: 'Other game' }] });
  expect((wrapper.get('#market-game-desktop').element as HTMLSelectElement).value).toBe('');
  expect(wrapper.findComponent(DiscoverMods).exists()).toBe(false);
  wrapper.unmount();
});

it('retains a visited category search when switching providers and categories', async () => {
  const browse = defineComponent({
    setup() { return { query: ref('') }; },
    template: '<input class="retained-search" v-model="query">',
  });
  rememberMarketTarget('mod', 'fabric');
  rememberMarketTarget('resource_pack', 'fabric');
  const wrapper = mount(MarketPage, { props: { games, initialKind: 'mod' }, global: { stubs: { ...stubs, DiscoverMods: browse } } });
  await wrapper.get('.retained-search').setValue('sodium');
  await wrapper.get('#market-provider-desktop').setValue('curseforge');
  await wrapper.get('#market-provider-desktop').setValue('modrinth');
  expect((wrapper.get('.retained-search').element as HTMLInputElement).value).toBe('sodium');
  await wrapper.findAll('.market-category-list button')[2]!.trigger('click');
  await wrapper.findAll('.market-category-list button')[1]!.trigger('click');
  expect((wrapper.get('.retained-search').element as HTMLInputElement).value).toBe('sodium');
  expect((wrapper.get('#market-game-desktop').element as HTMLSelectElement).value).toBe('fabric');
  wrapper.unmount();
});

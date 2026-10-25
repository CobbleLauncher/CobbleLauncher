// SPDX-License-Identifier: MIT
import { beforeEach, expect, it } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import SelectField from './ui/SelectField.vue';
import { rememberMarketWorkstate } from './market-workstate';
import MarketPage from './MarketPage.vue';
import MarketToolbar from './market/MarketToolbar.vue';
import DiscoverMods from './DiscoverMods.vue';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import { marketTarget, rememberMarketTarget } from './market-preferences';
import type { Game } from './games';

const vanilla: Game = { id: 'vanilla', name: 'Vanilla', version: '1.21.1', directory: 'test', status: 'installed', error: null, progress: null };
const fabric: Game = { ...vanilla, id: 'fabric', name: 'Fabric', loader_kind: 'fabric', loader_version: '0.16' };
const games = [vanilla, fabric];
const stubs = { DiscoverMods: true, CurseForgeDiscover: true, DiscoverPacks: true, DiscoverPacksPage: true };
beforeEach(() => { localStorage.clear(); rememberMarketWorkstate('market:selection', undefined); rememberMarketWorkstate('market:shared-query:mod:fabric', undefined); });

it('starts with all available sources even after an earlier single-source browse', () => {
  rememberMarketWorkstate('market:selection', {kind:'mod',provider:'curseforge',gameId:'fabric'});
  const wrapper=mount(MarketPage,{props:{games,initialKind:'mod'},global:{stubs}});
  expect(wrapper.findComponent(MarketToolbar).props('provider')).toBe('all');
  expect(wrapper.findComponent(DiscoverMods).exists()).toBe(true);
  expect(wrapper.findComponent(CurseForgeDiscover).exists()).toBe(true);
  wrapper.unmount();
});

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
  await select(first, 'market-game-desktop', 'fabric');
  first.unmount();
  const updated = { ...fabric, version: '1.21.4', loader_version: '0.17' };
  const restored = mount(MarketPage, { props: { games: [vanilla, updated], initialKind: 'mod' }, global: { stubs } });
  expect(restored.findComponent(DiscoverMods).props('game')).toEqual(updated);
  await select(restored, 'market-provider-desktop', 'curseforge');
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
  expect(field(wrapper, 'market-game-desktop').props('modelValue')).toBe('');
  expect(wrapper.findComponent(DiscoverMods).exists()).toBe(false);
  wrapper.unmount();
});

it('retains the shared search when switching providers and categories', async () => {
  rememberMarketTarget('mod', 'fabric'); rememberMarketTarget('resource_pack', 'fabric');
  const wrapper = mount(MarketPage, { props: { games, initialKind: 'mod' }, global: { stubs } });
  await wrapper.get('#market-search').setValue('sodium'); await wrapper.get('.market-query-row').trigger('submit');
  await select(wrapper, 'market-provider-desktop', 'curseforge');
  expect(wrapper.findComponent(CurseForgeDiscover).props('sharedRequest')).toMatchObject({ query:'sodium' });
  await select(wrapper, 'market-provider-desktop', 'modrinth');
  expect(wrapper.findComponent(DiscoverMods).props('sharedRequest')).toMatchObject({ query:'sodium' });
  await wrapper.findAll('.market-category-list button')[2]!.trigger('click');
  await wrapper.findAll('.market-category-list button')[1]!.trigger('click');
  expect((wrapper.get('#market-search').element as HTMLInputElement).value).toBe('sodium');
  expect(field(wrapper, 'market-game-desktop').props('modelValue')).toBe('fabric');
  wrapper.unmount();
});

function field(wrapper: ReturnType<typeof mount>, id: string) { return wrapper.findAllComponents(SelectField).find(field => field.props('id') === id)!; }
async function select(wrapper: ReturnType<typeof mount>, id: string, value: string) { if (id === 'market-provider-desktop') wrapper.findComponent(MarketToolbar).vm.$emit('provider', value); else field(wrapper, id).vm.$emit('update:modelValue', value); await flushPromises(); }

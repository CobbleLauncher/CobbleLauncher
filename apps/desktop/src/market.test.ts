// SPDX-License-Identifier: MIT
import { beforeEach, describe, expect, it } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import SelectField from './ui/SelectField.vue';
import { rememberMarketWorkstate } from './market-workstate';
import MarketPage from './MarketPage.vue';
import DiscoverPacksPage from './DiscoverPacksPage.vue';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import { marketGameLocked, marketGames } from './market';
import type { Game } from './games';
const vanilla: Game = { id: 'vanilla', name: 'Vanilla', version: '1.21.1', directory: 'test', status: 'installed', error: null, progress: null };
const fabric: Game = { ...vanilla, id: 'fabric', name: 'Fabric', loader_kind: 'fabric', loader_version: '0.16' };
const stubs = {
  DiscoverMods: { template: '<div data-test="mods" />' },
  CurseForgeDiscover: { template: '<div data-test="curseforge" />' },
  DiscoverPacks: { template: '<div data-test="packs" />' },
  DiscoverPacksPage: { template: '<div data-test="modrinth-packs" />' },
};
beforeEach(() => { localStorage.clear(); rememberMarketWorkstate('market:selection', undefined); });
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
    expect(field(wrapper, 'market-game-desktop').props('modelValue')).toBe('fabric');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(true);
    await wrapper.setProps({ initialKind: 'resource_pack', initialGameId: 'vanilla' });
    expect(field(wrapper, 'market-game-desktop').props('modelValue')).toBe('vanilla');
    expect(wrapper.find('[data-test="packs"]').exists()).toBe(true);
    wrapper.unmount();
  });
  it('shows both real provider workflows by default and keeps source choice in filters', async () => {
    const wrapper = mount(MarketPage, { props: { games: [vanilla, fabric], initialKind: 'mod', initialGameId: 'fabric' }, global: { stubs } });
    expect(field(wrapper, 'market-provider-desktop').props('modelValue')).toBe('all');
    expect(field(wrapper, 'market-game-desktop').props('modelValue')).toBe('fabric');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(true);
    expect(wrapper.findAll('.market-source-title').map(node => node.text())).toEqual(['Modrinth', 'CurseForge']);
    await select(wrapper, 'market-provider-desktop', 'curseforge');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(true);
    expect(wrapper.find('.market-advanced-filters').exists()).toBe(true);
    wrapper.unmount();
  });
  it('shows both modpack providers and leaves a single-source choice available', async () => {
    const wrapper = mount(MarketPage, { props: { games: [vanilla, fabric], initialKind: 'modpack', initialQuery: 'adventure' }, global: { stubs } });
    expect(wrapper.find('[data-test="modrinth-packs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(true);
    await select(wrapper, 'market-provider-desktop', 'modrinth');
    expect(wrapper.find('[data-test="modrinth-packs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(false);
    wrapper.unmount();
  });
});

function field(wrapper: ReturnType<typeof mount>, id: string) { return wrapper.findAllComponents(SelectField).find(field => field.props('id') === id)!; }
async function select(wrapper: ReturnType<typeof mount>, id: string, value: string) { field(wrapper, id).vm.$emit('update:modelValue', value); await flushPromises(); }

it('uses one filter request for both sources while keeping installation destination visible', async () => {
  const wrapper = mount(MarketPage, { props: { games:[vanilla,fabric],initialKind:'modpack',initialQuery:'adventure' },global:{stubs:{...stubs,DiscoverPacksPage:true,CurseForgeDiscover:true}} });
  const disclosure = wrapper.get('.market-advanced-filters .settings-disclosure-trigger');
  expect(disclosure.attributes('aria-expanded')).toBe('false');
  await disclosure.trigger('click');
  await wrapper.get('input[placeholder="1.21.1"]').setValue('1.21.1');
  await select(wrapper,'market-filter-loader','fabric');
  await wrapper.get('.market-query-row').trigger('submit');await flushPromises();
  const expected={query:'adventure',minecraftVersion:'1.21.1',loader:'fabric'};
  expect(wrapper.findComponent(DiscoverPacksPage).props('sharedRequest')).toMatchObject(expected);
  expect(wrapper.findComponent(CurseForgeDiscover).props('sharedRequest')).toMatchObject(expected);
  expect(wrapper.get('.market-new-game-target').text()).not.toBe('');
  wrapper.unmount();
});

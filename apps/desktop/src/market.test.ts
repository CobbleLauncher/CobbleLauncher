// SPDX-License-Identifier: MIT
import { beforeEach, describe, expect, it } from 'vitest';
import { DOMWrapper, mount, flushPromises } from '@vue/test-utils';
import SelectField from './ui/SelectField.vue';
import { rememberMarketWorkstate } from './market-workstate';
import MarketPage from './MarketPage.vue';
import MarketToolbar from './market/MarketToolbar.vue';
import MarketVersionFilter from './market/MarketVersionFilter.vue';
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
    expect(wrapper.findComponent(MarketToolbar).props('provider')).toBe('all');
    expect(wrapper.get('.market-source-trigger').attributes('aria-expanded')).toBe('false');
    expect(field(wrapper, 'market-game-desktop').props('modelValue')).toBe('fabric');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(true);
    expect(wrapper.findAll('.market-source-title').map(node => node.text())).toEqual(['Modrinth', 'CurseForge']);
    await selectSource(wrapper, 'CurseForge');
    expect(wrapper.find('[data-test="mods"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(true);
    expect(wrapper.find('.market-source-trigger').exists()).toBe(true);
    wrapper.unmount();
  });
  it('shows both modpack providers and leaves a single-source choice available', async () => {
    const wrapper = mount(MarketPage, { props: { games: [vanilla, fabric], initialKind: 'modpack', initialQuery: 'adventure' }, global: { stubs } });
    expect(wrapper.find('[data-test="modrinth-packs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(true);
    await selectSource(wrapper, 'Modrinth');
    expect(wrapper.find('[data-test="modrinth-packs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="curseforge"]').exists()).toBe(false);
    wrapper.unmount();
  });
});

function field(wrapper: ReturnType<typeof mount>, id: string) { return wrapper.findAllComponents(SelectField).find(field => field.props('id') === id)!; }
async function select(wrapper: ReturnType<typeof mount>, id: string, value: string) { field(wrapper, id).vm.$emit('update:modelValue', value); await flushPromises(); }

it('uses one filter request for both sources while keeping installation destination visible', async () => {
  const wrapper = mount(MarketPage, { props: { games:[vanilla,fabric],initialKind:'modpack',initialQuery:'adventure' },global:{stubs:{...stubs,DiscoverPacksPage:true,CurseForgeDiscover:true}} });
  expect(wrapper.get('.market-version-trigger').isVisible()).toBe(true);
  wrapper.findComponent(MarketVersionFilter).vm.$emit('update:modelValue', '1.21.1');
  await flushPromises();
  await select(wrapper,'market-filter-loader-wide','fabric');
  await wrapper.get('.market-query-row').trigger('submit');await flushPromises();
  const expected={query:'adventure',minecraftVersion:'1.21.1',loader:'fabric'};
  expect(wrapper.findComponent(DiscoverPacksPage).props('sharedRequest')).toMatchObject(expected);
  expect(wrapper.findComponent(CurseForgeDiscover).props('sharedRequest')).toMatchObject(expected);
  expect(wrapper.get('.market-new-game-target').text()).not.toBe('');
  wrapper.unmount();
});

async function selectSource(wrapper: ReturnType<typeof mount>, label: string) {
  await wrapper.get('.market-source-trigger').trigger('click');
  await flushPromises();
  const popup = new DOMWrapper(document.body.querySelector('.market-source-popover')!);
  await popup.findAll('[role=radio]').find(item => item.text() === label)!.trigger('click');
  await flushPromises();
}

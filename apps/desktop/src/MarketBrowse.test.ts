// SPDX-License-Identifier: MIT
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import DiscoverMods from './DiscoverMods.vue';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import type { Game, ModSearchPage } from './games';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));

const game: Game = {
  id: 'browse-fabric', name: 'Fabric 1.21.1', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16',
  directory: 'test', status: 'installed', error: null, progress: null,
};
const fixture = (project_id: string, title: string) => ({
  project_id, slug: project_id.toLowerCase(), title, description: `${title} test listing`, author: 'Cobble fixture', downloads: 12_345,
});

describe('market browsing', () => {
  beforeEach(() => invoke.mockReset());

  it('keeps CurseForge results on summary refresh and inherits the target compatibility', async () => {
    invoke.mockImplementation(async (command: string) => command === 'get_curseforge_availability'
      ? { available: true } : { hits: [], offset: 0, total_hits: 0 });
    const wrapper = mount(CurseForgeDiscover, { props: { kind: 'mod', game, initialQuery: 'Litematica' } });
    await flushPromises();
    expect(invoke).toHaveBeenLastCalledWith('search_curseforge', { query: 'Litematica', kind: 'mod', minecraftVersion: game.version, loader: 'fabric', offset: 0 });
    const calls = invoke.mock.calls.length;
    await wrapper.get('input[type=search]').setValue('another search');
    await wrapper.setProps({ game: { ...game, name: 'Updated game' } });
    await flushPromises();
    expect(invoke).toHaveBeenCalledTimes(calls);
    expect((wrapper.get('input[type=search]').element as HTMLInputElement).value).toBe('another search');
    wrapper.unmount();
  });

  it('searches a requested missing mod and keeps its query when the target summary refreshes', async () => {
    invoke.mockResolvedValue({ hits: [], offset: 0, total_hits: 0 });
    const wrapper = mount(DiscoverMods, { props: { game, locked: false, initialQuery: 'Litematica' } });
    await flushPromises();
    expect(invoke).toHaveBeenLastCalledWith('search_game_mods', { id: game.id, query: 'Litematica', offset: 0 });
    await wrapper.setProps({ game: { ...game, name: 'Updated game' } });
    expect(invoke).toHaveBeenCalledTimes(1);
    await wrapper.setProps({ initialQuery: 'WorldEdit' });
    await flushPromises();
    expect(invoke).toHaveBeenLastCalledWith('search_game_mods', { id: game.id, query: 'WorldEdit', offset: 0 });
    wrapper.unmount();
  });

  it('loads the empty-query listing on entry and appends the next page', async () => {
    invoke.mockImplementation(async (_command: string, args: { offset?: number } = {}): Promise<ModSearchPage> => (args.offset ?? 0) === 0
      ? { hits: [fixture('lambdynamiclights', 'LambDynamicLights'), fixture('sodium', 'Sodium')], offset: 0, total_hits: 3 }
      : { hits: [fixture('iris', 'Iris Shaders')], offset: 2, total_hits: 3 });

    const wrapper = mount(DiscoverMods, { props: { game, locked: false } });
    await flushPromises();

    expect(invoke).toHaveBeenCalledWith('search_game_mods', { id: game.id, query: '', offset: 0 });
    expect(wrapper.findAll('.discover-list .mod-row')).toHaveLength(2);
    expect(wrapper.text()).toContain('LambDynamicLights');
    await wrapper.get('.load-more').trigger('click');
    await flushPromises();

    expect(invoke).toHaveBeenLastCalledWith('search_game_mods', { id: game.id, query: '', offset: 2 });
    expect(wrapper.findAll('.discover-list .mod-row')).toHaveLength(3);
    expect(wrapper.text()).toContain('Iris Shaders');
    wrapper.unmount();
  });
});

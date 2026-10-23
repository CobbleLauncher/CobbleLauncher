// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import DiscoverPacksPage from './DiscoverPacksPage.vue';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));

const modrinthHit = {project_id: 'mr-1', slug: 'pack', title: 'Modrinth Pack', description: 'A pack', author: 'Author', downloads: 10, icon_url: 'https://example.invalid/icon.png'};
const curseForgeHit = {project_id: 'cf-1', slug: 'pack', title: 'CurseForge Pack', description: 'A pack', author: 'Author', downloads: 10, website_url: null, allow_distribution: true, available: true, icon_url: 'https://example.invalid/icon.png'};

beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockReset();
});
afterEach(() => setLocale('zh-CN'));

it('opens a Modrinth pack with a native whole-card button', async () => {
  vi.mocked(invoke).mockImplementation(async (command) => command === 'search_modrinth_packs'
    ? {hits: [modrinthHit], total_hits: 1, offset: 0}
    : []);
  const wrapper = mount(DiscoverPacksPage, {global: {stubs: {MarketImage: true, MarketGallery: true, LoadingIndicator: true}}});
  await flushPromises();

  const card = wrapper.get('.pack-result-card button');
  expect(card.attributes('type')).toBe('button');
  expect(card.classes()).toContain('ui-stretched-action');
  await card.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_modrinth_pack_versions', {projectId: 'mr-1', minecraftVersion: null, loader: null});
  expect(wrapper.text()).toContain('Modrinth Pack');
  wrapper.unmount();
});

it('opens a CurseForge pack with a native whole-card button', async () => {
  vi.mocked(invoke).mockImplementation(async (command) => command === 'get_curseforge_availability'
    ? {available: true}
    : command === 'search_curseforge'
      ? {hits: [curseForgeHit], total_hits: 1, offset: 0}
      : {files: [], total_hits: 0, offset: 0});
  const wrapper = mount(CurseForgeDiscover, {props: {kind: 'modpack'}, global: {stubs: {MarketImage: true, MarketGallery: true, LoadingIndicator: true}}});
  await flushPromises();

  const card = wrapper.get('.pack-result-card button');
  expect(card.attributes('type')).toBe('button');
  expect(card.classes()).toContain('ui-stretched-action');
  await card.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_curseforge_files', {projectId: 'cf-1', minecraftVersion: null, loader: null, offset: 0});
  wrapper.unmount();
});

it('localizes the Modrinth browse controls, card actions and counts', async () => {
  vi.mocked(invoke).mockResolvedValue({ hits: [modrinthHit], total_hits: 2, offset: 0 });
  const wrapper = mount(DiscoverPacksPage, { global: { stubs: { MarketImage: true } } });
  await flushPromises();
  expect(wrapper.get('input[type="search"]').attributes('placeholder')).toBe('Search by name or gameplay');
  expect(wrapper.text()).toContain('Search');
  expect(wrapper.text()).toContain('View');
  expect(wrapper.text()).toContain('10 downloads');
  expect(wrapper.text()).toContain('1 of 2 modpacks');
  expect(wrapper.text()).toContain('Load more');
  expect(wrapper.text()).not.toMatch(/[\u3400-\u9fff]/);
  setLocale('zh-CN'); await flushPromises();
  expect(wrapper.get('input[type="search"]').attributes('placeholder')).toBe('搜索整合包名称或玩法');
  expect(wrapper.text()).toContain('10 次下载');
  expect(wrapper.text()).toContain('已显示 1 / 2 个整合包');
  wrapper.unmount();
});

// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import DiscoverMods from './DiscoverMods.vue';
import { setLocale } from './i18n';
import type { Game, ModSearchPage } from './games';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const game = {
  id: 'fabric', name: 'Fabric', version: '1.21.1', directory: 'test', status: 'installed',
  error: null, progress: null, loader_kind: 'fabric', loader_version: '0.16.0',
} as Game;
const firstPage: ModSearchPage = {
  hits: [{ project_id: 'one', slug: 'one', title: 'First Mod', description: 'Fixture', author: 'Author', downloads: 10 }],
  offset: 0,
  total_hits: 2,
};

beforeEach(() => setLocale('en-US'));
afterEach(() => { vi.clearAllMocks(); setLocale('zh-CN'); });

it('shows a skeleton for the first search and compact feedback while loading another page', async () => {
  let finishFirst!: (result: ModSearchPage) => void;
  vi.mocked(invoke).mockReturnValue(new Promise<ModSearchPage>(resolve => { finishFirst = resolve; }) as never);
  const wrapper = mount(DiscoverMods, { props: { game, locked: false } });
  expect(wrapper.get('[role="status"]').attributes('aria-live')).toBe('polite');
  expect(wrapper.get('[role="status"]').text()).toContain('Searching');
  expect(wrapper.findAll('.skeleton-row')).toHaveLength(3);

  finishFirst(firstPage);
  await flushPromises();
  expect(wrapper.text()).toContain('First Mod');

  let finishNext!: (result: ModSearchPage) => void;
  vi.mocked(invoke).mockReturnValue(new Promise<ModSearchPage>(resolve => { finishNext = resolve; }) as never);
  await wrapper.get('.load-more').trigger('click');
  expect(wrapper.get('.loading-indicator.compact').text()).toContain('Searching');
  expect(wrapper.text()).toContain('First Mod');

  finishNext({ ...firstPage, hits: [], offset: 1 });
  await flushPromises();
  expect(wrapper.find('.loading-indicator').exists()).toBe(false);
  wrapper.unmount();
});

it('keeps cached results during refresh and ignores a superseded response', async () => {
  vi.mocked(invoke).mockResolvedValueOnce(firstPage);
  const wrapper = mount(DiscoverMods, { props: { game, locked: false } });
  await flushPromises();
  const finishes: ((page: ModSearchPage) => void)[] = [];
  vi.mocked(invoke).mockImplementation(() => new Promise(resolve => { finishes.push(resolve as (page: ModSearchPage) => void); }));
  await wrapper.get('input').setValue('older');
  await wrapper.get('form').trigger('submit');
  expect(wrapper.text()).toContain('First Mod');
  expect(wrapper.findAll('.skeleton-row')).toHaveLength(0);
  await wrapper.get('input').setValue('newer');
  await wrapper.get('form').trigger('submit');
  finishes[1]!({ ...firstPage, hits: [{ ...firstPage.hits[0]!, title: 'New Result' }] });
  await flushPromises();
  finishes[0]!({ ...firstPage, hits: [{ ...firstPage.hits[0]!, title: 'Stale Result' }] });
  await flushPromises();
  expect(wrapper.text()).toContain('New Result');
  expect(wrapper.text()).not.toContain('Stale Result');
  wrapper.unmount();
});

it('returns to the selected result with its search and native scroll position', async () => {
  vi.mocked(invoke).mockResolvedValue(firstPage);
  const host = document.createElement('div');
  document.body.append(host);
  const wrapper = mount(DiscoverMods, { attachTo: host, props: { game, locked: false }, global: { stubs: { ModVersionPicker: true } } });
  await flushPromises();
  await wrapper.get('input').setValue('retained query');
  host.scrollTop = 340;
  await wrapper.get('[data-market-project="one"]').trigger('click');
  expect(host.scrollTop).toBe(0);
  await wrapper.get('.text-button').trigger('click');
  expect(host.scrollTop).toBe(340);
  expect((wrapper.get('input').element as HTMLInputElement).value).toBe('retained query');
  expect(document.activeElement).toBe(wrapper.get('[data-market-project="one"]').element);
  wrapper.unmount(); host.remove();
});

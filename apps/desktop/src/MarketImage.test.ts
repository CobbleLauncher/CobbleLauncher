// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import MarketImage from './MarketImage.vue';
import MarketGallery from './MarketGallery.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); vi.unstubAllGlobals(); setLocale('zh-CN'); });
const png = 'data:image/png;base64,iVBORw0KGgo=';
it('keeps a no-image fallback and fetches artwork only when it enters the viewport', async () => {
  const callbacks: ((entries: { isIntersecting: boolean }[]) => void)[] = [];
  vi.stubGlobal('IntersectionObserver', class { constructor(callback: (entries: { isIntersecting: boolean }[]) => void) { callbacks.push(callback); } observe() {} disconnect() {} });
  vi.mocked(invoke).mockResolvedValue(png);
  const plain = mount(MarketImage, { props: { provider: 'modrinth', title: 'No image' } });
  expect(plain.text()).toBe('N'); expect(invoke).not.toHaveBeenCalled();
  const image = mount(MarketImage, { props: { provider: 'modrinth', title: 'Icon', url: 'https://cdn.modrinth.com/data/lazy/icon.png' } });
  expect(invoke).not.toHaveBeenCalled(); callbacks[0]!([{ isIntersecting: false }]); expect(invoke).not.toHaveBeenCalled();
  callbacks[0]!([{ isIntersecting: true }]); await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith('get_market_image', { provider: 'modrinth', url: 'https://cdn.modrinth.com/data/lazy/icon.png', variant: 'icon' });
  expect(image.get('img').attributes('src')).toBe(png); expect(image.get('img').attributes('loading')).toBe('lazy');
  plain.unmount(); image.unmount();
});
it('does not publish a stale image or expose a remote image URL after native failure', async () => {
  let finish!: (value: string) => void;
  vi.mocked(invoke).mockImplementation((_command, args) => (args as { url: string }).url.includes('/old/') ? new Promise(resolve => { finish = resolve; }) : Promise.reject(new Error('unavailable')));
  const wrapper = mount(MarketImage, { props: { provider: 'curseforge', title: 'Fallback', url: 'https://media.forgecdn.net/avatars/old/icon.png' } });
  await wrapper.setProps({ url: 'https://media.forgecdn.net/avatars/new/icon.png' }); await flushPromises();
  finish(png); await flushPromises();
  expect(wrapper.find('img').exists()).toBe(false); expect(wrapper.text()).toBe('F');
  wrapper.unmount();
});
it('announces a pending preview and clears its loading state after the cached image resolves', async () => {
  let finish!: (value: string) => void;
  vi.mocked(invoke).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const wrapper = mount(MarketImage, { props: { provider: 'modrinth', title: 'Preview', preview: true, url: 'https://cdn.modrinth.com/pending-preview.png' } });
  await flushPromises();
  expect(wrapper.attributes('aria-busy')).toBe('true');
  expect(wrapper.get('[role="status"]').text()).toBe('正在加载图片…');
  finish(png); await flushPromises();
  expect(wrapper.attributes('aria-busy')).toBe('false');
  expect(wrapper.find('[role="status"]').exists()).toBe(false);
  expect(wrapper.get('img').attributes('src')).toBe(png);
  wrapper.unmount();
});
it('renders a supplied gallery with accessible bilingual choices and no detail API requests', async () => {
  vi.mocked(invoke).mockResolvedValue(png);
  setLocale('en-US');
  const urls = ['https://cdn.modrinth.com/data/gallery/images/a.png', 'https://cdn.modrinth.com/data/gallery/images/b.png'];
  const wrapper = mount(MarketGallery, { props: { provider: 'modrinth', urls, title: 'Workshop' } }); await flushPromises();
  expect(wrapper.findAll('.market-gallery-choice')).toHaveLength(2); expect(wrapper.findAll('.market-gallery-choice')[0]!.attributes('aria-pressed')).toBe('true');
  expect(wrapper.findAll('.market-gallery-choice')[1]!.attributes('aria-label')).toBe('Workshop, preview 2');
  await wrapper.findAll('.market-gallery-choice')[1]!.trigger('click'); await flushPromises(); expect(wrapper.findAll('.market-gallery-choice')[1]!.attributes('aria-pressed')).toBe('true');
  setLocale('zh-CN'); await flushPromises(); expect(wrapper.findAll('.market-gallery-choice')[1]!.attributes('aria-label')).toBe('Workshop，预览 2');
  expect(vi.mocked(invoke).mock.calls.every(([command]) => command === 'get_market_image')).toBe(true);
  wrapper.unmount();
});

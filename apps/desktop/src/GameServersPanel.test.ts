// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import GameServersPanel from './GameServersPanel.vue';
import { setLocale } from './i18n';
const { invoke, writeText } = vi.hoisted(() => ({ invoke: vi.fn(), writeText: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText }));
const server = { id: 'bound-entry-id', name: 'Friends', address: 'play.example.org:25565', can_connect: true };
beforeEach(() => {
  setLocale('en-US'); invoke.mockReset(); writeText.mockReset(); writeText.mockResolvedValue(undefined);
  invoke.mockResolvedValue({ items: [server], direct_connect: true });
});
it('reads only the selected game and joins using its bound server ID', async () => {
  const wrapper = mount(GameServersPanel, { props: { gameId: 'abc', locked: false, running: false } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_game_servers', { gameId: 'abc' });
  await wrapper.get('.secondary-button').trigger('click');
  expect(wrapper.emitted('play')).toEqual([['bound-entry-id']]);
  expect(invoke).toHaveBeenCalledTimes(1);
  wrapper.unmount();
});
it('copies an address and prevents another launch while the game runs', async () => {
  const wrapper = mount(GameServersPanel, { props: { gameId: 'abc', locked: true, running: true } });
  await flushPromises();
  expect(wrapper.find('.secondary-button').exists()).toBe(false);
  await wrapper.get('[aria-label="Copy address for Friends"]').trigger('click');
  await flushPromises();
  expect(writeText).toHaveBeenCalledWith(server.address);
  expect(wrapper.text()).toContain('Copied');
  expect(wrapper.emitted('play')).toBeUndefined();
  wrapper.unmount();
});
it('keeps unsupported versions copyable and reports an empty saved list', async () => {
  invoke.mockResolvedValueOnce({ items: [{ ...server, can_connect: false }], direct_connect: false });
  const wrapper = mount(GameServersPanel, { props: { gameId: 'abc', locked: false, running: false } });
  await flushPromises();
  expect(wrapper.find('.secondary-button').exists()).toBe(false);
  expect(wrapper.text()).toContain('Copy an address to connect in this version');
  invoke.mockResolvedValueOnce({ items: [], direct_connect: true });
  await wrapper.setProps({ gameId: 'def' }); await flushPromises();
  expect(wrapper.text()).toContain('No saved servers');
  wrapper.unmount();
});
it('ignores an old request when the selected game changes', async () => {
  let finish!: (value: unknown) => void;
  invoke.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  const wrapper = mount(GameServersPanel, { props: { gameId: 'abc', locked: false, running: false } });
  invoke.mockResolvedValueOnce({ items: [], direct_connect: true });
  await wrapper.setProps({ gameId: 'def' }); await flushPromises();
  finish({ items: [server], direct_connect: true }); await flushPromises();
  expect(wrapper.text()).not.toContain('Friends');
  wrapper.unmount();
});

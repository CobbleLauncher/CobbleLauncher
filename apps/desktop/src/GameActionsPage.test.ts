// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameActionsPage from './GameActionsPage.vue';
import { setLocale } from './i18n';
import type { Game } from './games';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const game: Game = {
  id: 'game-1',
  name: 'Maple',
  version: '1.21.1',
  loader_kind: 'fabric',
  loader_version: '0.16.14',
  directory: 'C:/games/Maple',
  status: 'installed',
  error: null,
  progress: null,
  launch_state: 'idle',
};

beforeEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
afterEach(() => { setLocale('zh-CN'); document.body.innerHTML = ''; });

it('localizes and preserves copy options and result events', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockResolvedValue(game);
  const wrapper = mount(GameActionsPage, { props: { game, action: 'copy', starting: false } });

  expect(wrapper.text()).toContain('Copy the game, mods, and settings');
  expect((wrapper.get('#copy-game-name').element as HTMLInputElement).value).toBe('Maple copy');
  await wrapper.get('input[type="checkbox"]').setValue(true);
  await wrapper.get('form').trigger('submit');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('copy_game', { id: game.id, name: 'Maple copy', includeSaves: true });
  expect(wrapper.emitted('copied')?.[0]?.[0]).toEqual(game);
  wrapper.unmount();
});

it('keeps export warnings and reports successful export in Chinese', async () => {
  vi.mocked(invoke).mockResolvedValue(true);
  const wrapper = mount(GameActionsPage, { props: { game, action: 'export', starting: false } });

  expect(wrapper.text()).toContain('游戏本体、资源缓存和玩家账号不会导出');
  await wrapper.get('form').trigger('submit');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('export_game', { id: game.id, includeSaves: false });
  expect(wrapper.get('[role="status"]').text()).toContain('整合包已导出');
  wrapper.unmount();
});

it('keeps the irreversible delete warning and removes only the selected game folder', async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  const wrapper = mount(GameActionsPage, { props: { game, action: 'remove', starting: false } });

  expect(wrapper.text()).toContain('游戏文件和存档保留在原目录');
  await wrapper.get('[role="radio"][aria-label="连同文件删除"]').trigger('click');
  expect(wrapper.text()).toContain('此操作无法撤销');
  expect(wrapper.text()).toContain(game.directory);
  await wrapper.get('form').trigger('submit');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('remove_game', { id: game.id, deleteFiles: true });
  expect(wrapper.emitted('removed')?.[0]?.[0]).toBe(game.id);
  wrapper.unmount();
});

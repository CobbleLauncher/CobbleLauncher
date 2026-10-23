// SPDX-License-Identifier: MIT
import { expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import GameAppearancePanel from './GameAppearancePanel.vue';
import type { Game } from './games';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

it('keeps native picker cancellation unchanged and emits confirmed icon, reset, and preset updates', async () => {
  setLocale('en-US');
  const game: Game = { id: 'game', name: 'Game', version: '1.21.1', directory: 'synthetic', status: 'installed', progress: null, error: null, appearance_preset: 'grass', icon_ref: null };
  const customized = { ...game, icon_ref: 'a'.repeat(64) };
  const water = { ...game, appearance_preset: 'water' as const };
  vi.mocked(invoke).mockResolvedValueOnce(null).mockResolvedValueOnce(customized).mockResolvedValueOnce(game).mockResolvedValueOnce(water);
  const wrapper = mount(GameAppearancePanel, { props: { game } });
  const button = (text: string) => wrapper.findAll('button').find(value => value.text() === text)!;
  await button('Choose image').trigger('click'); await flushPromises();
  expect(wrapper.emitted('updated')).toBeUndefined();
  await button('Choose image').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('pick_game_icon', { id: game.id });
  expect(wrapper.emitted('updated')?.[0]).toEqual([customized]);
  await wrapper.setProps({ game: customized });
  await button('Reset icon').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('reset_game_icon', { id: game.id });
  expect(wrapper.emitted('updated')?.[1]).toEqual([game]);
  await wrapper.setProps({ game });
  await button('Water').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_game_appearance_preset', { id: game.id, preset: 'water' });
  expect(wrapper.emitted('updated')?.[2]).toEqual([water]);
  wrapper.unmount();
});

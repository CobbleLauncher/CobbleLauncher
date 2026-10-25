// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameModHistory from './GameModHistory.vue';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const game = { id: 'game', name: 'Game', version: '1.21.1', loader_version: '0.16', directory: 'private', status: 'installed' as const, error: null, progress: null };
const history = [{ operation_id: 'operation', created_at: 1, status: 'applied', files: [{ name: 'Example', before: 'old.jar.disabled', after: 'new.jar.disabled', enabled: false }] }];
it('previews conflicts and keeps undo unavailable until a clear review', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mod_history' ? history : ['Mod changed locally: dependency.jar']);
  const wrapper = mount(GameModHistory, { props: { game, locked: false } });
  await flushPromises();
  await wrapper.get('.settings-disclosure-trigger').trigger('click');
  await wrapper.findAll('button').find(button => button.text() === 'Review undo')!.trigger('click');
  await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Undo is blocked by a later change');
  expect(wrapper.text()).toContain('dependency.jar');
  expect(invoke).not.toHaveBeenCalledWith('undo_game_mod_operation', expect.anything());
  wrapper.unmount();
});
it('requires review then explicit undo and emits a refresh', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mod_history' ? history : []);
  const wrapper = mount(GameModHistory, { props: { game, locked: false } });
  await flushPromises();
  await wrapper.get('.settings-disclosure-trigger').trigger('click');
  await wrapper.findAll('button').find(button => button.text() === 'Review undo')!.trigger('click');
  await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('undo_game_mod_operation', expect.anything());
  await wrapper.findAll('button').find(button => button.text() === 'Undo changes')!.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('undo_game_mod_operation', { id: 'game', operationId: 'operation' });
  expect(wrapper.emitted('updated')).toHaveLength(1);
  expect(wrapper.text()).toContain('Disabled');
  wrapper.unmount();
});
it('opens a requested operation with a read-only preview and still requires explicit confirmation', async () => {
  setLocale('en-US'); vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mod_history' ? history : []);
  const wrapper = mount(GameModHistory, { props: { game, locked: false } }); await flushPromises();
  expect(await wrapper.vm.openOperation('operation')).toBe(true); await flushPromises();
  expect(wrapper.get('.settings-disclosure-trigger').attributes('aria-expanded')).toBe('true');
  expect(invoke).toHaveBeenCalledWith('preview_game_mod_operation_undo', { id: 'game', operationId: 'operation' });
  expect(invoke).not.toHaveBeenCalledWith('undo_game_mod_operation', expect.anything());
  await wrapper.findAll('button').find(button => button.text() === 'Undo changes')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('undo_game_mod_operation', { id: 'game', operationId: 'operation' }); wrapper.unmount();
});


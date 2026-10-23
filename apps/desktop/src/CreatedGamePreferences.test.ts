// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import CreatedGamePreferences from './CreatedGamePreferences.vue';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });

const configDoctor = { enabled: true, manifest: { id: 'config-doctor', frontend: { entry: 'ui/index.html' }, permissions: [{ permission: 'launcher:game-keybindings/read' }, { permission: 'launcher:game-keybindings/write' }] } };

it('offers key reuse when Config Doctor is enabled with keybinding access', async () => {
  vi.mocked(invoke).mockResolvedValue([configDoctor]);
  setLocale('en-US');
  const wrapper = mount(CreatedGamePreferences, { props: { gameId: 'new-game' } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_plugins');
  expect(wrapper.text()).toContain('Use keys from another game');
  await wrapper.get('button.created-game-preferences-action').trigger('click');
  expect(wrapper.emitted('reuse')?.[0]).toEqual(['new-game']);
  wrapper.unmount();
});

it('shows nothing when Config Doctor is absent or cannot copy keybindings', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const wrapper = mount(CreatedGamePreferences, { props: { gameId: 'new-game' } });
  await flushPromises();
  expect(wrapper.find('.created-game-preferences').exists()).toBe(false);
  await wrapper.setProps({ gameId: 'another-game' });
  wrapper.unmount();

  vi.mocked(invoke).mockResolvedValue([{ ...configDoctor, manifest: { ...configDoctor.manifest, permissions: [{ permission: 'launcher:game-keybindings/read' }] } }]);
  const noWrite = mount(CreatedGamePreferences, { props: { gameId: 'new-game' } });
  await flushPromises();
  expect(noWrite.find('.created-game-preferences').exists()).toBe(false);
  noWrite.unmount();
});

it('lets the user dismiss the suggestion', async () => {
  vi.mocked(invoke).mockResolvedValue([configDoctor]);
  const wrapper = mount(CreatedGamePreferences, { props: { gameId: 'new-game' } });
  await flushPromises();
  await wrapper.get('button[aria-label]').trigger('click');
  expect(wrapper.emitted('dismiss')).toHaveLength(1);
  wrapper.unmount();
});

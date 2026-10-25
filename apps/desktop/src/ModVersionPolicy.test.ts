// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { setLocale } from './i18n';
import ModVersionPolicy from './ModVersionPolicy.vue';
import SelectField from './ui/SelectField.vue';
import { emptyModUpdatePolicy } from './mod-version-policy';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });

it('persists a provider-scoped ignored version and restores it without changing the game channel', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => command === 'get_game_mod_update_policy' ? emptyModUpdatePolicy() : undefined);
  const wrapper = mount(ModVersionPolicy, { props: { gameId: 'game', projectId: 'Project', selectedVersion: { version_id: 'v2', version_number: '2.0' } } });
  await flushPromises();
  await wrapper.findAll('button').find(button => button.text().startsWith('Ignore this update'))!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('set_game_mod_update_policy', { id: 'game', policy: { schema_version: 1, channel: 'stable', projects: { 'modrinth:Project': { ignored_versions: ['v2'] } } } });
  expect(wrapper.text()).toContain('Required dependencies can still use ignored versions.');
  await wrapper.findAll('button').find(button => button.text().startsWith('Allow this version ·'))!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('set_game_mod_update_policy', { id: 'game', policy: emptyModUpdatePolicy() });
  expect(wrapper.emitted('changed')).toHaveLength(2);
  wrapper.unmount();
});

it('supports a project channel override and blocks mutations while locked', async () => {
  setLocale('en-US'); vi.mocked(invoke).mockImplementation(async command => command === 'get_game_mod_update_policy' ? emptyModUpdatePolicy() : undefined);
  const wrapper = mount(ModVersionPolicy, { props: { gameId: 'game', projectId: 'Project', selectedVersion: { version_id: 'v2', version_number: '2.0' } } });
  await flushPromises();
  wrapper.findAllComponents(SelectField)[1]!.vm.$emit('update:modelValue', 'preview'); await flushPromises();
  expect(invoke).toHaveBeenLastCalledWith('set_game_mod_update_policy', { id: 'game', policy: { schema_version: 1, channel: 'stable', projects: { 'modrinth:Project': { channel: 'preview', ignored_versions: [] } } } });
  await wrapper.setProps({ disabled: true });
  const count = vi.mocked(invoke).mock.calls.length;
  await wrapper.findAll('button').find(button => button.text().startsWith('Ignore this update'))!.trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls).toHaveLength(count);
  wrapper.unmount();
});

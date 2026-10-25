// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import CurseForgeUpdates from './CurseForgeUpdates.vue';
import CurseForgeUpdateReview from './CurseForgeUpdateReview.vue';
import ModVersionPolicy from './ModVersionPolicy.vue';
import SelectField from './ui/SelectField.vue';
import { emptyModUpdatePolicy } from './mod-version-policy';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const changes = [{ project_id: '42', name: 'Forest Map', from_version: '1', to_version: '2', filename: 'map.jar', enabled: true }, { project_id: '44', name: 'Required API', from_version: null, to_version: '1', filename: 'api.jar', enabled: true }];
const missing = [{ project_id: '44', file_id: '88', name: 'Required API', file_name: 'api.jar', size_bytes: 10, website_url: null, reason: 'manual' }];
function mockCommands() {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_game_mod_update_policy') return emptyModUpdatePolicy();
    if (command === 'list_game_curseforge_mods') return ['42', '43'].map(project_id => ({ project_id, version_id: '80', filename: `${project_id}.jar`, enabled: true, pinned: false }));
    if (command === 'check_game_curseforge_updates') return ['42', '43'].map(project_id => ({ project_id, name: `Mod ${project_id}`, current_version: '1', version_number: '2', version_id: '84', filename: `${project_id}.jar`, enabled: true }));
    if (command === 'preview_game_curseforge_updates') return { plan_id: 'update-plan', changes, missing: [], ready: true };
    return null;
  });
}
beforeEach(() => { setLocale('en-US'); mockCommands(); });
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });

it('previews selected CurseForge roots together and requires the reviewed shared apply command', async () => {
  const wrapper = mount(CurseForgeUpdates, { props: { gameId: 'game', locked: false } });
  await wrapper.vm.check(); await flushPromises();
  await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true);
  await wrapper.findAll('button').find(button => button.text() === 'Update selected (2)')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_game_curseforge_updates', { id: 'game', projectIds: ['42', '43'] });
  expect(wrapper.text()).toContain('Required API');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'apply_game_mod_updates')).toBe(false);
  await wrapper.get('.primary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('apply_game_mod_updates', { id: 'game', planId: 'update-plan' });
  expect(wrapper.emitted('updated')).toHaveLength(1);
  wrapper.unmount();
});

it('shares policy controls with a provider-scoped key and sends source-aware pin mutations', async () => {
  const wrapper = mount(CurseForgeUpdates, { props: { gameId: 'game', locked: false } });
  await wrapper.vm.check(); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Preferences')!.trigger('click'); await flushPromises();
  const policy = wrapper.getComponent(ModVersionPolicy);
  expect(policy.props('source')).toBe('curseforge');
  policy.getComponent(SelectField).vm.$emit('update:modelValue', 'preview'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_game_mod_update_policy', { id: 'game', policy: { schema_version: 1, channel: 'stable', projects: { 'curseforge:42': { channel: 'preview', ignored_versions: [] } } } });
  await wrapper.findAll('button').find(button => button.text() === 'CurseForge versions')!.trigger('click'); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Pin version')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_game_mod_pinned', { id: 'game', projectId: '42', source: 'curseforge', pinned: true });
  wrapper.unmount();
});

it('keeps required manual dependencies in review and blocks apply until native verification succeeds', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'preview_game_curseforge_updates' ? { plan_id: 'update-plan', changes, missing, ready: false } : command === 'select_curseforge_manual_file' ? { plan_id: 'update-plan', missing: [], ready: true } : null);
  const wrapper = mount(CurseForgeUpdateReview, { props: { gameId: 'game', projectIds: ['42'], locked: false } }); await flushPromises();
  expect(wrapper.text()).toContain('Required API');
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeDefined();
  await wrapper.get('.cf-missing .secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('select_curseforge_manual_file', { id: 'game', planId: 'update-plan', projectId: '44', fileId: '88' });
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeUndefined();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'apply_game_mod_updates')).toBe(false);
  wrapper.unmount();
});

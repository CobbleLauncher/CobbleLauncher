// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { setLocale } from '../i18n';
import CreateGamePage from '../CreateGamePage.vue';
import ReverseConfiguration from './ReverseConfiguration.vue';
import SelectField from '../ui/SelectField.vue';
import { reverseConfigurationFixture, reverseModFixtures } from './fixtures';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
const global = { stubs: { MarketImage: true, CompatibilityNotice: true } };
const page = { hits: reverseModFixtures, offset: 0, total_hits: 2 };
beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'search_environment_mods') return page;
    if (command === 'reverse_configure_environment') return reverseConfigurationFixture;
    if (command === 'get_download_settings') return { source: 'official', mirror: '' };
    if (command === 'default_game_directory') return 'C:/Games';
    if (command === 'list_game_versions') return [{ id: '1.21.1', kind: 'release' }];
    if (command === 'preview_environment_installation') return { loader_version:'0.16.14',requirements:{ready:true,missing:[]},space:{can_proceed:true,exact:true,volumes:[]} };
    if (command === 'create_game_from_environment') return { id: 'from-mods', status: 'installing', name: 'My shaders' };
    return null;
  });
});
afterEach(() => { setLocale('zh-CN'); vi.useRealTimers(); });
function deferred<T>() { let resolve!: (value: T) => void; let reject!: (reason: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }

it('reviews exact versions and required dependencies before creating from the selected host plan', async () => {
  const wrapper = mount(CreateGamePage, { global }); await flushPromises();
  await wrapper.findAll('[role=radio]').find(item => item.text() === 'Start with Mods')!.trigger('click'); await flushPromises();
  expect(wrapper.find('#game-name').exists()).toBe(false);
  await wrapper.findAll('.reverse-hit')[0]!.trigger('click');
  await wrapper.findAll('.reverse-hit')[1]!.trigger('click');
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('reverse_configure_environment', { roots: [{source:'modrinth',project_id:'sodium'}, {source:'modrinth',project_id:'iris'}], preferredGameVersions: [], loaders: ['fabric', 'neoforge', 'forge'], requestId: expect.any(String) });
  expect(wrapper.get('.reverse-plan').text()).toContain('0.6.13+mc1.21.1');
  expect(wrapper.get('.settings-disclosure').text()).toContain('Required dependencies (1)');
  expect(wrapper.get('#game-name').element).toHaveProperty('value', 'Minecraft 1.21.1 Fabric');
  expect(wrapper.find('#create-install-source').exists()).toBe(false);
  expect(wrapper.find('#create-install-directory').exists()).toBe(false);
  await wrapper.get('.create-install-options').trigger('click');
  expect(wrapper.find('#create-install-source').exists()).toBe(true);
  expect(wrapper.find('#create-install-directory').exists()).toBe(true);
  await wrapper.findAll('.reverse-choice')[1]!.trigger('click'); await flushPromises();
  expect(wrapper.get('.reverse-plan').text()).toContain('neoforge');
  await wrapper.get('#game-name').setValue('My shaders');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('create_game_from_environment', { planId: 'reviewed-neoforge-plan', name: 'My shaders', automaticName: false, source: { source: 'official', mirror: '' }, useDefaultDirectory: true });
  expect(invoke).not.toHaveBeenCalledWith('create_game', expect.anything());
  expect(wrapper.emitted('created')?.[0]?.[0]).toEqual(expect.objectContaining({ id: 'from-mods', status: 'installing' }));
  wrapper.unmount();
});

it('ignores superseded search responses and failures', async () => {
  const old = deferred<typeof page>();
  const latest = deferred<typeof page>();
  vi.mocked(invoke).mockImplementation(command => command === 'search_environment_mods' ? old.promise : Promise.resolve(null));
  const wrapper = mount(ReverseConfiguration, { global });
  await wrapper.get('input').setValue('iris');
  vi.mocked(invoke).mockImplementation(command => command === 'search_environment_mods' ? latest.promise : Promise.resolve(null));
  await wrapper.get('.reverse-search button').trigger('click');
  latest.resolve({ hits: [reverseModFixtures[1]!], offset: 0, total_hits: 1 }); await flushPromises();
  old.reject(new Error('old offline request')); await flushPromises();
  expect(wrapper.findAll('.reverse-hit')).toHaveLength(1);
  expect(wrapper.get('.reverse-hit').text()).toContain('Iris');
  expect(wrapper.find('[role=alert]').exists()).toBe(false);
  wrapper.unmount();
});

it('cancels native solving and suppresses a stale plan when the selected Mods change', async () => {
  const response = deferred<typeof reverseConfigurationFixture>();
  vi.mocked(invoke).mockImplementation(command => command === 'search_environment_mods' ? Promise.resolve(page) : command === 'reverse_configure_environment' ? response.promise : Promise.resolve(null));
  const wrapper = mount(ReverseConfiguration, { global }); await flushPromises();
  await wrapper.findAll('.reverse-hit')[0]!.trigger('click');
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click');
  const args = vi.mocked(invoke).mock.calls.find(call => call[0] === 'reverse_configure_environment')![1] as { requestId: string };
  await wrapper.get('[aria-label="Remove Sodium"]').trigger('click');
  expect(invoke).toHaveBeenCalledWith('cancel_environment_configuration', { requestId: args.requestId });
  response.resolve(reverseConfigurationFixture); await flushPromises();
  expect(wrapper.find('.reverse-proposals').exists()).toBe(false);
  expect(wrapper.emitted('selected')?.at(-1)).toEqual([null]);
  expect(wrapper.get('.reverse-solve-actions .primary-button').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('shows retryable network errors, empty search results, and conflicts without a create action', async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error('network'));
  const wrapper = mount(ReverseConfiguration, { global }); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('Could not search this source');
  vi.mocked(invoke).mockResolvedValueOnce({ hits: [], offset: 0, total_hits: 0 });
  await wrapper.get('.reverse-inline-error button').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('No Mods found');
  vi.mocked(invoke).mockResolvedValueOnce(page);
  await wrapper.get('.reverse-search button').trigger('click'); await flushPromises();
  await wrapper.findAll('.reverse-hit')[0]!.trigger('click');
  vi.mocked(invoke).mockResolvedValueOnce({ ...reverseConfigurationFixture, choices: [], conflicts: [{ game_version: '1.21.1', loader: 'fabric', code: 'version_conflict', message: 'Required dependency versions conflict.', projects: [{source:'modrinth',project_id:'sodium'}] }] });
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('No available combination');
  expect(wrapper.get('.reverse-conflicts').text()).toContain('Required dependency versions conflict.');
  expect(wrapper.get('.reverse-conflicts').text()).toContain('Sodium');
  expect(wrapper.emitted('selected')?.at(-1)).toEqual([null]);
  wrapper.unmount();
});

it('clears the selected plan when leaving the Mods creation mode', async () => {
  const wrapper = mount(CreateGamePage, { global }); await flushPromises();
  await wrapper.findAll('[role=radio]').find(item => item.text() === 'Start with Mods')!.trigger('click'); await flushPromises();
  await wrapper.findAll('.reverse-hit')[0]!.trigger('click');
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click'); await flushPromises();
  expect(wrapper.find('#game-name').exists()).toBe(true);
  await wrapper.findAll('[role=radio]').find(item => item.text() === 'Choose a version')!.trigger('click'); await flushPromises();
  await wrapper.findAll('[role=radio]').find(item => item.text() === 'Start with Mods')!.trigger('click'); await flushPromises();
  expect(wrapper.find('#game-name').exists()).toBe(false);
  expect(wrapper.find('.done-button').exists()).toBe(false);
  wrapper.unmount();
});

it('requires a fresh review when the native plan expires', async () => {
  const wrapper = mount(CreateGamePage, { global }); await flushPromises();
  await wrapper.findAll('[role=radio]').find(item => item.text() === 'Start with Mods')!.trigger('click'); await flushPromises();
  await wrapper.findAll('.reverse-hit')[0]!.trigger('click');
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click'); await flushPromises();
  vi.mocked(invoke).mockRejectedValueOnce(new Error('ENVIRONMENT_PLAN_EXPIRED'));
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('This combination expired');
  expect(wrapper.get('.done-button').attributes('disabled')).toBeDefined();
  await wrapper.get('.create-resolve-again').trigger('click'); await flushPromises();
  expect(wrapper.find('[role=alert]').exists()).toBe(false);
  expect(wrapper.get('.done-button').attributes('disabled')).toBeUndefined();
  expect(vi.mocked(invoke).mock.calls.filter(call => call[0] === 'reverse_configure_environment')).toHaveLength(2);
  wrapper.unmount();
});

it('bounds the visible combinations and localizes metadata limitations', async () => {
  const many = { ...reverseConfigurationFixture, choices: Array.from({ length: 32 }, (_, index) => ({ ...reverseConfigurationFixture.choices[0]!, plan_id: `plan-${index}` })), limitations: ['modrinth_only', 'stable_unpinned_versions_only'] };
  vi.mocked(invoke).mockImplementation(async command => command === 'search_environment_mods' ? page : command === 'reverse_configure_environment' ? many : null);
  const wrapper = mount(ReverseConfiguration, { global }); await flushPromises();
  await wrapper.findAll('.reverse-hit')[0]!.trigger('click');
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click'); await flushPromises();
  expect(wrapper.findAll('.reverse-choice')).toHaveLength(5);
  expect(wrapper.get('.reverse-conflicts').text()).toContain('Currently searches Mods from Modrinth only.');
  await wrapper.findAll('button').find(button => button.text() === 'More combinations')!.trigger('click');
  expect(wrapper.findAll('.reverse-choice')).toHaveLength(10);
  wrapper.unmount();
});

it('keeps equal project IDs from different sources distinct in the solver request', async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'search_environment_mods') {
      const source = (args as {source:'modrinth'|'curseforge'}).source;
      return { hits:[{...reverseModFixtures[0]!, source, title: source === 'modrinth' ? 'Sodium' : 'Another Sodium'}],offset:0,total_hits:1 };
    }
    if (command === 'reverse_configure_environment') return reverseConfigurationFixture;
    return null;
  });
  const wrapper=mount(ReverseConfiguration,{global});await flushPromises();
  await wrapper.get('.reverse-hit').trigger('click');
  wrapper.findComponent(SelectField).vm.$emit('update:modelValue','curseforge');await flushPromises();
  await wrapper.get('.reverse-hit').trigger('click');
  expect(wrapper.findAll('.reverse-selected-list li')).toHaveLength(2);
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click');await flushPromises();
  expect(invoke).toHaveBeenCalledWith('reverse_configure_environment',expect.objectContaining({roots:[{source:'modrinth',project_id:'sodium'},{source:'curseforge',project_id:'sodium'}]}));
  wrapper.unmount();
});

it('blocks creation until native file and space preflight succeeds', async () => {
  const original=vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_environment_installation'?Promise.resolve({loader_version:'0.16.14',requirements:{ready:false,missing:[]},space:{can_proceed:false,exact:true,volumes:[]}}):original(command,args));
  const wrapper=mount(CreateGamePage,{global});await flushPromises();
  await wrapper.findAll('[role=radio]').find(item=>item.text()==='Start with Mods')!.trigger('click');await flushPromises();
  await wrapper.get('.reverse-hit').trigger('click');
  await wrapper.get('.reverse-solve-actions .primary-button').trigger('click');await flushPromises();
  expect(wrapper.get('.done-button').attributes('disabled')).toBeDefined();
  await wrapper.get('form').trigger('submit');await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='create_game_from_environment')).toBe(false);
  wrapper.unmount();
});

it('refreshes readiness when native creation detects changed disk space and allows rechecking after cleanup', async () => {
  const original=vi.mocked(invoke).getMockImplementation()!;let preflights=0;
  vi.mocked(invoke).mockImplementation((command,args)=>{
    if(command==='preview_environment_installation'){preflights+=1;return Promise.resolve({loader_version:'0.16.14',requirements:{ready:true,missing:[]},space:{can_proceed:preflights!==2,exact:true,volumes:[]}});}
    if(command==='create_game_from_environment')return Promise.reject(new Error('SPACE_INSUFFICIENT'));
    return original(command,args);
  });
  const wrapper=mount(CreateGamePage,{global});await flushPromises();
  await wrapper.findAll('[role=radio]').find(item=>item.text()==='Start with Mods')!.trigger('click');await flushPromises();
  await wrapper.get('.reverse-hit').trigger('click');await wrapper.get('.reverse-solve-actions .primary-button').trigger('click');await flushPromises();
  await wrapper.get('form').trigger('submit');await flushPromises();
  expect(preflights).toBe(2);expect(wrapper.get('.done-button').attributes('disabled')).toBeDefined();
  await wrapper.findAll('button').find(button=>button.text()==='Check space again')!.trigger('click');await flushPromises();
  expect(preflights).toBe(3);expect(wrapper.get('.done-button').attributes('disabled')).toBeUndefined();expect(wrapper.find('[role=alert]').exists()).toBe(false);
  wrapper.unmount();
});

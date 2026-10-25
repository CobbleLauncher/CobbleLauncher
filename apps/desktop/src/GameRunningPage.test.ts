// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameRunningPage from './GameRunningPage.vue';
import type { Game } from './games';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const game: Game = { id: 'game-a', name: 'Create Together', version: '1.21.1', status: 'installed', directory: 'private', progress: null, error: null, launch_state: 'running', log_path: 'private/log' };
const backup = { preference: 'enabled', last_backup: '2026-10-23T18:10:15+08:00[+08:00]', last_error: null };
beforeEach(() => { setLocale('en-US'); localStorage.clear(); vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockResolvedValue(backup); });
afterEach(() => setLocale('zh-CN'));
it('shows real session context and read-only after-exit backup, sharing uses the host flow', async () => {
  const wrapper = mount(GameRunningPage, { props: { game, profile: {id:'selected',name:'NewlySelectedPlayer',uuid:'fixture',kind:'offline',note:null} } }); await flushPromises();
  expect(wrapper.text()).not.toContain('NewlySelectedPlayer');
  expect(wrapper.text()).toContain('Game is running'); expect(wrapper.text()).toContain('After the game closes'); expect(wrapper.text()).toContain('Last backup:');
  await wrapper.findAll('button').find(button => button.text().includes('Play with friends'))!.trigger('click');
  expect(wrapper.emitted('share')).toEqual([[]]); expect(vi.mocked(invoke).mock.calls.map(call => call[0])).toEqual(['get_automatic_world_backup']); wrapper.unmount();
});
it('does not invent tools, and binds only eligible supplied tools to the current game', async () => {
  const wrapper = mount(GameRunningPage, { props: { game, commands: [{ pluginId: 'seed', commandId: 'lookup', label: 'Seed lookup', workflow: 'tool_form', instanceId: 'game-a' }, { pluginId: 'other', commandId: 'inspect', label: 'Other game', workflow: 'viewer', instanceId: 'game-b' }] } });
  expect(wrapper.text()).not.toContain('Other game'); await wrapper.get('.tool-action').trigger('click');
  expect(wrapper.emitted('extensionCommand')?.[0]?.[0]).toMatchObject({ pluginId: 'seed', instanceId: 'game-a' });
  await wrapper.setProps({ commands: [] }); expect(wrapper.find('.tool-grid').exists()).toBe(false); wrapper.unmount();
});
it('keeps preparing and verification distinct and offers safe verification retry', async () => {
  const wrapper = mount(GameRunningPage, { props: { game, starting: true } });
  expect(wrapper.text()).toContain('Starting game'); expect(wrapper.text()).not.toContain('Play with friends');
  await wrapper.setProps({ starting: false, game: { ...game, launch_state: 'verifying' } }); expect(wrapper.text()).toContain('Checking game process');
  await wrapper.setProps({ game: { ...game, launch_state: 'recovery_failed' } }); await wrapper.findAll('button').find(button => button.text() === 'Check again')!.trigger('click');
  expect(wrapper.emitted('retrySessionRecovery')).toEqual([['game-a']]); expect(wrapper.emitted('play')).toBeUndefined(); wrapper.unmount();
});
it('uses existing failure recovery and keeps after-exit launch separate', async () => {
  const wrapper = mount(GameRunningPage, { props: { game: { ...game, launch_state: 'idle', launch_error: 'Java is incompatible' } }, global: { stubs: { LaunchRecoveryPanel: true } } });
  expect(wrapper.findComponent({ name: 'LaunchRecoveryPanel' }).exists()).toBe(true);
  await wrapper.setProps({ game: { ...game, launch_state: 'idle', launch_error: null } });
  await wrapper.findAll('button').find(button => button.text().includes('Launch again'))!.trigger('click'); expect(wrapper.emitted('play')).toEqual([['game-a']]); wrapper.unmount();
});
it('ignores stale backup responses after switching games and never enables unavailable log', async () => {
  let resolveOld!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }));
  const wrapper = mount(GameRunningPage, { props: { game } });
  await wrapper.setProps({ game: { ...game, id: 'game-b', log_path: null } }); await flushPromises(); resolveOld({ ...backup, preference: 'disabled' }); await flushPromises();
  expect(wrapper.text()).toContain('After the game closes'); expect(wrapper.findAll('button').find(button => button.text().includes('Open log'))!.attributes('disabled')).toBeDefined(); wrapper.unmount();
});

it('shows tool refresh failures and disables duplicate opens while the host is busy', async () => {
  const command = {pluginId:'seed',commandId:'lookup',label:'Seed lookup',workflow:'tool_form' as const,instanceId:'game-a'};
  const wrapper = mount(GameRunningPage, {props:{game,commands:[command],toolsBusy:true,toolsError:'Unable to load tools'}});
  expect(wrapper.get('.tool-action').attributes('disabled')).toBeDefined();
  expect(wrapper.text()).toContain('Unable to load tools');
  await wrapper.findAll('button').find(button => button.text() === 'Check again')!.trigger('click');
  expect(wrapper.emitted('refreshTools')).toEqual([[]]);
  await wrapper.setProps({toolsBusy:false}); await wrapper.get('.tool-action').trigger('click');
  expect(wrapper.emitted('extensionCommand')?.[0]?.[0]).toBe(wrapper.props('commands')![0]);
  wrapper.unmount();
});

it('keeps Seed Compass out of the generic tools and disables its entry while tools are busy', async () => {
  const wrapper = mount(GameRunningPage, { props: {
    game,
    toolsLoading: true,
    commands: [
      { pluginId: 'seed-compass', commandId: 'analyze', label: 'Analyze seed', workflow: 'tool_form', instanceId: 'game-a' },
      { pluginId: 'other', commandId: 'inspect', label: 'Inspect world', workflow: 'tool_form', instanceId: 'game-a' },
    ],
  } });
  const seedButton = wrapper.findAll('button').find(button => button.text().includes('Seed'))!;
  expect(seedButton.attributes('disabled')).toBeDefined();
  expect(wrapper.text()).not.toContain('Analyze seed');
  expect(wrapper.text()).toContain('Inspect world');
  await seedButton.trigger('click');
  expect(wrapper.emitted('seed')).toBeUndefined();

  await wrapper.setProps({ toolsLoading: false, toolsBusy: false });
  await wrapper.findAll('button').find(button => button.text().includes('Seed'))!.trigger('click');
  expect(wrapper.emitted('seed')).toEqual([[]]);
  wrapper.unmount();
});

it('pins a running tool for this game and keeps the pin after reopening', async () => {
  const commands = Array.from({ length: 5 }, (_, index) => ({ pluginId: `tool-${index}`, commandId: 'open', label: `Tool ${index}`, workflow: 'tool_form' as const, instanceId: 'game-a' }));
  const wrapper = mount(GameRunningPage, { props: { game, commands } });
  expect(wrapper.findAll('.tool-action')).toHaveLength(4);
  await wrapper.findAll('button').find(button => button.text() === 'Show all tools')!.trigger('click');
  expect(wrapper.findAll('.tool-action')).toHaveLength(5);
  await wrapper.get('button[aria-label="Pin Tool 4"]').trigger('click');
  expect(wrapper.get('button[aria-label="Unpin Tool 4"]').attributes('aria-pressed')).toBe('true');
  expect(JSON.parse(localStorage.getItem('cobble.running-tool-pins.v1')!)['game-a']).toEqual(['tool-4:open']);
  wrapper.unmount();
  const reopened = mount(GameRunningPage, { props: { game, commands } });
  expect(reopened.findAll('.tool-action')).toHaveLength(4);
  expect(reopened.findAll('.tool-action')[0]!.text()).toContain('Tool 4');
  reopened.unmount();
});

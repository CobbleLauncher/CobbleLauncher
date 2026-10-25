// SPDX-License-Identifier: MIT
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { defineComponent, h, ref } from 'vue';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import { setLocale } from './i18n';
import type { ExtensionContribution, ResolvedCommand } from './extensions/contributions';
import { recordToolOpen } from './tool-usage';
import { useRunningTools } from './use-running-tools';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const wrappers: VueWrapper[] = [];
const descriptors: ExtensionContribution[] = [{ pluginId: 'real-plugin', pluginName: 'Real plugin', settings: [], commands: [
  { id: 'tools', label: 'Tools action', workflow: 'report', menus: [{ position: 'tools', when: 'always' }] },
  { id: 'instance', label: 'Instance action', workflow: 'tool_form', menus: [{ position: 'instance', when: 'instance_selected' }] },
  { id: 'blueprint', label: 'Blueprint action', workflow: 'viewer', menus: [{ position: 'blueprint', when: 'blueprint_selected' }] },
] }];
function harness(activeValue = true, gameValue: string | undefined = 'game-a') {
  const gameId = ref<string | undefined>(gameValue), launchState = ref('running'), active = ref(activeValue);
  let tools!: ReturnType<typeof useRunningTools>;
  const wrapper = mount(defineComponent({ setup() { tools = useRunningTools({ gameId, launchState, active }); return () => h('div'); } }));
  wrappers.push(wrapper); return { wrapper, tools, gameId, launchState, active };
}
function deferred<T>() { let finish!: (value: T) => void, fail!: (error: Error) => void; const promise = new Promise<T>((resolve, reject) => { finish = resolve; fail = reject; }); return { promise, finish, fail }; }
beforeEach(() => { vi.mocked(invoke).mockReset(); localStorage.clear(); setLocale('en-US'); });
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); vi.restoreAllMocks(); setLocale('zh-CN'); });

it('loads native context metadata, excludes blueprint menus, binds and ranks eligible commands', async () => {
  recordToolOpen({ pluginId: 'real-plugin', commandId: 'instance' });
  vi.mocked(invoke).mockResolvedValue(descriptors);
  const { tools } = harness(); await flushPromises();
  expect(invoke).toHaveBeenCalledExactlyOnceWith('list_extension_contributions', { locale: 'en', context: { instanceId: 'game-a' } });
  expect(tools.commands.value.map(command => command.commandId)).toEqual(['instance', 'tools']);
  expect(tools.commands.value.every(command => command.instanceId === 'game-a')).toBe(true);
  expect(tools.loading.value).toBe(false); expect(tools.error.value).toBe('');
});
it('does no work while inactive or without a game, and refreshes only on context changes or request', async () => {
  vi.mocked(invoke).mockResolvedValue(descriptors);
  const { tools, active, gameId, launchState } = harness(false, undefined); await flushPromises(); expect(invoke).not.toHaveBeenCalled();
  gameId.value = 'game-a'; await flushPromises(); expect(invoke).not.toHaveBeenCalled();
  active.value = true; await flushPromises(); expect(invoke).toHaveBeenCalledTimes(1);
  launchState.value = 'stopped'; await flushPromises(); expect(invoke).toHaveBeenCalledTimes(2);
  setLocale('zh-CN'); await flushPromises(); expect(invoke).toHaveBeenLastCalledWith('list_extension_contributions', { locale: 'zh-CN', context: { instanceId: 'game-a' } });
  await tools.refresh(); expect(invoke).toHaveBeenCalledTimes(4);
  active.value = false; await flushPromises(); expect(tools.commands.value).toEqual([]); expect(invoke).toHaveBeenCalledTimes(4);
});
it('resolves a real issued command with the exact listing game, locale and menu', async () => {
  const resolved: ResolvedCommand = { pluginId: 'real-plugin', commandId: 'tools', label: 'Native resolved label', workflow: 'report', instanceId: 'game-a' };
  vi.mocked(invoke).mockImplementation(command => Promise.resolve(command === 'list_extension_contributions' ? descriptors : resolved));
  const { tools } = harness(); await flushPromises();
  const command = tools.commands.value.find(command => command.commandId === 'tools')!;
  expect(await tools.resolve(command)).toEqual(resolved);
  expect(invoke).toHaveBeenLastCalledWith('resolve_extension_command', { pluginId: 'real-plugin', commandId: 'tools', position: 'tools', locale: 'en', context: { instanceId: 'game-a' } });
  expect(tools.resolving.value).toBe(false);
  expect(await tools.resolve({ ...command, commandId: 'invented' })).toBeNull(); expect(invoke).toHaveBeenCalledTimes(2);
});
it('discards stale loads, errors and stale clicks after a game switch', async () => {
  const stale = deferred<ExtensionContribution[]>();
  vi.mocked(invoke).mockImplementation((_command, args) => (args as { context: { instanceId: string } }).context.instanceId === 'game-a' ? stale.promise : Promise.resolve(descriptors));
  const { tools, gameId } = harness(); gameId.value = 'game-b'; await flushPromises();
  expect(tools.commands.value.every(command => command.instanceId === 'game-b')).toBe(true);
  stale.fail(new Error('old load failed')); await flushPromises(); expect(tools.error.value).toBe('');
  const previous = tools.commands.value[0]!; gameId.value = 'game-c'; await flushPromises();
  const before = vi.mocked(invoke).mock.calls.length; expect(await tools.resolve(previous)).toBeNull(); expect(invoke).toHaveBeenCalledTimes(before);
});
it('suppresses a pending resolution when state, game or mount lifetime changes', async () => {
  const pending = deferred<ResolvedCommand>();
  vi.mocked(invoke).mockImplementation(command => command === 'list_extension_contributions' ? Promise.resolve(descriptors) : pending.promise);
  const { tools, launchState, wrapper } = harness(); await flushPromises();
  const command = tools.commands.value[0]!, result = tools.resolve(command);
  expect(tools.resolving.value).toBe(true); launchState.value = 'stopping'; await flushPromises();
  pending.finish({ ...command }); expect(await result).toBeNull();
  const fresh = tools.commands.value[0]!, afterUnmount = tools.resolve(fresh); wrapper.unmount(); expect(await afterUnmount).toBeNull(); expect(await tools.resolve(fresh)).toBeNull(); expect(tools.commands.value).toEqual([]);
});
it('prevents duplicate resolves and suppresses late resolution after a locale change', async () => {
  const pending = deferred<ResolvedCommand>();
  vi.mocked(invoke).mockImplementation(command => command === 'list_extension_contributions' ? Promise.resolve(descriptors) : pending.promise);
  const { tools } = harness(); await flushPromises(); const command = tools.commands.value[0]!, result = tools.resolve(command);
  expect(await tools.resolve(command)).toBeNull(); expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'resolve_extension_command')).toHaveLength(1);
  setLocale('zh-CN'); await flushPromises(); pending.finish({ ...command }); expect(await result).toBeNull(); expect(tools.error.value).toBe('');
});
it('discards a pending load after unmount without introducing commands or errors', async () => {
  const pending = deferred<ExtensionContribution[]>(); vi.mocked(invoke).mockReturnValue(pending.promise);
  const { tools, wrapper } = harness(); expect(tools.loading.value).toBe(true); wrapper.unmount(); pending.finish(descriptors); await flushPromises();
  expect(tools.commands.value).toEqual([]); expect(tools.loading.value).toBe(false); expect(tools.error.value).toBe('');
});
it('exposes friendly load/resolve failures, retries and rejects a mismatched native binding', async () => {
  vi.mocked(invoke).mockRejectedValue(new Error('private native error'));
  const { tools } = harness(); await flushPromises(); expect(tools.error.value).not.toBe(''); expect(tools.error.value).not.toContain('private');
  vi.mocked(invoke).mockResolvedValue(descriptors); await tools.refresh(); expect(tools.error.value).toBe('');
  vi.mocked(invoke).mockResolvedValue({ ...tools.commands.value[0]!, instanceId: 'wrong-game' });
  expect(await tools.resolve(tools.commands.value[0]!)).toBeNull(); expect(tools.error.value).not.toBe('');
});

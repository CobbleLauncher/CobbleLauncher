// SPDX-License-Identifier: MIT
import { defineComponent, h, ref } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameRuntimeSettings from './GameRuntimeSettings.vue';
import SelectField from './ui/SelectField.vue';
import { setLocale } from './i18n';
import type { Game } from './games';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const initial: Game = { id: 'runtime-game', name: 'Runtime', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'idle', runtime_overrides: { java: { mode: 'inherit' }, memory_mb: null } };
const candidate = { id: 'java-candidate21', version: '21.0.1', major: 21, arch: 'x86_64', vendor: 'Fixture', executable: 'C:\\Java21\\bin\\javaw.exe', recommended: false };
let stored: Game, failMemory: boolean, picker: typeof candidate | null;
const wrappers: ReturnType<typeof mount>[] = [];
beforeEach(() => {
  stored = structuredClone(initial); failMemory = false; picker = candidate; setLocale('en-US');
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} });
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    const value = args as Record<string, unknown>;
    if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'synthetic', custom_game_directory: false };
    if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
    if (command === 'list_game_java_runtimes') return { runtimes: [candidate], requiredMajor: 21, recommendedId: candidate.id };
    if (command === 'pick_game_java_candidate') return picker;
    if (command === 'set_game_java_mode') stored = { ...stored, runtime_overrides: { ...stored.runtime_overrides!, java: { mode: value.mode as 'inherit' | 'automatic' } } };
    if (command === 'set_game_java_runtime') stored = { ...stored, runtime_overrides: { ...stored.runtime_overrides!, java: { mode: 'custom', executable: candidate.executable } } };
    if (command === 'set_game_memory') {
      if (failMemory) throw new Error('synthetic memory failure');
      stored = { ...stored, runtime_overrides: { ...stored.runtime_overrides!, memory_mb: value.memoryMb as number | null } };
    }
    return structuredClone(stored);
  });
});
afterEach(() => { for (const wrapper of wrappers.splice(0)) wrapper.unmount(); vi.mocked(invoke).mockReset(); vi.unstubAllGlobals(); setLocale('zh-CN'); });
function harness(game = initial) {
  const component = defineComponent({ setup() { const value = ref(structuredClone(game)); return () => h(GameRuntimeSettings, { game: value.value, onUpdated: updated => { value.value = updated; } }); } });
  const wrapper = mount(component, { attachTo: document.body }); wrappers.push(wrapper); return wrapper;
}
function field(wrapper: ReturnType<typeof mount>, id: string) { return wrapper.findAllComponents(SelectField).find(item => item.props('id') === id)!; }
async function draft(wrapper: ReturnType<typeof mount>) {
  field(wrapper, 'game-java-mode').vm.$emit('update:modelValue', 'automatic');
  field(wrapper, 'game-memory-mode').vm.$emit('update:modelValue', 'custom'); await flushPromises();
  await wrapper.get('#game-memory').setValue('6145');
}
const mutations = () => vi.mocked(invoke).mock.calls.filter(([command]) => command.startsWith('set_game_'));

it('stages Java and exact memory together with one Apply and sequential existing setters', async () => {
  const wrapper = harness(); await flushPromises(); await draft(wrapper);
  expect(mutations()).toHaveLength(0);
  expect(wrapper.findAll('.basic-runtime-actions')).toHaveLength(1);
  await wrapper.get('.basic-runtime-actions .secondary-button').trigger('click'); await flushPromises();
  expect(mutations().map(([command]) => command)).toEqual(['set_game_java_mode', 'set_game_memory']);
  expect(invoke).toHaveBeenCalledWith('set_game_memory', { id: 'runtime-game', memoryMb: 6145 });
  expect(wrapper.getComponent(GameRuntimeSettings).vm.canLeave()).toBe(true);
  expect(wrapper.find('.basic-runtime-actions').exists()).toBe(false);
});

it('opens the read-only picker immediately, keeps its opaque ID as a draft, and cancels both fields locally', async () => {
  const wrapper = harness(); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Choose installed Java')!.trigger('click');
  await wrapper.findAll('button').find(button => button.text() === 'Choose Java executable')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('pick_game_java_candidate', { id: 'runtime-game' });
  expect(mutations()).toHaveLength(0);
  expect(field(wrapper, 'game-java-mode').props('modelValue')).toBe('custom');
  field(wrapper, 'game-memory-mode').vm.$emit('update:modelValue', 'custom'); await flushPromises(); await wrapper.get('#game-memory').setValue('8192');
  await wrapper.get('.basic-runtime-actions .text-button').trigger('click'); await flushPromises();
  expect(wrapper.getComponent(GameRuntimeSettings).vm.canLeave()).toBe(true);
  expect(mutations()).toHaveLength(0);
  field(wrapper, 'game-java-mode').vm.$emit('update:modelValue', 'automatic'); await flushPromises(); picker = null;
  await wrapper.findAll('button').find(button => button.text() === 'Choose Java executable')!.trigger('click'); await flushPromises();
  expect(field(wrapper, 'game-java-mode').props('modelValue')).toBe('automatic');
});

it('preserves unsaved memory after Java succeeds and retries only the remaining field', async () => {
  const wrapper = harness(); await flushPromises(); await draft(wrapper); failMemory = true;
  await wrapper.get('.basic-runtime-actions .secondary-button').trigger('click'); await flushPromises();
  expect(stored.runtime_overrides?.java.mode).toBe('automatic');
  expect(stored.runtime_overrides?.memory_mb).toBeNull();
  expect((wrapper.get('#game-memory').element as HTMLInputElement).value).toBe('6145');
  expect(wrapper.text()).toContain('Java saved. Memory changes are still unsaved.');
  expect(wrapper.getComponent(GameRuntimeSettings).vm.canLeave()).toBe(false);
  failMemory = false; await wrapper.get('.basic-runtime-actions .secondary-button').trigger('click'); await flushPromises();
  expect(mutations().map(([command]) => command)).toEqual(['set_game_java_mode', 'set_game_memory', 'set_game_memory']);
  expect(wrapper.getComponent(GameRuntimeSettings).vm.canLeave()).toBe(true);
});

it('applies a chosen file through its opaque candidate ID only after Apply', async () => {
  const wrapper = harness(); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Choose installed Java')!.trigger('click');
  await wrapper.findAll('button').find(button => button.text() === 'Choose Java executable')!.trigger('click'); await flushPromises();
  expect(mutations()).toHaveLength(0);
  await wrapper.get('.basic-runtime-actions .secondary-button').trigger('click'); await flushPromises();
  expect(mutations()).toEqual([['set_game_java_runtime', { id: 'runtime-game', runtimeId: candidate.id }]]);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'pick_game_java_executable')).toBe(false);
  expect(wrapper.getComponent(GameRuntimeSettings).vm.canLeave()).toBe(true);
});

it('tracks a valid memory edit before blur so window-close draft guards see it', async () => {
  const game = { ...initial, runtime_overrides: { java: { mode: 'inherit' as const }, memory_mb: 4096 } }; stored = structuredClone(game);
  const wrapper = harness(game); await flushPromises();
  const input = wrapper.get<HTMLInputElement>('#game-memory'); input.element.value = '6145'; await input.trigger('input');
  expect(wrapper.getComponent(GameRuntimeSettings).vm.canLeave()).toBe(false);
  expect(mutations()).toHaveLength(0);
});

it('guards a Java-only draft on leave and applies it through the same group action', async () => {
  const wrapper = harness(); await flushPromises();
  field(wrapper, 'game-java-mode').vm.$emit('update:modelValue', 'automatic'); await flushPromises();
  const runtime = wrapper.getComponent(GameRuntimeSettings); const keep = runtime.vm.confirmLeave(); await flushPromises();
  [...document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')].find(button => button.textContent === 'Keep editing')!.click(); await flushPromises();
  expect(await keep).toBe(false); expect(mutations()).toHaveLength(0);
  const leave = runtime.vm.confirmLeave(); await flushPromises();
  [...document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')].find(button => button.textContent === 'Apply')!.click(); await flushPromises();
  expect(await leave).toBe(true);
  expect(mutations().map(([command]) => command)).toEqual(['set_game_java_mode']);
});

it('keeps runtime mutations locked while the game is running', async () => {
  const running = { ...initial, launch_state: 'running' as const }; stored = structuredClone(running);
  const wrapper = harness(running); await flushPromises();
  expect(field(wrapper, 'game-java-mode').props('disabled')).toBe(true);
  expect(field(wrapper, 'game-memory-mode').props('disabled')).toBe(true);
  field(wrapper, 'game-memory-mode').vm.$emit('update:modelValue', 'custom'); await flushPromises();
  expect(wrapper.get('.basic-runtime-actions .secondary-button').attributes('disabled')).toBeDefined();
  await wrapper.get('.basic-runtime-actions .secondary-button').trigger('click'); await flushPromises();
  expect(mutations()).toHaveLength(0);
});

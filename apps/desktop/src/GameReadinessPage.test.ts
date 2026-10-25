// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
import GameReadinessPage from './GameReadinessPage.vue';
import type { Game } from './games';
import type { GameReadiness, ReadinessPreparation } from './readiness';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

const game = (id = 'game-a'): Game => ({
  id, name: `Game ${id}`, version: '1.21.1', directory: `C:/Games/${id}`,
  status: 'installed', error: null, progress: null,
});

const readiness = (overrides: Partial<GameReadiness> = {}): GameReadiness => ({
  files: { ready: true, checked_files: 12, ready_files: 12, missing_files: 0, changed_files: 0, unknown: 0, issues: [], issues_truncated: false, required_java: 21 },
  java: { state: 'ready', code: 'local_runtime_available' },
  account: { state: 'unknown', code: 'online_auth_not_checked' },
  server: { state: 'unknown', code: 'server_not_checked' },
  ...overrides,
});

const preparation = (overrides: Partial<ReadinessPreparation> = {}): ReadinessPreparation => ({
  space: { can_proceed: true, exact: true, volumes: [] },
  requirements: { ready: true, missing: [] },
  ...overrides,
});

beforeEach(() => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'inspect_game_readiness') return readiness();
    if (command === 'preview_game_readiness') return preparation();
    if (command === 'prepare_game_readiness') return { id: 'task-1', state: 'running' };
    return null;
  });
});
afterEach(() => { setLocale('zh-CN'); vi.clearAllMocks(); });

it('checks only local readiness and keeps an unknown account state unknown when files are ready', async () => {
  const wrapper = mount(GameReadinessPage, { props: { game: game() } });
  await flushPromises();
  expect(invoke).toHaveBeenCalledTimes(1);
  expect(invoke).toHaveBeenCalledWith('inspect_game_readiness', expect.objectContaining({ id: 'game-a', requestId: expect.any(String) }));
  expect(vi.mocked(invoke).mock.calls.map(call => call[0])).toEqual(['inspect_game_readiness']);
  expect(wrapper.findAll('.readiness-condition').find(row => row.text().includes('Player profile'))?.attributes('data-state')).toBe('unknown');
  expect(wrapper.text()).toContain('Current sign-in status has not been verified');
  expect(wrapper.text()).not.toContain('Prepare missing files');
  wrapper.unmount();
});

it('previews before creating a durable task and allows manual required files when space is sufficient', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'inspect_game_readiness'
    ? readiness({ files: { ...readiness().files, ready: false, ready_files: 10, missing_files: 2 } })
    : command === 'preview_game_readiness'
      ? preparation({ requirements: { ready: false, missing: [{ path: 'mods/example.jar' } as never, { path: 'mods/another.jar' } as never] } })
      : { id: 'task-manual', state: 'running' });
  const wrapper = mount(GameReadinessPage, { props: { game: game() } });
  await flushPromises();
  await wrapper.findAll('button').find(button => button.text().includes('Prepare missing files'))!.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_game_readiness', expect.objectContaining({ id: 'game-a', requestId: expect.any(String) }));
  expect(invoke).not.toHaveBeenCalledWith('prepare_game_readiness', expect.anything());
  expect(wrapper.text()).toContain('2 files require manual downloads');
  const start = wrapper.findAll('button').find(button => button.text().includes('Start preparation'))!;
  expect((start.element as HTMLButtonElement).disabled).toBe(false);
  await start.trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('prepare_game_readiness', { id: 'game-a' });
  expect(wrapper.emitted('task')?.[0]?.[0]).toMatchObject({ id: 'task-manual' });
  wrapper.unmount();
});

it('does not start preparation or preview when readiness found changed files', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'inspect_game_readiness'
    ? readiness({ files: { ...readiness().files, ready: false, changed_files: 1, issues: [{ category: 'asset', path: 'assets/example', state: 'changed', size_bytes: null, repairable: false }] } })
    : preparation());
  const wrapper = mount(GameReadinessPage, { props: { game: game() } });
  await flushPromises();
  expect(wrapper.text()).toContain('Some files have changed.');
  const prepare = wrapper.findAll('button').find(button => button.text().includes('Prepare missing files'))!;
  expect((prepare.element as HTMLButtonElement).disabled).toBe(true);
  expect(invoke).not.toHaveBeenCalledWith('preview_game_readiness', expect.anything());
  wrapper.unmount();
});

it('cancels a read-only check and ignores its late result', async () => {
  const pending = deferred<GameReadiness>();
  vi.mocked(invoke).mockImplementation(command => command === 'inspect_game_readiness' ? pending.promise : Promise.resolve(null));
  const wrapper = mount(GameReadinessPage, { props: { game: game() } });
  await flushPromises();
  expect(wrapper.text()).toContain('Checking local files');
  const request = vi.mocked(invoke).mock.calls[0]?.[1] as { requestId: string };
  await wrapper.get('.readiness-busy button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cancel_game_readiness_check', { requestId: request.requestId });
  pending.resolve(readiness({ files: { ...readiness().files, ready_files: 99 } }));
  await flushPromises();
  expect(wrapper.find('.readiness-content').exists()).toBe(false);
  wrapper.unmount();
});

it('cancels the old game check and ignores its result after switching games', async () => {
  const old = deferred<GameReadiness>();
  const current = deferred<GameReadiness>();
  vi.mocked(invoke).mockImplementation((command: string, args?: InvokeArgs) => {
    if (command !== 'inspect_game_readiness') return Promise.resolve(null);
    return (args as Record<string, unknown> | undefined)?.id === 'game-a' ? old.promise : current.promise;
  });
  const wrapper = mount(GameReadinessPage, { props: { game: game('game-a') } });
  await nextTick();
  const request = vi.mocked(invoke).mock.calls[0]?.[1] as { requestId: string };
  await wrapper.setProps({ game: game('game-b') });
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cancel_game_readiness_check', { requestId: request.requestId });
  old.resolve(readiness({ files: { ...readiness().files, ready_files: 99 } }));
  await flushPromises();
  expect(wrapper.find('.readiness-content').exists()).toBe(false);
  current.resolve(readiness({ files: { ...readiness().files, ready_files: 7 } }));
  await flushPromises();
  expect(wrapper.text()).toContain('7 / 12 files ready');
  wrapper.unmount();
});

it('cancels the check on unmount and ignores a late result', async () => {
  const pending = deferred<GameReadiness>();
  vi.mocked(invoke).mockImplementation(command => command === 'inspect_game_readiness' ? pending.promise : Promise.resolve(null));
  const wrapper = mount(GameReadinessPage, { props: { game: game() } });
  await nextTick();
  const request = vi.mocked(invoke).mock.calls[0]?.[1] as { requestId: string };
  wrapper.unmount();
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('cancel_game_readiness_check', { requestId: request.requestId });
  pending.resolve(readiness());
  await flushPromises();
  expect(invoke).toHaveBeenCalledTimes(2);
});

it('keeps an unknown-only file check and custom Java issue in their localized states', async () => {
  vi.mocked(invoke).mockResolvedValue(readiness({
    files: { ...readiness().files, ready: false, checked_files: 12, ready_files: 11, missing_files: 0, unknown: 1, issues: [{ category: 'loader', path: null, state: 'unknown', size_bytes: null, repairable: false }] },
    java: { state: 'missing', code: 'custom_java_unavailable' },
  }));
  const wrapper = mount(GameReadinessPage, { props: { game: game() } });
  await flushPromises();
  const files = wrapper.findAll('.readiness-condition').find(row => row.text().includes('Game files'))!;
  expect(files.attributes('data-state')).toBe('unknown');
  await wrapper.get('.settings-disclosure-trigger').trigger('click');
  expect(wrapper.text()).toContain('Mod loader');
  expect(wrapper.text()).toContain('The selected Java could not be used. Choose a matching runtime.');
  setLocale('zh-CN');
  await flushPromises();
  expect(wrapper.text()).toContain('Mod 加载器');
  expect(wrapper.text()).toContain('所选 Java 无法使用，请选择匹配的运行时。');
  wrapper.unmount();
});


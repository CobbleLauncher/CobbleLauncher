// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import GameDetailsPage from './GameDetailsPage.vue';
import TasksPage from './TasksPage.vue';
import type { Game } from './games';
import type { TaskSnapshot } from './tasks';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const player = { id: 'player', name: 'Builder', kind: 'offline' as const, uuid: 'synthetic-uuid', note: null };
const game: Game = { id: 'game-exchange', name: 'Shared game', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.10', directory: 'synthetic', status: 'installed', error: null, progress: null };
const childStubs = {
  GameModsPanel: true,
  LocalPacks: true,
  GameWorlds: true,
  GameScreenshots: true,
  GameDiagnosticsPanel: true,
  GameAppearancePanel: true,
  InstanceExtensionSettings: true,
  KeybindingTransfer: true,
  GameRuntimeSettings: true,
  GameConfigsPanel: true,
  ExtensionMenuItems: true,
};

beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockResolvedValue(null); });
afterEach(() => { setLocale('zh-CN'); });

it('emits distinct share, apply, and transfer actions from the game menu', async () => {
  const wrapper = mount(GameDetailsPage, {
    props: { game, starting: false, profile: player, library: { profiles: [player], active_id: player.id } },
    global: { stubs: childStubs },
    attachTo: document.body,
  });
  try {
    for (const [label, mode] of [
      ['Share changes…', 'export'],
      ['Apply shared changes…', 'import'],
      ['Continue on another computer…', 'transfer'],
    ] as const) {
      await wrapper.get('.game-more-trigger').trigger('click'); await flushPromises();
      const item = [...document.body.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(element => element.textContent?.trim() === label);
      expect(item, `menu item ${label}`).toBeTruthy();
      item!.click(); await flushPromises();
      expect(wrapper.emitted('exchange')?.at(-1)).toEqual([mode]);
    }
    expect(wrapper.emitted('exchange')).toEqual([['export'], ['import'], ['transfer']]);

    setLocale('zh-CN'); await flushPromises();
    for (const [label, mode] of [
      ['分享改动…', 'export'],
      ['应用共享改动…', 'import'],
      ['换电脑继续玩…', 'transfer'],
    ] as const) {
      await wrapper.get('.game-more-trigger').trigger('click'); await flushPromises();
      const item = [...document.body.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(element => element.textContent?.trim() === label);
      expect(item, `menu item ${label}`).toBeTruthy();
      item!.click(); await flushPromises();
      expect(wrapper.emitted('exchange')?.at(-1)).toEqual([mode]);
    }
    expect(wrapper.emitted('exchange')).toEqual([['export'], ['import'], ['transfer'], ['export'], ['import'], ['transfer']]);
  } finally { wrapper.unmount(); }
});

it('keeps transfer available for vanilla games while hiding mod exchange actions', async () => {
  const vanilla: Game = { ...game, id: 'vanilla-game', loader_kind: null, loader_version: null };
  const wrapper = mount(GameDetailsPage, {
    props: { game: vanilla, starting: false, profile: player, library: { profiles: [player], active_id: player.id } },
    global: { stubs: childStubs },
    attachTo: document.body,
  });
  try {
    await wrapper.get('.game-more-trigger').trigger('click'); await flushPromises();
    const labels = [...document.body.querySelectorAll<HTMLElement>('[role="menuitem"]')].map(element => element.textContent?.trim());
    expect(labels).not.toContain('Share changes…');
    expect(labels).not.toContain('Apply shared changes…');
    expect(labels).toContain('Continue on another computer…');
  } finally { wrapper.unmount(); }
});

it('disables all exchange menu actions during a running game', async () => {
  const wrapper = mount(GameDetailsPage, {
    props: { game, starting: false, profile: player, library: { profiles: [player], active_id: player.id } },
    global: { stubs: childStubs },
    attachTo: document.body,
  });
  try {
    await wrapper.get('.game-more-trigger').trigger('click'); await flushPromises();
    await wrapper.setProps({ game: { ...game, launch_state: 'running' } }); await flushPromises();
    const labels = ['Share changes…', 'Apply shared changes…', 'Continue on another computer…'];
    const items = labels.map(label => [...document.body.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(element => element.textContent?.trim() === label));
    expect(items.every(Boolean)).toBe(true);
    for (const item of items) {
      expect(item!.getAttribute('aria-disabled')).toBe('true');
      item!.click();
    }
    await flushPromises();
    expect(wrapper.emitted('exchange')).toBeUndefined();
    expect(wrapper.get('.game-more-trigger').attributes('disabled')).toBeDefined();
  } finally { wrapper.unmount(); }
});

it('shows receiving byte progress and opens the exact transfer task in English and Chinese', async () => {
  const megabyte = 1024 ** 2;
  const task: TaskSnapshot = {
    id: 'transfer-task-93', game_id: null, game_name: '', title: 'Transfer fixture', kind: 'game_transfer', state: 'running',
    progress: { phase: 'receiving', completed: 14 * megabyte, total: 38 * megabyte, reused: 0, detail: {
      bytesCompleted: 14 * megabyte, bytesTotal: 38 * megabyte, bytesReceived: 14 * megabyte, bytesPerSecond: 1536 * 1024, activeDownloads: 2,
      items: [{ source: 'official', item: 'game-assets.zip', bytesReceived: 6 * megabyte, bytesTotal: 20 * megabyte }],
    } },
    error: null, can_retry: false, created_at: 1, updated_at: 2, has_result: false,
  };
  const wrapper = mount(TasksPage, { props: { tasks: [task], loading: false, error: '' } });
  expect(wrapper.get('.task-row h2').text()).toBe('Receive game');
  expect(wrapper.get('.task-progress-caption').text()).toContain('Receiving game files');
  expect(wrapper.get('.task-progress-caption').text()).toContain('14 MB / 38 MB');
  expect(wrapper.get('.task-transfer-details .settings-disclosure-meta').text()).toContain('1.5 MB/s');
  await wrapper.get('.task-transfer-details .settings-disclosure-trigger').trigger('click'); await flushPromises();
  expect(wrapper.get('.task-transfer-details').text()).toContain('game-assets.zip');
  expect(wrapper.get('.task-transfer-details').text()).toContain('6 MB / 20 MB');
  await wrapper.findAll('button').find(button => button.text().trim() === 'View transfer')!.trigger('click');
  expect(wrapper.emitted('open-transfer')).toEqual([['transfer-task-93']]);

  setLocale('zh-CN'); await flushPromises();
  expect(wrapper.get('.task-row h2').text()).toBe('接收游戏');
  expect(wrapper.get('.task-progress-caption').text()).toContain('正在接收游戏文件');
  expect(wrapper.findAll('button').find(button => button.text().trim() === '查看传输')).toBeTruthy();
  wrapper.unmount();
});

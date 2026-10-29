// SPDX-License-Identifier: MIT
/* eslint-disable vue/one-component-per-file -- Small mount fixtures exercise composable lifecycle. */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { defineComponent, h, ref } from 'vue';
import { mount } from '@vue/test-utils';
import { commandRegistry, listExtensionActions, preparePinyinSearch, searchCommands } from './registry';
import { clearShortcut, normalizeShortcut, readShortcuts, resetShortcuts, saveShortcut, shortcutAction, shortcutFromEvent } from './shortcuts';
import { useShellCommands } from './use-shell-commands';
import type { Game } from '../games';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('command registry', () => {
  it('uses stable ids and filters disabled commands', () => {
    const run = vi.fn();
    const commands = commandRegistry([{ id: 'navigation.library', label: 'Library', run }], [{ id: 'navigation.library', label: 'Duplicate', run }, { id: 'game.launch', label: 'Launch game', enabled: false, run }]);
    expect(commands.map(command => command.label)).toEqual(['Library', 'Launch game']);
    expect(searchCommands(commands, 'launch')).toEqual([]);
    expect(searchCommands(commands, 'LIB')).toEqual([commands[0]]);
  });

  it('ranks exact labels ahead of prefixes and pinned results on an empty query', () => {
    const run = vi.fn();
    const actions = [{ id: 'keyword', label: 'Browse', keywords: ['play'], run }, { id: 'prefix', label: 'Play game', run }, { id: 'exact', label: 'Play', run }];
    expect(searchCommands(actions, 'play').map(action => action.id)).toEqual(['exact', 'prefix', 'keyword']);
    expect(searchCommands([{ ...actions[0]!, pinned: true }, ...actions.slice(1)], '')[0]?.id).toBe('keyword');
    expect(searchCommands([{ ...actions[1]!, pinned: true, recentAt: Date.now() }, actions[2]!], 'play')[0]?.id).toBe('exact');
  });

  it('matches Chinese command labels by original text, full pinyin, and initials', async () => {
    const run = vi.fn();
    const actions = [
      { id: 'launch', label: '启动游戏', run },
      { id: 'settings', label: '设置', run },
      { id: 'redstone', label: '红石工坊', run },
    ];
    expect(searchCommands(actions, '启动')).toEqual([actions[0]]);
    expect(await preparePinyinSearch()).toBe(true);
    expect(searchCommands(actions, 'qidongyouxi')).toEqual([actions[0]]);
    expect(searchCommands(actions, 'qdyx')).toEqual([actions[0]]);
    expect(searchCommands(actions, 'shezhi')).toEqual([actions[1]]);
    expect(searchCommands(actions, 'sz')).toEqual([actions[1]]);
    expect(searchCommands(actions, 'hsg')).toEqual([actions[2]]);
    expect(searchCommands(actions, 'hsgf')).toEqual([actions[2]]);
  });

  it('matches plugin names and search keywords by original text and pinyin', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{
      pluginId: 'planner', pluginName: '材料规划', commands: [{
        id: 'materials', label: '列出材料', searchKeywords: ['蓝图资源', 'crafting'],
        menus: [{ position: 'tools' }],
      }],
    }]);
    const actions = await listExtensionActions('tools', undefined, vi.fn());
    await preparePinyinSearch();
    expect(searchCommands(actions, '材料')).toEqual(actions);
    expect(searchCommands(actions, 'clgh')).toEqual(actions);
    expect(searchCommands(actions, 'lantuziyuan')).toEqual(actions);
    expect(searchCommands(actions, 'ltzy')).toEqual(actions);
    expect(searchCommands(actions, 'crafting')).toEqual(actions);
    expect(searchCommands(actions, 'planner')).toEqual(actions);
  });

  it('lists native plugin actions and resolves again before execution', async () => {
    const resolved = { pluginId: 'viewer', commandId: 'open', label: 'Open', workflow: 'viewer' };
    vi.mocked(invoke).mockImplementation(async command => command === 'list_extension_contributions'
      ? [{ pluginId: 'viewer', pluginName: 'Viewer', commands: [{ id: 'open', label: 'Open', menus: [{ position: 'tools' }] }] }]
      : resolved);
    const selected = vi.fn();
    const actions = await listExtensionActions('tools', undefined, selected);
    expect(actions).toHaveLength(1);
    expect(actions[0]?.id).toBe('plugin:viewer:open');
    await actions[0]!.run();
    expect(invoke).toHaveBeenLastCalledWith('resolve_extension_command', expect.objectContaining({ pluginId: 'viewer', commandId: 'open', position: 'tools' }));
    expect(selected).toHaveBeenCalledWith(resolved);
  });
  it('uses optional plugin discovery metadata while keeping native resolution', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{ pluginId: 'command-pocket', pluginName: 'Command Pocket', commands: [{ id: 'command-pocket.run', label: 'Build a command', workflow: 'tool_form', icon: 'search', searchKeywords: ['teleport'], quickEntry: true, suggestedShortcut: 'Ctrl+Shift+C', menus: [{ position: 'tools' }] }] }]);
    const actions = await listExtensionActions('tools', undefined, vi.fn());
    expect(actions[0]).toMatchObject({ id: 'plugin:command-pocket:command-pocket.run', icon: 'search', quickEntry: true, suggestedShortcut: 'Ctrl+Shift+C' });
    expect(searchCommands(actions, 'teleport')).toHaveLength(1);
  });
  it('ignores an extension result after its context becomes stale', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{ pluginId: 'p', pluginName: 'P', commands: [{ id: 'open', label: 'Open', menus: [{ position: 'tools' }] }] }]);
    expect(await listExtensionActions('tools', undefined, vi.fn(), () => false)).toEqual([]);
  });
});

describe('shortcuts', () => {
  beforeEach(() => { localStorage.clear(); resetShortcuts(); });
  it('normalizes standard modified keys and rejects bare or reserved keys', () => {
    for (const [input, expected] of [
      ['ctrl+shift+p', 'Ctrl+Shift+P'], ['ctrl+F5', 'Ctrl+F5'], ['alt+arrowright', 'Alt+ArrowRight'],
      ['Ctrl+Home', 'Ctrl+Home'], ['Ctrl+End', 'Ctrl+End'], ['Ctrl+PageUp', 'Ctrl+PageUp'],
      ['Ctrl+PageDown', 'Ctrl+PageDown'], ['Ctrl+Delete', 'Ctrl+Delete'], ['Ctrl+Backspace', 'Ctrl+Backspace'],
    ]) expect(normalizeShortcut(input!)).toBe(expected);
    expect(normalizeShortcut('F5')).toBeNull();
    expect(normalizeShortcut('Ctrl+Escape')).toBeNull();
    expect(shortcutFromEvent(new KeyboardEvent('keydown', { key: 'PageDown', ctrlKey: true }))).toBe('Ctrl+PageDown');
  });
  it('uses defaults for valid JSON values that are not shortcut objects', () => {
    for (const value of ['null', '[]', '42', '"text"']) {
      localStorage.setItem('cobble-shortcuts-v1', value);
      expect(readShortcuts()['palette.open']).toBe('Ctrl+K');
    }
  });
  it('saves overrides, rejects collisions, and resets', () => {
    expect(saveShortcut('palette.open', 'ctrl+f')).toBe('conflict');
    expect(saveShortcut('palette.open', 'Meta+P')).toBe('saved');
    expect(readShortcuts()['palette.open']).toBe('Meta+P');
    expect(saveShortcut('settings.open', 'Ctrl+,')).toBe('saved');
    expect(readShortcuts()['settings.open']).toBe('Ctrl+,');
    expect(shortcutFromEvent(new KeyboardEvent('keydown', { key: ',', ctrlKey: true }))).toBe('Ctrl+,');
    clearShortcut('palette.open');
    expect(readShortcuts()['palette.open']).toBeNull();
    resetShortcuts();
    expect(readShortcuts()['palette.open']).toBe('Ctrl+K');
  });
  it('accepts a custom action binding and detects a collision with a built-in action', () => {
    expect(saveShortcut('plugin:viewer:open', 'Ctrl+Shift+V')).toBe('saved');
    expect(readShortcuts()['plugin:viewer:open']).toBe('Ctrl+Shift+V');
    expect(saveShortcut('plugin:viewer:open', 'Ctrl+K')).toBe('conflict');
  });
  it('allows explicit search in editable targets while preserving IME and modal priority', () => {
    const input = document.createElement('input');
    const typed = new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, bubbles: true });
    Object.defineProperty(typed, 'target', { value: input });
    expect(shortcutAction(typed, readShortcuts())).toBe('library.search');
    expect(shortcutAction(typed, readShortcuts(), { modalOpen: true })).toBeNull();
    const palette = new KeyboardEvent('keydown', { key: 'k', ctrlKey: true });
    Object.defineProperty(palette, 'target', { value: input });
    expect(shortcutAction(palette, readShortcuts())).toBe('palette.open');
    expect(shortcutAction(palette, readShortcuts(), { modalOpen: true })).toBeNull();
    expect(shortcutFromEvent(new KeyboardEvent('keydown', { key: 'Process', ctrlKey: true }))).toBeNull();
  });
  it('does not swallow a saved binding when its plugin is unavailable', () => {
    expect(saveShortcut('plugin:disabled:run', 'Ctrl+Shift+P')).toBe('saved');
    let handle: ((event: KeyboardEvent) => void) | undefined;
    const component = defineComponent({ setup() {
      handle = useShellCommands({
        games: ref([]), page: ref({ view: 'library' }), selectedGameId: ref(''), busy: ref(false), ready: ref(false),
        navigate: vi.fn(), openGame: vi.fn(), launchGame: vi.fn(), searchGames: vi.fn(), back: vi.fn(), openSettings: vi.fn(), openExtension: vi.fn(), onGameUpdated: vi.fn(),
      }).handleKeydown;
      return () => h('div');
    } });
    const wrapper = mount(component);
    const event = new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, shiftKey: true, cancelable: true });
    handle!(event);
    expect(event.defaultPrevented).toBe(false);
    wrapper.unmount();
  });
  it('shows one selected-game management entry', () => {
    const game = { id: 'g', name: 'Fixture', status: 'installed', launch_state: 'idle', pinned: false } as Game;
    let actions: { value: { id: string }[] } | undefined;
    const component = defineComponent({ setup() {
      actions = useShellCommands({
        games: ref([game]), page: ref({ view: 'library' }), selectedGameId: ref('g'), busy: ref(false), ready: ref(false),
        navigate: vi.fn(), openGame: vi.fn(), launchGame: vi.fn(), searchGames: vi.fn(), back: vi.fn(), openSettings: vi.fn(), openExtension: vi.fn(), onGameUpdated: vi.fn(),
      }).commands;
      return () => h('div');
    } });
    const wrapper = mount(component);
    expect(actions!.value.map(action => action.id).filter(id => id === 'game.open:g' || id === 'game.manage')).toEqual(['game.open:g']);
    wrapper.unmount();
  });
  it('does not launch a game or open the palette behind a destructive confirmation', () => {
    const launch = vi.fn();
    let shell: ReturnType<typeof useShellCommands>;
    const component = defineComponent({ setup() {
      shell = useShellCommands({
        games: ref([{ id: 'g', name: 'Game', status: 'installed', launch_state: 'idle' } as Game]), page: ref({ view: 'details', gameId: 'g' }), selectedGameId: ref('g'), busy: ref(false), ready: ref(false),
        navigate: vi.fn(), openGame: vi.fn(), launchGame: launch, searchGames: vi.fn(), back: vi.fn(), openSettings: vi.fn(), openExtension: vi.fn(), onGameUpdated: vi.fn(),
      });
      return () => h('div');
    } });
    const wrapper = mount(component);
    const dialog = document.createElement('div'); dialog.setAttribute('role', 'alertdialog'); dialog.setAttribute('aria-modal', 'true'); document.body.append(dialog);
    try {
      shell!.handleKeydown(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, cancelable: true }));
      shell!.handleKeydown(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, cancelable: true }));
      expect(launch).not.toHaveBeenCalled(); expect(shell!.paletteOpen.value).toBe(false);
      dialog.remove(); shell!.handleKeydown(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, cancelable: true })); expect(launch).toHaveBeenCalledWith('g');
    } finally { dialog.remove(); wrapper.unmount(); }
  });
});

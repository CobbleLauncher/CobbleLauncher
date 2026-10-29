// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameConfigsPanel from './GameConfigsPanel.vue';
import type { Game } from './games';
import { configTextBytes, MAX_CONFIG_BYTES, type GameConfigDocument } from './game-configs';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const game: Game = { id: 'game', name: 'Game', version: '1.21.1', directory: 'synthetic', status: 'installed', progress: null, error: null, launch_state: 'idle' };
const files = [{ id: 'config-a', name: 'mod/settings.toml', size: 12 }, { id: 'config-b', name: 'other.json', size: 2 }];
const original: GameConfigDocument = { id: 'config-a', name: files[0]!.name, text: 'enabled=true', revision: 'revision-1', backups: [{ id: 'backup-a', createdAt: 1700000000123 }] };
function nativeRead(command: string, args?: unknown) {
  if (command === 'list_game_configs') return files;
  if (command === 'read_game_config') return (args as { configId: string }).configId === 'config-a' ? structuredClone(original) : { ...original, id: 'config-b', name: 'other.json', text: '{}', revision: 'revision-b' };
  return null;
}
function dialogButton(text: string) { return [...document.body.querySelectorAll<HTMLButtonElement>('.config-dialog button')].find(button => button.textContent?.trim() === text)!; }
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockImplementation(async (command, args) => nativeRead(command, args)); });
afterEach(() => { document.body.innerHTML = ''; setLocale('zh-CN'); });

it('searches full file names and saves only the selected native ID with its revision and a truthful pending lock', async () => {
  let finish!: (value: GameConfigDocument) => void;
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'save_game_config' ? new Promise(resolve => { finish = resolve; }) : nativeRead(command, args));
  const wrapper = mount(GameConfigsPanel, { props: { game } }); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_game_configs', { gameId: 'game' });
  await wrapper.get('input[type="search"]').setValue('settings.toml');
  expect(wrapper.findAll('.config-file')).toHaveLength(1);
  await wrapper.get('.config-file').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('read_game_config', { gameId: 'game', configId: 'config-a' });
  await wrapper.get('textarea').setValue('enabled=false');
  await wrapper.get('.config-actions .secondary-button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('save_game_config', { gameId: 'game', configId: 'config-a', text: 'enabled=false', expectedRevision: 'revision-1' });
  expect(wrapper.emitted('busy')).toEqual([[true]]);
  expect(wrapper.find('.config-actions .ui-busy-icon').exists()).toBe(true);
  expect((wrapper.vm as unknown as { canLeave(): boolean }).canLeave()).toBe(false);
  expect(wrapper.get('textarea').attributes('readonly')).toBeDefined();
  finish({ ...original, text: 'enabled=false', revision: 'revision-2' }); await flushPromises();
  expect(wrapper.emitted('busy')).toEqual([[true], [false]]);
  expect((wrapper.vm as unknown as { canLeave(): boolean }).canLeave()).toBe(true);
  expect(wrapper.text()).toContain('Saved'); wrapper.unmount();
});

it('keeps rejected conflict edits and requires an explicit revert instead of reporting success', async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => { if (command === 'save_game_config') throw 'CONFIG_CHANGED'; return nativeRead(command, args); });
  const wrapper = mount(GameConfigsPanel, { props: { game } }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises(); await wrapper.get('textarea').setValue('local edit');
  await wrapper.get('.config-actions .secondary-button').trigger('click'); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('changed outside the launcher');
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('local edit');
  expect((wrapper.vm as unknown as { canLeave(): boolean }).canLeave()).toBe(false);
  expect(wrapper.find('.config-editor [role="status"]').exists()).toBe(false);
  await wrapper.get('.config-actions .text-button').trigger('click');
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe(original.text);
  wrapper.unmount();
});

it('locks writes while running or parent-locked while keeping inspection and unsaved text', async () => {
  const wrapper = mount(GameConfigsPanel, { props: { game } }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises(); await wrapper.get('textarea').setValue('draft');
  await wrapper.setProps({ game: { ...game, launch_state: 'running' } });
  expect(wrapper.get('textarea').attributes('readonly')).toBeDefined();
  expect(wrapper.get('.config-actions .secondary-button').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.config-backups button').attributes('disabled')).toBeDefined();
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('draft');
  await wrapper.setProps({ game, locked: true });
  expect(wrapper.get('.config-actions .secondary-button').attributes('disabled')).toBeDefined();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'save_game_config')).toBe(false);
  wrapper.unmount();
});

it('guards file changes with Save, Discard and Keep editing', async () => {
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises(); await wrapper.get('textarea').setValue('draft');
  void wrapper.findAll('.config-file')[1]!.trigger('click'); await flushPromises();
  expect(dialogButton('Save')).toBeDefined(); dialogButton('Keep editing').click(); await flushPromises();
  expect(wrapper.get('.config-editor h2').text()).toBe(original.name);
  void wrapper.findAll('.config-file')[1]!.trigger('click'); await flushPromises(); dialogButton('Discard changes').click(); await flushPromises();
  expect(wrapper.get('.config-editor h2').text()).toBe('other.json');
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('{}');
  wrapper.unmount();
});

it('restores a selected backup only after confirmation and keeps revision checks scoped to that file', async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'restore_game_config_backup' ? { ...original, text: 'older=true', revision: 'revision-restored' } : nativeRead(command, args));
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises();
  await wrapper.get('.config-backups select').setValue('backup-a'); await wrapper.get('.config-backups button').trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'restore_game_config_backup')).toBe(false);
  expect(document.body.textContent).toContain('backed up before replacement');
  dialogButton('Restore').click(); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('restore_game_config_backup', { gameId: 'game', configId: 'config-a', backupId: 'backup-a', expectedRevision: 'revision-1' });
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('older=true');
  wrapper.unmount();
});

it('ignores a stale file response after changing game context and counts UTF-8 size', async () => {
  let finish!: (value: GameConfigDocument) => void;
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'read_game_config' ? new Promise(resolve => { finish = resolve; }) : nativeRead(command, args));
  const wrapper = mount(GameConfigsPanel, { props: { game } }); await flushPromises();
  void wrapper.get('.config-file').trigger('click'); await flushPromises();
  await wrapper.setProps({ game: { ...game, id: 'other-game' } }); await flushPromises(); finish(original); await flushPromises();
  expect(wrapper.find('textarea').exists()).toBe(false);
  expect(invoke).toHaveBeenLastCalledWith('list_game_configs', { gameId: 'other-game' });
  expect(configTextBytes('🙂')).toBe(4);
  expect(configTextBytes('🙂'.repeat(MAX_CONFIG_BYTES / 4 + 1))).toBeGreaterThan(MAX_CONFIG_BYTES);
  wrapper.unmount();
});


it('does not report a failed backup restore as success or replace the editor optimistically', async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => { if (command === 'restore_game_config_backup') throw 'CONFIG_CHANGED'; return nativeRead(command, args); });
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises();
  await wrapper.get('.config-backups select').setValue('backup-a'); await wrapper.get('.config-backups button').trigger('click'); await flushPromises();
  dialogButton('Restore').click(); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('changed outside the launcher');
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe(original.text);
  expect(wrapper.find('.config-editor [role="status"]').exists()).toBe(false);
  expect(wrapper.emitted('busy')).toEqual([[true], [false]]);
  wrapper.unmount();
});


it('keeps the leave prompt open with the actual save failure and preserves the draft', async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => { if (command === 'save_game_config') throw 'CONFIG_CHANGED'; return nativeRead(command, args); });
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises(); await wrapper.get('textarea').setValue('unsaved');
  const decision = (wrapper.vm as unknown as { confirmLeave(): Promise<boolean> }).confirmLeave(); await flushPromises();
  dialogButton('Save').click(); await flushPromises();
  expect(document.body.querySelector('.config-dialog [role="alert"]')?.textContent).toContain('changed outside the launcher');
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('unsaved');
  dialogButton('Keep editing').click(); await flushPromises(); expect(await decision).toBe(false);
  wrapper.unmount();
});

it('opens a searched file with Enter, honors IME and focuses the editor without clearing the search', async () => {
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  const search = wrapper.get('input[type="search"]');
  await search.trigger('compositionstart'); await search.setValue('other.json');
  await search.trigger('keydown', { key: 'Enter', isComposing: true }); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'read_game_config')).toBe(false);
  await search.trigger('compositionend'); await search.trigger('keydown', { key: 'Enter' }); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('read_game_config', { gameId: 'game', configId: 'config-b' });
  expect(document.activeElement).toBe(wrapper.get('textarea').element);
  expect((search.element as HTMLInputElement).value).toBe('other.json');
  await wrapper.get('textarea').setValue('kept draft'); (search.element as HTMLInputElement).focus();
  await search.trigger('keydown', { key: 'Escape' }); await flushPromises();
  expect((search.element as HTMLInputElement).value).toBe('');
  expect(wrapper.findAll('.config-file')).toHaveLength(2);
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('kept draft');
  wrapper.unmount();
});

it('keeps the existing dirty-file decision when opening a search result with Enter', async () => {
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises(); await wrapper.get('textarea').setValue('keep me');
  const search = wrapper.get('input[type="search"]'); await search.setValue('other.json');
  await search.trigger('keydown', { key: 'Enter' }); await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('read_game_config', { gameId: 'game', configId: 'config-b' });
  dialogButton('Keep editing').click(); await flushPromises();
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('keep me');
  await search.trigger('keydown', { key: 'Enter' }); await flushPromises();
  dialogButton('Discard changes').click(); await flushPromises();
  expect(wrapper.get('.config-editor h2').text()).toBe('other.json');
  expect(document.activeElement).toBe(wrapper.get('textarea').element);
  wrapper.unmount();
});

it('saves with Ctrl/Cmd+S, keeps a failed draft retryable and prevents overlapping shortcut saves', async () => {
  let finish!: (value: GameConfigDocument) => void, attempts = 0;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command !== 'save_game_config') return nativeRead(command, args);
    if (++attempts === 1) throw 'CONFIG_IO';
    return new Promise(resolve => { finish = resolve; });
  });
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises(); await wrapper.get('textarea').setValue('shortcut draft');
  const key = new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true, cancelable: true });
  wrapper.get('textarea').element.dispatchEvent(key); await flushPromises();
  expect(key.defaultPrevented).toBe(true); expect(wrapper.get('[role="alert"]').text()).toContain('Your edits are kept');
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('shortcut draft');
  await wrapper.get('textarea').trigger('keydown', { key: 'S', metaKey: true }); await flushPromises();
  await wrapper.get('textarea').trigger('keydown', { key: 's', ctrlKey: true }); await flushPromises();
  expect(attempts).toBe(2);
  finish({ ...original, text: 'shortcut draft', revision: 'shortcut-saved' }); await flushPromises();
  expect(wrapper.text()).toContain('Saved');expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect((wrapper.vm as unknown as { canLeave(): boolean }).canLeave()).toBe(true);wrapper.unmount();
});

it('ignores save shortcuts for IME, repeats, Save As, clean text and running games', async () => {
  const wrapper = mount(GameConfigsPanel, { props: { game } }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises();
  await wrapper.get('textarea').trigger('keydown', { key: 's', ctrlKey: true });
  await wrapper.get('textarea').setValue('draft');
  await wrapper.get('textarea').trigger('keydown', { key: 's', ctrlKey: true, isComposing: true });
  await wrapper.get('textarea').trigger('keydown', { key: 's', ctrlKey: true, repeat: true });
  await wrapper.get('textarea').trigger('keydown', { key: 's', ctrlKey: true, shiftKey: true });
  await wrapper.setProps({ game: { ...game, launch_state: 'running' } });
  await wrapper.get('textarea').trigger('keydown', { key: 's', metaKey: true }); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'save_game_config')).toBe(false);
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('draft');wrapper.unmount();
});

it('uses the same save-and-leave flow for a shortcut inside the unsaved changes dialog', async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'save_game_config' ? { ...original, text: 'saved on exit', revision: 'revision-2' } : nativeRead(command, args));
  const wrapper = mount(GameConfigsPanel, { props: { game }, attachTo: document.body }); await flushPromises();
  await wrapper.get('.config-file').trigger('click'); await flushPromises(); await wrapper.get('textarea').setValue('saved on exit');
  await wrapper.findAll('.config-file')[1]!.trigger('click'); await flushPromises();
  dialogButton('Save').dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true, cancelable: true })); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('save_game_config', { gameId: 'game', configId: 'config-a', text: 'saved on exit', expectedRevision: 'revision-1' });
  expect(wrapper.get('.config-editor h2').text()).toBe('other.json');expect(document.body.querySelector('.config-dialog')).toBeNull();wrapper.unmount();
});

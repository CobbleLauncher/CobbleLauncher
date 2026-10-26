// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import type { Game } from '../games';
import { setLocale } from '../i18n';
import ChangeSharePage from './ChangeSharePage.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const game: Game = { id: 'one', name: 'Birch', version: '1.21.1', directory: 'C:/Birch', status: 'installed', error: null, progress: null, launch_state: 'idle' };
beforeEach(() => { vi.mocked(invoke).mockReset(); setLocale('en-US'); });

it('exports only reviewed selections and keeps configs unselected by default', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'preview_game_change_export'
    ? { revision: 'r1', mods: [{ projectId: 'a', versionId: 'v1', name: 'Apple', sha1: 'hash', size: 3 }], configs: [{ id: 'config/a.json', name: 'a.json', size: 20, text: '{"token":"private"}' }], excludedMods: ['Local.jar'] }
    : { saved: true, mods: 1, configs: 0 });
  const wrapper = mount(ChangeSharePage, { props: { game, mode: 'export' } });
  await flushPromises();
  expect(wrapper.text()).toContain('Local.jar');
  expect((wrapper.findAll('input[type="checkbox"]')[0]!.element as HTMLInputElement).checked).toBe(false);
  expect((wrapper.findAll('input[type="checkbox"]')[1]!.element as HTMLInputElement).checked).toBe(false);
  await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true);
  await wrapper.find('.change-share-footer .primary-button').trigger('click');
  expect(wrapper.text()).toContain('Apple');
  expect(wrapper.text()).not.toContain('a.json');
  await wrapper.find('.change-share-footer .primary-button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('export_game_change', { id: 'one', revision: 'r1', modIds: ['a'], configIds: [] });
  expect(wrapper.text()).toContain('List saved');
});

it('shows selected config text before saving', async () => {
  vi.mocked(invoke).mockResolvedValue({ revision: 'r1', mods: [], configs: [{ id: 'config/a.json', name: 'a.json', size: 20, text: '{"secret":"review me"}' }], excludedMods: [] });
  const wrapper = mount(ChangeSharePage, { props: { game, mode: 'export' } });
  await flushPromises();
  await wrapper.find('input[type="checkbox"]').setValue(true);
  await wrapper.find('.change-share-footer .primary-button').trigger('click');
  expect(wrapper.find('.change-share-review-group pre').text()).toContain('review me');
});

it('previews target differences and sends explicit config choices', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_games') return [game];
    if (command === 'import_game_change') return { importId: 'import-1', document: { name: 'Friends', gameVersion: '1.21.1', mods: [], configs: [] } };
    if (command === 'preview_game_change_apply') return { planId: 'plan-1', changes: [{ projectId: 'a', name: 'Apple', fromVersion: null, toVersion: 'v2', filename: 'a.jar', enabled: true }], skipped: [], configs: [{ path: 'config/a.json', currentText: 'old', incomingText: 'new', currentRevision: 'r1', choice: 'keep' }], additionalLocalMods: 1 };
    return { updated: 0, added: 1, configsWritten: 1, configBackups: [], operationId: 'operation-1', configError: null, complete: true };
  });
  const wrapper = mount(ChangeSharePage, { props: { game, mode: 'import' } });
  await flushPromises();
  await wrapper.find('.change-share-field .secondary-button').trigger('click');
  await flushPromises();
  await wrapper.find('.change-share-footer .primary-button').trigger('click');
  await flushPromises();
  expect(wrapper.text()).toContain('Other local Mods stay');
  await wrapper.find('[role="radio"][data-state="unchecked"]').trigger('click');
  await wrapper.find('.change-share-footer .primary-button').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('apply_game_change', { id: 'one', planId: 'plan-1', configChoices: [{ path: 'config/a.json', choice: 'adopt' }] });
  expect(wrapper.emitted('applied')).toBeUndefined();
  await wrapper.find('.change-share-footer .primary-button').trigger('click');
  await flushPromises();
  expect(wrapper.emitted('applied')?.[0]?.[0]).toBe('one');
  expect(wrapper.emitted('applied')?.[0]?.[1]).toMatchObject({ updated: 0, added: 1, configsWritten: 1, complete: true });
});

it('keeps a partial apply visible and restores committed changes', async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_games') return [game];
    if (command === 'import_game_change') return { importId: 'import-1', document: { name: 'Friends', gameVersion: '1.21.1', mods: [], configs: [] } };
    if (command === 'preview_game_change_apply') return { planId: 'plan-1', changes: [], skipped: [], configs: [{ path: 'config/a.json', currentText: 'old', incomingText: 'new', currentRevision: 'r1', choice: 'keep' }], additionalLocalMods: 0 };
    if (command === 'apply_game_change') return { updated: 0, added: 1, configsWritten: 1, configBackups: [{ path: 'config/a.json', configId: 'config-id', backupId: 'backup-id', newRevision: 'r2', undoId: null }], operationId: 'operation-1', configError: 'Config changed', complete: false };
    return undefined;
  });
  const wrapper = mount(ChangeSharePage, { props: { game, mode: 'import' } });
  await flushPromises();
  await wrapper.find('.change-share-field .secondary-button').trigger('click'); await flushPromises();
  await wrapper.find('.change-share-footer .primary-button').trigger('click'); await flushPromises();
  await wrapper.find('.change-share-footer .primary-button').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('Some changes were applied');
  expect(wrapper.emitted('applied')).toBeUndefined();
  await wrapper.findAll('.change-share-partial button')[0]!.trigger('click'); await flushPromises();
  await wrapper.findAll('.change-share-partial button')[0]!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('undo_game_mod_operation', { id: 'one', operationId: 'operation-1' });
  expect(invoke).toHaveBeenCalledWith('restore_game_config_backup', { gameId: 'one', configId: 'config-id', backupId: 'backup-id', expectedRevision: 'r2' });
});

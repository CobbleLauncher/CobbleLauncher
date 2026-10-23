// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import ImportPackPage from './ImportPackPage.vue';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));

afterEach(() => {
  vi.mocked(invoke).mockReset();
  setLocale('zh-CN');
});

it('keeps preview, install arguments, and CurseForge routing while copy changes locale', async () => {
  setLocale('en-US');
  const preview = {
    preview_id: 'pack-preview-1', name: 'Example pack', version_id: '1.0.0', summary: 'A small test pack',
    minecraft_version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', required_files: 2,
    optional_files: [{ path: 'resourcepacks/example.zip', size: 1024 }], overrides: 3,
  };
  const game = { id: 'created-game', name: 'Imported pack', version: '1.21.1', directory: 'synthetic', status: 'installing', progress: null, error: null };
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
    if (command === 'default_game_directory') return 'C:/Games';
    if (command === 'preview_mrpack') return preview;
    if (command === 'create_mrpack_game') return game;
    return null;
  });
  const wrapper = mount(ImportPackPage);
  await flushPromises();
  expect(wrapper.find('.create-description').exists()).toBe(false);
  expect(wrapper.text()).toContain('Import CurseForge ZIP');
  await wrapper.findAll('button').find(button => button.text() === 'Import CurseForge ZIP')!.trigger('click');
  expect(wrapper.emitted('curseforge')).toHaveLength(1);

  await wrapper.get('.pack-picker').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('preview_mrpack');
  expect(wrapper.find('label[for="pack-game-name"]').text()).toBe('Game name');
  expect(wrapper.get('.pack-picker').text()).toContain('Choose another pack');
  await wrapper.get('#pack-game-name').setValue('Imported pack');
  await wrapper.get('input[type="checkbox"]').setValue(true);
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('create_mrpack_game', {
    previewId: 'pack-preview-1', name: 'Imported pack', source: { source: 'automatic', mirror: '' },
    useDefaultDirectory: true, optionalFiles: ['resourcepacks/example.zip'],
  });
  expect(wrapper.emitted('created')?.[0]).toEqual([game]);

  setLocale('zh-CN');
  await flushPromises();
  expect(wrapper.find('label[for="pack-game-name"]').text()).toBe('游戏名称');
  expect(wrapper.find('.done-button').text()).toBe('导入并安装');
  wrapper.unmount();
});

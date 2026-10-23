// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import ImportLauncherPage from './ImportLauncherPage.vue';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));

afterEach(() => {
  vi.mocked(invoke).mockReset();
  setLocale('zh-CN');
});

it('updates import labels in both locales without changing the preview flow', async () => {
  setLocale('en-US');
  const preview = {
    preview_id: 'preview-1',
    source_display: 'C:/Games/.minecraft',
    candidates: [{
      candidate_id: 'candidate-1', name: 'Example', minecraft_version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14',
      supported: true, unsupported_reason: null,
      content_roots: [{ content_id: 'root-1', label: '.minecraft', has_saves: false }], warnings: [],
    }],
  };
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
    if (command === 'default_game_directory') return 'C:/Games';
    if (command === 'preview_launcher_import') return preview;
    return null;
  });
  const wrapper = mount(ImportLauncherPage);
  await flushPromises();
  expect(wrapper.find('.create-description').text()).toContain('Import a game from another launcher');
  expect(wrapper.find('.pack-picker').text()).toContain('Choose game folder');
  await wrapper.get('.pack-picker').trigger('click');
  await flushPromises();
  expect(wrapper.find('#launcher-import-candidate').exists()).toBe(true);
  expect(wrapper.find('.done-button').text()).toBe('Import as separate game');
  expect(invoke).toHaveBeenCalledWith('preview_launcher_import');

  setLocale('zh-CN');
  await flushPromises();
  expect(wrapper.find('.create-description').text()).toContain('从其他启动器迁移游戏');
  expect(wrapper.find('label[for="launcher-import-candidate"]').text()).toBe('来源游戏');
  expect(wrapper.find('.done-button').text()).toBe('导入为独立游戏');
  wrapper.unmount();
});

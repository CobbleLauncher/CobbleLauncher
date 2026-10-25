// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import GameImportResult from './GameImportResult.vue';
import { setLocale } from './i18n';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

afterEach(() => {
  vi.mocked(invoke).mockReset();
  setLocale('zh-CN');
});

it('shows only move results and explains retained source data', async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ mode: 'copy', sourceRetained: true, recycledRoots: 0, retainedRoots: 2 });
  const wrapper = mount(GameImportResult, { props: { gameId: 'copy-game', status: 'installed' } });
  await flushPromises();
  expect(wrapper.find('[role="status"]').exists()).toBe(false);

  vi.mocked(invoke).mockResolvedValueOnce({ mode: 'move', sourceRetained: true, recycledRoots: 3, retainedRoots: 1 });
  await wrapper.setProps({ gameId: 'move-game' }); await flushPromises();
  expect(wrapper.get('[role="status"]').text()).toContain('已从来源目录移出 3 个独立内容目录。');
  expect(wrapper.get('.import-result-warning').text()).toContain('有 1 个内容目录仍保留在来源位置。');
  expect(wrapper.get('.shared-info').attributes('aria-label')).toBe('共享资源和库文件始终保留在来源目录。');
  vi.mocked(invoke).mockResolvedValueOnce({ mode: 'move', sourceRetained: true, recycledRoots: 0, retainedRoots: 1 });
  await wrapper.setProps({ gameId: 'retained-game' }); await flushPromises();
  expect(wrapper.get('.import-result-line').text()).toContain('游戏已导入，原内容保留。');
  expect(wrapper.find('.import-result-line .ui-text-muted').exists()).toBe(true);
  expect(wrapper.get('.import-result-warning').text()).toContain('有 1 个内容目录仍保留在来源位置。');
  wrapper.unmount();
});

it('looks up only installed games and refreshes when installation completes', async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ mode: 'move', sourceRetained: false, recycledRoots: 2, retainedRoots: 0 }).mockResolvedValueOnce(null);
  const wrapper = mount(GameImportResult, { props: { gameId: 'game-a', status: 'installing' } });
  expect(invoke).not.toHaveBeenCalled();
  await wrapper.setProps({ status: 'installed' }); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_launcher_import_result', { id: 'game-a' });
  expect(wrapper.get('[role="status"]').text()).toContain('已从来源目录移出 2 个独立内容目录。');
  await wrapper.setProps({ status: 'installing' });
  expect(wrapper.find('[role="status"]').exists()).toBe(false);
  await wrapper.setProps({ status: 'installed' }); await flushPromises();
  expect(invoke).toHaveBeenCalledTimes(2);
  expect(wrapper.find('[role="status"]').exists()).toBe(false);
  wrapper.unmount();
});

it('ignores an older result after the selected game changes', async () => {
  let resolveOld!: (value: unknown) => void;
  vi.mocked(invoke)
    .mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }))
    .mockResolvedValueOnce({ mode: 'move', sourceRetained: false, recycledRoots: 4, retainedRoots: 0 });
  const wrapper = mount(GameImportResult, { props: { gameId: 'game-old', status: 'installed' } });
  await wrapper.setProps({ gameId: 'game-new' }); await flushPromises();
  resolveOld({ mode: 'move', sourceRetained: false, recycledRoots: 1, retainedRoots: 0 });
  await flushPromises();
  expect(wrapper.get('[role="status"]').text()).toContain('已从来源目录移出 4 个独立内容目录。');
  expect(wrapper.get('[role="status"]').text()).not.toContain('移出 1 个独立内容目录');
  wrapper.unmount();
});

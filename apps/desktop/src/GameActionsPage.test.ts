// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameActionsPage from './GameActionsPage.vue';
import { setLocale } from './i18n';
import type { Game } from './games';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const game: Game = {
  id: 'game-1',
  name: 'Maple',
  version: '1.21.1',
  loader_kind: 'fabric',
  loader_version: '0.16.14',
  directory: 'C:/games/Maple',
  status: 'installed',
  error: null,
  progress: null,
  launch_state: 'idle',
};

beforeEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
afterEach(() => { vi.useRealTimers(); setLocale('zh-CN'); document.body.innerHTML = ''; });

it('starts a durable copy and releases navigation without awaiting copy completion', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockResolvedValue('task-copy-1');
  const wrapper = mount(GameActionsPage, { props: { game, action: 'copy', starting: false } });

  expect(wrapper.text()).toContain('Copy the game, mods, and settings');
  expect((wrapper.get('#copy-game-name').element as HTMLInputElement).value).toBe('Maple copy');
  await wrapper.get('input[type="checkbox"]').setValue(true);
  await wrapper.get('form').trigger('submit');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('start_game_copy', { id: game.id, name: 'Maple copy', includeSaves: true });
  expect(wrapper.emitted('copyStarted')?.[0]?.[0]).toBe('task-copy-1');
  expect(wrapper.emitted('copied')).toBeUndefined();
  expect(wrapper.emitted('busy')).toEqual([[true],[false]]);
  expect(wrapper.text()).toContain('Copy started');
  wrapper.unmount();
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='cancel_task')).toBe(false);
});
it('leaves worlds excluded and keeps copy startup failure recoverable',async()=>{
 vi.mocked(invoke).mockRejectedValueOnce('GAME_COPY_BUSY').mockResolvedValueOnce('task-copy-2');
 const wrapper=mount(GameActionsPage,{props:{game,action:'copy',starting:false}});
 expect((wrapper.get('input[type="checkbox"]').element as HTMLInputElement).checked).toBe(false);
 await wrapper.get('form').trigger('submit');await flushPromises();
 expect(wrapper.find('[role="alert"]').exists()).toBe(true);
 expect(wrapper.emitted('busy')).toEqual([[true],[false]]);
 await wrapper.get('form').trigger('submit');await flushPromises();
 expect(invoke).toHaveBeenLastCalledWith('start_game_copy',{id:game.id,name:'Maple 副本',includeSaves:false});
 expect(wrapper.emitted('copyStarted')?.[0]).toEqual(['task-copy-2']);wrapper.unmount();
});

it('keeps export warnings and reports successful export in Chinese', async () => {
  vi.mocked(invoke).mockResolvedValue(true);
  const wrapper = mount(GameActionsPage, { props: { game, action: 'export', starting: false } });

  expect(wrapper.text()).toContain('游戏本体、资源缓存和玩家账号不会导出');
  await wrapper.get('form').trigger('submit');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('export_game', { id: game.id, includeSaves: false });
  expect(wrapper.get('[role="status"]').text()).toContain('整合包已导出');
  wrapper.unmount();
});
it('keeps the existing export action explicitly busy until it finishes',async()=>{
 let finish!:(value:boolean)=>void;vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
 const wrapper=mount(GameActionsPage,{props:{game,action:'export',starting:false}});
 await wrapper.get('form').trigger('submit');await flushPromises();
 expect(wrapper.emitted('busy')).toEqual([[true]]);
 expect(wrapper.get('button[type="submit"]').text()).toContain('导出中');
 finish(true);await flushPromises();expect(wrapper.emitted('busy')).toEqual([[true],[false]]);wrapper.unmount();
});

it('keeps the irreversible delete warning and removes only the selected game folder', async () => {
  vi.useFakeTimers({toFake:['Date','setTimeout','clearTimeout','setInterval','clearInterval']});
  vi.mocked(invoke).mockResolvedValue(undefined);
  const wrapper = mount(GameActionsPage, { props: { game, action: 'remove', starting: false } });

  expect(wrapper.text()).toContain('游戏文件和存档保留在原目录');
  await wrapper.get('[role="radio"][aria-label="连同文件删除"]').trigger('click');
  expect(wrapper.text()).toContain('此操作无法撤销');
  expect(wrapper.text()).toContain(game.directory);
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(invoke).not.toHaveBeenCalled();
  const confirmation=document.querySelector<HTMLButtonElement>('.confirmation-confirm')!;
  expect(confirmation.disabled).toBe(true);
  await vi.advanceTimersByTimeAsync(3000);
  confirmation.click();await flushPromises();
  expect(invoke).toHaveBeenCalledWith('remove_game', { id: game.id, deleteFiles: true });
  expect(wrapper.emitted('removed')?.[0]?.[0]).toBe(game.id);
  wrapper.unmount();
});
it('confirms library-only removal without a countdown and leaves cancellation harmless',async()=>{setLocale('en-US');vi.mocked(invoke).mockResolvedValue(undefined);const wrapper=mount(GameActionsPage,{props:{game,action:'remove',starting:false}});await wrapper.get('form').trigger('submit');await flushPromises();expect(document.querySelector<HTMLButtonElement>('.confirmation-confirm')!.disabled).toBe(false);document.querySelector<HTMLButtonElement>('.confirmation-cancel')!.click();await flushPromises();expect(invoke).not.toHaveBeenCalled();await wrapper.get('form').trigger('submit');await flushPromises();document.querySelector<HTMLButtonElement>('.confirmation-confirm')!.click();await flushPromises();expect(invoke).toHaveBeenCalledWith('remove_game',{id:game.id,deleteFiles:false});wrapper.unmount();});

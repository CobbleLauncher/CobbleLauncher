// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import GameDirectorySetup from './GameDirectorySetup.vue';
import FirstRunPage from '../FirstRunPage.vue';
import { setLocale } from '../i18n';
vi.mock('@tauri-apps/api/core', () => ({invoke:vi.fn(),isTauri:()=>true}));
const standard = {path:'C:/Users/Player/AppData/Local/CobbleLauncher/games',available_bytes:150_000_000_000};
const selected = {path:'D:/Minecraft',available_bytes:4_000_000_000};
beforeEach(() => {
  setLocale('en-US'); vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async command => command==='get_launcher_preferences'?{game_directory:standard.path,custom_game_directory:false}:command==='get_default_game_directory_candidate'||command==='get_standard_game_directory'?standard:command==='pick_default_game_directory_candidate'?selected:undefined);
});
afterEach(() => setLocale('zh-CN'));
async function choose(wrapper:ReturnType<typeof mount>) { await wrapper.findAll('button').find(button=>button.text()==='Choose folder')!.trigger('click');await flushPromises(); }
async function save(wrapper:ReturnType<typeof mount>) { return (wrapper.vm as unknown as {save:()=>Promise<boolean>}).save(); }
it('shows the actual recommended game path and free space without writing preferences', async () => {
  const wrapper=mount(GameDirectorySetup);await flushPromises();
  expect(wrapper.text()).toContain(standard.path);expect(wrapper.text()).toContain('Free 150 GB');
  expect(wrapper.text()).toContain('Recommended');expect(wrapper.emitted('ready')?.at(-1)).toEqual([true]);
  expect(await save(wrapper)).toBe(true);
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command.startsWith('set_')||command.startsWith('reset_'))).toBe(false);
  wrapper.unmount();
});
it('previews another drive and advisory low space, then saves it only on continuation', async () => {
  const wrapper=mount(GameDirectorySetup);await flushPromises();await choose(wrapper);
  expect(wrapper.text()).toContain('D:/Minecraft');expect(wrapper.text()).toContain('Free 4 GB');
  expect(wrapper.text()).toContain('Free space is low');expect(invoke).not.toHaveBeenCalledWith('set_default_game_directory',expect.anything());
  expect(await save(wrapper)).toBe(true);expect(invoke).toHaveBeenCalledWith('set_default_game_directory',{path:selected.path});
  wrapper.unmount();
});
it('treats a cancelled picker and unknown free space as non-destructive', async () => {
  vi.mocked(invoke).mockImplementation(async command=>command==='get_launcher_preferences'?{custom_game_directory:false}:command==='get_default_game_directory_candidate'?{...standard,available_bytes:null}:null);
  const wrapper=mount(GameDirectorySetup);await flushPromises();await choose(wrapper);
  expect(wrapper.text()).toContain(standard.path);expect(wrapper.text()).toContain('Free space is currently unavailable');
  expect(wrapper.text()).not.toContain('0 GB');expect(await save(wrapper)).toBe(true);wrapper.unmount();
});
it('uses the standard-directory reset rather than trying to set a possibly absent folder', async () => {
  vi.mocked(invoke).mockImplementation(async command=>command==='get_launcher_preferences'?{custom_game_directory:true}:command==='get_default_game_directory_candidate'?selected:command==='get_standard_game_directory'?standard:undefined);
  const wrapper=mount(GameDirectorySetup);await flushPromises();
  await wrapper.findAll('button').find(button=>button.text()==='Use recommended folder')!.trigger('click');await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('reset_default_game_directory',undefined);
  expect(await save(wrapper)).toBe(true);expect(invoke).toHaveBeenCalledWith('reset_default_game_directory',undefined);
  expect(invoke).not.toHaveBeenCalledWith('set_default_game_directory',expect.anything());wrapper.unmount();
});
it('does not advance the game step when saving the chosen folder fails', async () => {
  vi.mocked(invoke).mockImplementation(async command=>{
    if(command==='get_launcher_preferences')return{custom_game_directory:false};
    if(command==='get_default_game_directory_candidate')return standard;
    if(command==='pick_default_game_directory_candidate')return selected;
    if(command==='set_default_game_directory')throw new Error('synthetic write error');
    return undefined;
  });
  const wrapper=mount(FirstRunPage,{props:{step:'game',consentAccepted:true,meshChoice:null}});await flushPromises();
  await choose(wrapper);
  await wrapper.findAll('button').find(button=>button.text()==='Choose game')!.trigger('click');await flushPromises();
  expect(wrapper.emitted('createGame')).toBeUndefined();expect(wrapper.get('[role=alert]').text()).toContain('Could not save this folder');
  vi.mocked(invoke).mockResolvedValue(undefined);
  await wrapper.findAll('button').find(button=>button.text()==='Choose game')!.trigger('click');await flushPromises();
  expect(wrapper.emitted('createGame')).toEqual([[]]);wrapper.unmount();
});
it('saves the selected game folder before forwarding launcher import', async () => {
  const wrapper=mount(FirstRunPage,{props:{step:'game',consentAccepted:true,meshChoice:null}});await flushPromises();
  await choose(wrapper);
  const action=wrapper.findAll('button').find(button=>button.text()==='Import from PCL or another launcher')!;
  await action.trigger('click');await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_default_game_directory',{path:selected.path});
  expect(wrapper.emitted('importLauncher')).toEqual([[]]);
  expect(wrapper.emitted('createGame')).toBeUndefined();
  wrapper.unmount();
});
it('discards a folder-picker result that finishes after leaving the step', async () => {
  let finish!:(value:unknown)=>void;
  vi.mocked(invoke).mockImplementation(async command=>command==='get_launcher_preferences'?{custom_game_directory:false}:command==='get_default_game_directory_candidate'?standard:new Promise(resolve=>{finish=resolve;}));
  const wrapper=mount(GameDirectorySetup);await flushPromises();
  await wrapper.findAll('button').find(button=>button.text()==='Choose folder')!.trigger('click');
  wrapper.unmount();finish(selected);await flushPromises();
  expect(wrapper.emitted('ready')?.at(-1)).toEqual([false]);
  expect(invoke).not.toHaveBeenCalledWith('set_default_game_directory',expect.anything());
});

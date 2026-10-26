// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import MarketVersionFilter from './MarketVersionFilter.vue';
import { setLocale } from '../i18n';
const { invoke, settings } = vi.hoisted(() => ({ invoke: vi.fn(), settings: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('../download-settings', () => ({ readDownloadSettings: settings }));
const catalog = [
  { id:'26w01a',kind:'snapshot' },
  ...Array.from({length:45},(_,index)=>({id:`1.21.${45-index}`,kind:'release'})),
  {id:'1.20.1',kind:'release'},{id:'1.12.2',kind:'release'},{id:'b1.7.3',kind:'old_beta'},
];
let wrapper: VueWrapper;
beforeEach(()=>{setLocale('en-US');invoke.mockReset().mockResolvedValue(catalog);settings.mockReset().mockResolvedValue({source:'automatic',mirror:''});});
afterEach(()=>{wrapper?.unmount();document.body.innerHTML='';setLocale('zh-CN');});
function setup(value=''){wrapper=mount(MarketVersionFilter,{attachTo:document.body,props:{modelValue:value,installedVersions:['1.21.8']}});}
async function open(){await wrapper.get('.market-version-trigger').trigger('click');await flushPromises();}
function radio(id:string){return [...document.querySelectorAll<HTMLButtonElement>('.market-version-popover [role=radio]')].find(node=>node.textContent?.trim().startsWith(id))!;}
async function click(node:HTMLElement){node.click();await flushPromises();}
it('loads only on open, shows bounded installed/stable choices, selects and reuses the catalog',async()=>{
  setup('1.21.2');expect(invoke).not.toHaveBeenCalled();await open();
  expect(invoke).toHaveBeenCalledWith('list_game_versions',{source:{source:'automatic',mirror:''},includeAll:true});
  expect(document.querySelectorAll('.market-version-popover [role=radio]').length).toBeLessThanOrEqual(7);
  expect(radio('1.21.8')).toBeDefined();expect(radio('1.20.1')).toBeDefined();expect(radio('1.21.2')).toBeDefined();
  expect(document.querySelector('.market-version-popover input')).toBeNull();
  await click(radio('1.20.1'));expect(wrapper.emitted('update:modelValue')).toEqual([['1.20.1']]);
  expect(document.querySelector('.market-version-popover')).toBeNull();
  await open();expect(invoke).toHaveBeenCalledTimes(1);
  await click(radio('Any version'));expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['']);
});
it('offers bounded searchable history and snapshot/legacy choices without another request',async()=>{
  setup();await open();
  await click([...document.querySelectorAll<HTMLButtonElement>('.minecraft-version-actions button')].find(node=>node.textContent?.includes('More versions'))!);
  expect(document.querySelectorAll('.market-version-popover [role=radio]')).toHaveLength(21);
  const search=document.querySelector<HTMLInputElement>('.market-version-popover input[type=search]')!;
  search.value='1.12.2';search.dispatchEvent(new Event('input',{bubbles:true}));await flushPromises();expect(radio('1.12.2')).toBeDefined();
  await click([...document.querySelectorAll<HTMLButtonElement>('.minecraft-version-kinds button')].find(node=>node.textContent==='Snapshots')!);
  expect(radio('26w01a')).toBeDefined();
  await click([...document.querySelectorAll<HTMLButtonElement>('.minecraft-version-kinds button')].find(node=>node.textContent==='Older editions')!);
  expect(radio('b1.7.3')).toBeDefined();expect(invoke).toHaveBeenCalledTimes(1);
});
it('keeps a saved selection on failure and allows retry or clearing it',async()=>{
  invoke.mockRejectedValueOnce(new Error('offline'));setup('1.20.1');await open();
  expect(wrapper.get('.market-version-trigger').text()).toContain('1.20.1');
  expect(document.querySelector('.market-version-error')).not.toBeNull();
  await click(document.querySelector<HTMLButtonElement>('.market-version-error button')!);
  expect(invoke).toHaveBeenCalledTimes(2);expect(document.querySelector('.market-version-error')).toBeNull();
  await click(radio('Any version'));expect(wrapper.emitted('update:modelValue')).toEqual([['']]);
});
it('reads a fresh catalog after the configured download source changes',async()=>{
  setup();await open();await click(radio('Any version'));
  settings.mockResolvedValue({source:'bmclapi',mirror:''});await open();
  expect(invoke).toHaveBeenCalledTimes(2);
  expect(invoke).toHaveBeenLastCalledWith('list_game_versions',{source:{source:'bmclapi',mirror:''},includeAll:true});
});

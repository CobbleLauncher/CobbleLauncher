// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import InspectorPanel from './InspectorPanel.vue';
import type { InspectorPageReply, InspectorQuery, InspectorSession } from './inspector/types';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { vi.mocked(invoke).mockReset(); setLocale('zh-CN'); });
const cell = (text: string) => ({ text, byte_length: new TextEncoder().encode(text).length, complete: true });
const session: InspectorSession = { sessionId:'owned-session',summary:{current:{name:cell('Current pack'),version_id:cell('2'),declared_files:120,override_files:3,declared_download_bytes:2048},baseline:{name:cell('Old pack'),version_id:cell('1'),declared_files:110,override_files:2,declared_download_bytes:1024},changes:{added:11,removed:1,changed:4,unchanged:97}} };
function mockSession(empty = false) {
  vi.mocked(invoke).mockImplementation(async (command, args = {}) => {
    if(command === 'begin_pack_inspection') return session;
    if(command === 'query_pack_inspection') {
      const input = args as { query: InspectorQuery; revision: number };
      return {sessionId:session.sessionId,revision:input.revision,page:{query:input.query,total_rows:empty?0:123,matched_rows:empty?0:1,next_offset:null,rows:empty?[]:[{id:'row',source:'declared_file',change:'changed',path:cell('mods/map.jar'),before:cell('<script>old</script>'),after:cell('new complete value')}]},report:{schema_version:1,findings:[],tables:[]}} satisfies InspectorPageReply;
    }
    return null;
  });
}
function button(wrapper: ReturnType<typeof mount>, text: string) { return wrapper.findAll('button').find(value=>value.text()===text)!; }

it('starts without file access and shows only actual inventory and comparison summary counts after explicit open', async () => {
  setLocale('en-US'); mockSession();
  const wrapper = mount(InspectorPanel,{props:{pluginId:'fixture',allowCompare:true}});
  expect(invoke).not.toHaveBeenCalled();
  expect(wrapper.text()).toContain('No modpack open.');
  await button(wrapper,'Compare modpacks').trigger('click'); await flushPromises();
  expect(wrapper.get('.inspection-summary').text()).toContain('Current pack');
  expect(wrapper.get('.inspection-summary').text()).toContain('Old pack');
  expect(wrapper.get('.inspection-summary').text()).toContain('120');
  expect(wrapper.get('.inspection-summary').text()).toContain('2.0 KiB');
  expect(wrapper.get('.inspection-change-counts').text()).toContain('Added11');
  expect(invoke).toHaveBeenCalledWith('begin_pack_inspection',{pluginId:'fixture',compare:true,locale:'en'});
  wrapper.unmount();
});

it('inspects complete rows and before/after values without unnecessary native detail reads or HTML rendering', async () => {
  setLocale('en-US'); mockSession();
  const wrapper=mount(InspectorPanel,{props:{pluginId:'fixture'}});
  await button(wrapper,'Open modpack').trigger('click'); await flushPromises();
  await wrapper.get('button[aria-label="Inspect mods/map.jar"]').trigger('click');
  expect(wrapper.get('.inspector-detail pre').text()).toBe('new complete value');
  expect(wrapper.get('.inspector-selected-row').text()).toContain('mods/map.jar');
  const fields=wrapper.get('[aria-label="Detail field"]');
  await fields.findAll('button').find(value=>value.text()==='Comparison version')!.trigger('click');
  expect(wrapper.get('pre').text()).toBe('<script>old</script>');
  expect(wrapper.find('pre script').exists()).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='detail_pack_inspection')).toBe(false);
  wrapper.unmount();
});

it('distinguishes an empty loaded section from an unopened or filtered inventory', async () => {
  setLocale('en-US'); mockSession(true);
  const wrapper=mount(InspectorPanel,{props:{pluginId:'fixture'}});
  await button(wrapper,'Open modpack').trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('This section contains no items.');
  expect(wrapper.text()).not.toContain('No modpack open.');
  expect(wrapper.text()).toContain('Items 0–0 of 0');
  wrapper.unmount();
});

it('closes the original pending plugin context and discards its late session after selection changes', async () => {
  setLocale('en-US'); let finish!: (value: unknown)=>void;
  vi.mocked(invoke).mockImplementation(command=>command==='begin_pack_inspection'?new Promise(resolve=>{finish=resolve;}):Promise.resolve(null));
  const wrapper=mount(InspectorPanel,{props:{pluginId:'original',instanceId:'game-a'}});
  await button(wrapper,'Open modpack').trigger('click'); await flushPromises();
  await wrapper.setProps({pluginId:'next',instanceId:'game-b'}); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('close_pack_inspection',{pluginId:'original',instanceId:'game-a'});
  finish(session); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('close_pack_inspection',{sessionId:session.sessionId});
  expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='query_pack_inspection')).toBe(false);
  expect(wrapper.text()).toContain('No modpack open.');
  wrapper.unmount();
});

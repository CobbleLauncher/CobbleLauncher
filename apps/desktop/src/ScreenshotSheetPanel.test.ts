// SPDX-License-Identifier: MIT
import {mount,flushPromises} from '@vue/test-utils';
import {beforeEach,afterEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import ScreenshotSheetPanel from './ScreenshotSheetPanel.vue';
import {setLocale} from './i18n';
import type {GameScreenshot} from './screenshots';
import type {ScreenshotSheetPreview} from './screenshot-sheet';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
const screenshots:GameScreenshot[]=[{id:'second-selected',name:'Second.png',modified_at:2,size_bytes:1},{id:'first-selected',name:'First.png',modified_at:1,size_bytes:1}];
const result:ScreenshotSheetPreview={preview_id:'opaque-preview',data_url:'data:image/png;base64,c3ludGhldGlj',width:512,height:256};
const plugins=[{plugin_id:'community-layout',name:'Community layout'}];
beforeEach(()=>{setLocale('en-US');vi.mocked(invoke).mockReset();vi.mocked(invoke).mockImplementation(async command=>command==='list_screenshot_sheet_plugins'?plugins:command==='preview_screenshot_sheet'?result:undefined);});
afterEach(()=>setLocale('zh-CN'));
const mountPanel=()=>mount(ScreenshotSheetPanel,{props:{gameId:'game',screenshots}});
const button=(wrapper:ReturnType<typeof mountPanel>,name:string)=>wrapper.findAll('button').find(item=>item.text()===name)!;
it('uses discovered implementations and exact selection order, then waits for explicit Save As',async()=>{
 const wrapper=mountPanel();await flushPromises();expect(invoke).toHaveBeenCalledExactlyOnceWith('list_screenshot_sheet_plugins');
 await button(wrapper,'Preview contact sheet').trigger('click');await flushPromises();
 const args=vi.mocked(invoke).mock.calls.find(([command])=>command==='preview_screenshot_sheet')![1] as Record<string,unknown>;
 expect(args).toMatchObject({gameId:'game',screenshotIds:['second-selected','first-selected'],pluginId:'community-layout',columns:0,ordering:'selected',locale:'en'});expect(args.requestId).toMatch(/^[a-f0-9-]{36}$/);
 expect(wrapper.get('.sheet-preview').attributes('src')).toBe(result.data_url);expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='save_screenshot_sheet')).toBe(false);
 vi.mocked(invoke).mockResolvedValueOnce(false);await button(wrapper,'Save As…').trigger('click');await flushPromises();expect(wrapper.text()).toContain('Save canceled.');expect(wrapper.text()).not.toContain('Contact sheet saved.');
 vi.mocked(invoke).mockResolvedValueOnce(true);await button(wrapper,'Save As…').trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('save_screenshot_sheet',{previewId:result.preview_id});expect(wrapper.text()).toContain('Contact sheet saved.');wrapper.unmount();
});
it('keeps cancellation pending until the actual worker returns without inventing a preview',async()=>{
 let finish!:(value:null)=>void;vi.mocked(invoke).mockImplementation(async command=>command==='list_screenshot_sheet_plugins'?plugins:command==='preview_screenshot_sheet'?new Promise<null>(resolve=>{finish=resolve;}):undefined);
 const wrapper=mountPanel();await flushPromises();await button(wrapper,'Preview contact sheet').trigger('click');await flushPromises();await button(wrapper,'Cancel').trigger('click');await flushPromises();
 expect(wrapper.emitted('busy')).toEqual([[true]]);expect(wrapper.find('.sheet-preview').exists()).toBe(false);finish(null);await flushPromises();expect(wrapper.emitted('busy')).toEqual([[true],[false]]);expect(wrapper.text()).toContain('Preview canceled.');expect(wrapper.find('[role="alert"]').exists()).toBe(false);wrapper.unmount();
});
it('cancels only its own request and rejects late previews after game context changes',async()=>{
 let finish!:(value:ScreenshotSheetPreview)=>void;vi.mocked(invoke).mockImplementation(async command=>command==='list_screenshot_sheet_plugins'?plugins:command==='preview_screenshot_sheet'?new Promise<ScreenshotSheetPreview>(resolve=>{finish=resolve;}):undefined);
 const wrapper=mountPanel();await flushPromises();await button(wrapper,'Preview contact sheet').trigger('click');await flushPromises();const requestId=(vi.mocked(invoke).mock.calls.find(([command])=>command==='preview_screenshot_sheet')![1] as {requestId:string}).requestId;
 await wrapper.setProps({gameId:'other-game'});await flushPromises();expect(invoke).toHaveBeenCalledWith('cancel_screenshot_sheet',{requestId});finish(result);await flushPromises();expect(wrapper.find('.sheet-preview').exists()).toBe(false);expect(wrapper.emitted('busy')?.at(-1)).toEqual([false]);wrapper.unmount();
});
it('does not cancel another panel when unmounted without owning a preview or worker',async()=>{
 const wrapper=mountPanel();await flushPromises();wrapper.unmount();expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='cancel_screenshot_sheet')).toBe(false);
});
it('shows missing extension and source-change failures without fake export success',async()=>{
 vi.mocked(invoke).mockResolvedValueOnce([]);const wrapper=mountPanel();await flushPromises();expect(wrapper.text()).toContain('Install an image layout extension');expect(button(wrapper,'Preview contact sheet')).toBeUndefined();await button(wrapper,'Extensions').trigger('click');expect(wrapper.emitted('extensions')).toEqual([[]]);wrapper.unmount();
 vi.mocked(invoke).mockImplementation(async command=>{if(command==='list_screenshot_sheet_plugins')return plugins;if(command==='preview_screenshot_sheet')throw 'SCREENSHOT_CHANGED';});const failed=mountPanel();await flushPromises();await button(failed,'Preview contact sheet').trigger('click');await flushPromises();expect(failed.get('[role="alert"]').text()).toContain('screenshot changed');expect(failed.find('.sheet-preview').exists()).toBe(false);expect(button(failed,'Save As…')).toBeUndefined();failed.unmount();
});
it('invalidates the old preview when layout options change and sends numeric columns',async()=>{
 const wrapper=mountPanel();await flushPromises();await button(wrapper,'Preview contact sheet').trigger('click');await flushPromises();
 await wrapper.findAll('select')[0]!.setValue('2');await wrapper.findAll('select')[1]!.setValue('newest');await flushPromises();expect(wrapper.find('.sheet-preview').exists()).toBe(false);
 await button(wrapper,'Preview contact sheet').trigger('click');await flushPromises();expect(invoke).toHaveBeenLastCalledWith('preview_screenshot_sheet',expect.objectContaining({columns:2,ordering:'newest'}));wrapper.unmount();
});

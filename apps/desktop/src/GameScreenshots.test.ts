// SPDX-License-Identifier: MIT
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import ScreenshotSheetPanel from './ScreenshotSheetPanel.vue';
import GameScreenshots from './GameScreenshots.vue';
import { setLocale } from './i18n';
import type { GameScreenshot, GameScreenshotImage } from './screenshots';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const item: GameScreenshot = { id: 'opaque-image-a', name: '2026-10-22_10.00.00.png', modified_at: 1792634400, size_bytes: 1234 };
const image: GameScreenshotImage = { data_url: 'data:image/png;base64,c3ludGhldGlj', width: 320, height: 180, original_width: 1920, original_height: 1080 };
const mountGallery = () => mount(GameScreenshots, { props: { gameId: 'game-a' }, attachTo: document.body });
const dialog = () => new DOMWrapper(document.body.querySelector('[role="dialog"]')!);
beforeEach(() => { setLocale('en-US'); vi.mocked(invoke).mockReset(); });
afterEach(() => { document.body.innerHTML = ''; setLocale('zh-CN'); });

it('shows loading and a real empty state, and opens only the authorized game folder', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_screenshots' ? new Promise(resolve => { finish = resolve; }) : undefined);
  const wrapper = mountGallery();
  expect(wrapper.get('[role="status"]').text()).toBe('Loading screenshots…');
  finish({ items: [], total: 0, next_offset: null }); await flushPromises();
  expect(wrapper.text()).toContain('No screenshots yet.');
  expect(invoke).not.toHaveBeenCalledWith('get_game_screenshot', expect.anything());
  await wrapper.findAll('button').find(button => button.text() === 'Open screenshots folder')!.trigger('click');
  expect(invoke).toHaveBeenCalledWith('open_game_screenshots_folder', { id: 'game-a' });
  wrapper.unmount();
});

it('loads thumbnails first, fetches a preview only on selection, and opens the exact original ID', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_screenshots' ? { items: [item], total: 1, next_offset: null } : command === 'get_game_screenshot' ? image : undefined);
  const wrapper = mountGallery(); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_game_screenshot', { id: 'game-a', screenshotId: item.id, thumbnail: true });
  expect(vi.mocked(invoke).mock.calls.filter(([command, args]) => command === 'get_game_screenshot' && (args as { thumbnail: boolean }).thumbnail === false)).toHaveLength(0);
  expect(wrapper.get('.screenshot-card img').attributes('src')).toBe(image.data_url);
  await wrapper.get('.screenshot-card').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('get_game_screenshot', { id: 'game-a', screenshotId: item.id, thumbnail: false });
  expect(dialog().get('img').attributes('src')).toBe(image.data_url);
  expect(dialog().text()).toContain('1920 × 1080');
  await dialog().findAll('button').find(button => button.text() === 'Open original')!.trigger('click');
  expect(invoke).toHaveBeenCalledWith('open_game_screenshot', { id: 'game-a', screenshotId: item.id });
  wrapper.unmount();
});

it('bounds thumbnail work at four and discards old-game results without starting more old requests', async () => {
  const finish: ((value: unknown) => void)[] = [];
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'list_game_screenshots') return (args as { id: string }).id === 'game-a' ? { items: Array.from({ length: 6 }, (_, index) => ({ ...item, id: `image-${index}` })), total: 6, next_offset: null } : { items: [], total: 0, next_offset: null };
    return new Promise(resolve => finish.push(resolve));
  });
  const wrapper = mountGallery(); await flushPromises();
  expect(finish).toHaveLength(4);
  finish[0]!(image); await flushPromises();
  expect(finish).toHaveLength(5);
  await wrapper.setProps({ gameId: 'game-b' }); await flushPromises();
  finish.slice(1).forEach(resolve => resolve(image)); await flushPromises();
  expect(finish).toHaveLength(5);
  expect(wrapper.findAll('.screenshot-card')).toHaveLength(0);
  expect(wrapper.text()).toContain('No screenshots yet.');
  expect(invoke).toHaveBeenCalledWith('list_game_screenshots', { id: 'game-b', offset: 0, limit: 40 });
  wrapper.unmount();
});

it('pages using native offsets and shows the exact total rather than truncating silently', async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'list_game_screenshots' ? (args as { offset: number }).offset === 0 ? { items: Array.from({ length: 40 }, (_, index) => ({ ...item, id: `image-${index}` })), total: 41, next_offset: 40 } : { items: [{ ...item, id: 'last-image', name: 'last.png' }], total: 41, next_offset: null } : image);
  const wrapper = mountGallery(); await flushPromises();
  expect(wrapper.get('.screenshot-pages').text()).toContain('1–40 / 41');
  await wrapper.findAll('button').find(button => button.text() === 'Next')!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_game_screenshots', { id: 'game-a', offset: 40, limit: 40 });
  expect(wrapper.get('.screenshot-pages').text()).toContain('41–41 / 41');
  expect(wrapper.findAll('.screenshot-card')).toHaveLength(1);
  expect(wrapper.findAll('button').find(button => button.text() === 'Next')!.attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('does not reopen a preview when its image arrives after the dialog was closed', async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'list_game_screenshots' ? { items: [item], total: 1, next_offset: null } : (args as { thumbnail: boolean }).thumbnail ? image : new Promise(resolve => { finish = resolve; }));
  const wrapper = mountGallery(); await flushPromises();
  await wrapper.get('.screenshot-card').trigger('click'); await flushPromises();
  expect(dialog().get('[role="status"]').text()).toBe('Loading image…');
  await dialog().get('button[aria-label="Close preview"]').trigger('click'); await flushPromises();
  finish(image); await flushPromises();
  expect(document.body.querySelector('[role="dialog"]')).toBeNull();
  wrapper.unmount();
});

it('surfaces native failures and lets refresh retry instead of showing a fake empty gallery', async () => {
  let fail = true;
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'list_game_screenshots') { if (fail) throw 'Screenshot folder is unavailable'; return { items: [], total: 0, next_offset: null }; }
    return undefined;
  });
  const wrapper = mountGallery(); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain('Could not load screenshots. Refresh to try again.');
  expect(wrapper.text()).not.toContain('No screenshots yet.');
  fail = false;
  await wrapper.findAll('button').find(button => button.text() === 'Refresh')!.trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('No screenshots yet.');
  wrapper.unmount();
});

it('keeps the current gallery during same-page refresh and reuses unchanged thumbnails', async () => {
  let refresh!: (value: unknown) => void;
  let lists = 0;
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_screenshots' ? ++lists === 1 ? { items: [item], total: 1, next_offset: null } : new Promise(resolve => { refresh = resolve; }) : image);
  const wrapper = mountGallery(); await flushPromises();
  await wrapper.findAll('button').find(button => button.text() === 'Refresh')!.trigger('click'); await flushPromises();
  expect(wrapper.findAll('.screenshot-card')).toHaveLength(1);
  expect(wrapper.get('.screenshot-card img').attributes('src')).toBe(image.data_url);
  expect(wrapper.get('.loading-indicator').text()).toBe('Loading screenshots…');
  refresh({ items: [item, { ...item, id: 'new-image', name: 'new.png' }], total: 2, next_offset: null }); await flushPromises();
  expect(wrapper.findAll('.screenshot-card')).toHaveLength(2);
  const imageCalls = vi.mocked(invoke).mock.calls.filter(([command]) => command === 'get_game_screenshot');
  expect(imageCalls).toHaveLength(2);
  expect(imageCalls[1]![1]).toEqual({ id: 'game-a', screenshotId: 'new-image', thumbnail: true });
  wrapper.unmount();
});

it('browses preview images with arrow keys and reports real positions without wrapping at the ends', async () => {
  const second = { ...item, id: 'second-image', name: 'second.png' };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_screenshots' ? { items: [item, second], total: 2, next_offset: null } : image);
  const wrapper = mountGallery(); await flushPromises();
  await wrapper.findAll('.screenshot-card')[0]!.trigger('click'); await flushPromises();
  expect(dialog().get('.screenshot-browse span').text()).toBe('1 / 2');
  expect(dialog().get('button[aria-label="Previous image"]').attributes('disabled')).toBeDefined();
  await dialog().trigger('keydown', { key: 'ArrowRight' }); await flushPromises();
  expect(dialog().get('h2').text()).toBe('second.png');
  expect(dialog().get('.screenshot-browse span').text()).toBe('2 / 2');
  expect(invoke).toHaveBeenCalledWith('get_game_screenshot', { id: 'game-a', screenshotId: second.id, thumbnail: false });
  expect(dialog().get('button[aria-label="Next image"]').attributes('disabled')).toBeDefined();
  const calls = vi.mocked(invoke).mock.calls.length;
  await dialog().trigger('keydown', { key: 'ArrowRight' });
  await dialog().trigger('keydown', { key: 'ArrowLeft', ctrlKey: true }); await flushPromises();
  expect(invoke).toHaveBeenCalledTimes(calls);
  await dialog().trigger('keydown', { key: 'ArrowLeft' }); await flushPromises();
  expect(dialog().get('.screenshot-browse span').text()).toBe('1 / 2');
  wrapper.unmount();
});

it('browses preview across native pages in both directions with exact global positions', async () => {
  const firstPage = Array.from({ length: 40 }, (_, index) => ({ ...item, id: `page-image-${index}`, name: `${index}.png` }));
  const last = { ...item, id: 'last-image', name: 'last.png' };
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'list_game_screenshots' ? (args as { offset: number }).offset === 0 ? { items: firstPage, total: 41, next_offset: 40 } : { items: [last], total: 41, next_offset: null } : image);
  const wrapper = mountGallery(); await flushPromises();
  await wrapper.findAll('.screenshot-card')[39]!.trigger('click'); await flushPromises();
  expect(dialog().get('.screenshot-browse span').text()).toBe('40 / 41');
  await dialog().get('button[aria-label="Next image"]').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_game_screenshots', { id: 'game-a', offset: 40, limit: 40 });
  expect(dialog().get('h2').text()).toBe('last.png');
  expect(dialog().get('.screenshot-browse span').text()).toBe('41 / 41');
  await dialog().get('button[aria-label="Previous image"]').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('list_game_screenshots', { id: 'game-a', offset: 0, limit: 40 });
  expect(dialog().get('h2').text()).toBe('39.png');
  expect(dialog().get('.screenshot-browse span').text()).toBe('40 / 41');
  const fullReads = vi.mocked(invoke).mock.calls.filter(([command, args]) => command === 'get_game_screenshot' && !(args as { thumbnail: boolean }).thumbnail);
  expect(fullReads).toHaveLength(3);
  wrapper.unmount();
});

it('browses preview without accepting a cross-page result after the dialog is closed', async () => {
  let finish!: (value: unknown) => void;
  const firstPage = Array.from({ length: 40 }, (_, index) => ({ ...item, id: `page-image-${index}`, name: `${index}.png` }));
  vi.mocked(invoke).mockImplementation(async (command, args) => command === 'list_game_screenshots' ? (args as { offset: number }).offset === 0 ? { items: firstPage, total: 41, next_offset: 40 } : new Promise(resolve => { finish = resolve; }) : image);
  const wrapper = mountGallery(); await flushPromises();
  await wrapper.findAll('.screenshot-card')[39]!.trigger('click'); await flushPromises();
  await dialog().get('button[aria-label="Next image"]').trigger('click'); await flushPromises();
  expect(dialog().get('img').attributes('src')).toBe(image.data_url);
  await dialog().get('button[aria-label="Close preview"]').trigger('click'); await flushPromises();
  finish({ items: [{ ...item, id: 'last-image', name: 'last.png' }], total: 41, next_offset: null }); await flushPromises();
  expect(document.body.querySelector('[role="dialog"]')).toBeNull();
  expect(wrapper.findAll('.screenshot-card')).toHaveLength(40);
  expect(invoke).not.toHaveBeenCalledWith('get_game_screenshot', { id: 'game-a', screenshotId: 'last-image', thumbnail: false });
  wrapper.unmount();
});

it('preserves bounded screenshot selection order across native pages and clears it on game changes',async()=>{
 vi.mocked(invoke).mockImplementation(async(command,args)=>{if(command==='list_game_screenshots')return(args as {offset:number}).offset===0?{items:Array.from({length:40},(_,index)=>({...item,id:`selected-${index}`,name:`File ${index}.png`})),total:41,next_offset:40}:{items:[{...item,id:'last-selected',name:'Last.png'}],total:41,next_offset:null};if(command==='list_screenshot_sheet_plugins')return[{plugin_id:'layout',name:'Layout'}];if(command==='preview_screenshot_sheet')return{preview_id:'sheet',data_url:image.data_url,width:512,height:256};return image;});
 const wrapper=mountGallery();await flushPromises();await wrapper.findAll('input[type="checkbox"]')[1]!.setValue(true);await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true);await flushPromises();
 expect(wrapper.findComponent(ScreenshotSheetPanel).exists()).toBe(false);expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='list_screenshot_sheet_plugins')).toBe(false);
 await wrapper.findAll('button').find(value=>value.text()==='Preview contact sheet')!.trigger('click');await flushPromises();
 expect(wrapper.findComponent(ScreenshotSheetPanel).props('screenshots').map((value:GameScreenshot)=>value.id)).toEqual(['selected-1','selected-0']);
 await wrapper.findAll('button').find(value=>value.text()==='Next')!.trigger('click');await flushPromises();await wrapper.get('input[type="checkbox"]').setValue(true);expect(wrapper.findComponent(ScreenshotSheetPanel).props('screenshots').map((value:GameScreenshot)=>value.id)).toEqual(['selected-1','selected-0','last-selected']);
 await wrapper.findAll('button').find(value=>value.text()==='Previous')!.trigger('click');await flushPromises();for(let index=2;index<11;index++)await wrapper.findAll('input[type="checkbox"]')[index]!.setValue(true);await flushPromises();
 expect(wrapper.findComponent(ScreenshotSheetPanel).props('screenshots')).toHaveLength(12);expect(wrapper.findAll('input[type="checkbox"]')[11]!.attributes('disabled')).toBeDefined();expect(wrapper.text()).toContain('12 / 12 selected');
 await wrapper.findAll('button').find(value=>value.text()==='Preview contact sheet')!.trigger('click');await flushPromises();const requestId=(vi.mocked(invoke).mock.calls.find(([command])=>command==='preview_screenshot_sheet')![1] as {requestId:string}).requestId;
 await wrapper.findAll('button').find(value=>value.text()==='Clear selection')!.trigger('click');await flushPromises();expect(invoke).toHaveBeenCalledWith('cancel_screenshot_sheet',{requestId});expect(wrapper.findComponent(ScreenshotSheetPanel).exists()).toBe(false);
 await wrapper.findAll('input[type="checkbox"]')[0]!.setValue(true);await flushPromises();expect(wrapper.findComponent(ScreenshotSheetPanel).exists()).toBe(false);
 await wrapper.setProps({gameId:'game-b'});await flushPromises();expect(wrapper.findComponent(ScreenshotSheetPanel).exists()).toBe(false);expect(wrapper.findAll('input[type="checkbox"]').every(value=>!(value.element as HTMLInputElement).checked)).toBe(true);wrapper.unmount();
});

it('offers preview and sheet selection from the screenshot context menu', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_screenshots' ? { items: [item], total: 1, next_offset: null } : command === 'get_game_screenshot' ? image : undefined);
  const wrapper = mountGallery(); await flushPromises();
  await wrapper.get('.screenshot-card').trigger('contextmenu', { button: 2, clientX: 80, clientY: 80 });
  await flushPromises();
  const selection = new DOMWrapper(document.body.querySelector('[role="menuitemcheckbox"]')!);
  expect(selection.attributes('aria-checked')).toBe('false');
  await selection.trigger('click'); await flushPromises();
  expect((wrapper.get('.screenshot-selection-toggle input').element as HTMLInputElement).checked).toBe(true);
  expect(vi.mocked(invoke).mock.calls.filter(([command, args]) => command === 'get_game_screenshot' && (args as { thumbnail: boolean }).thumbnail === false)).toHaveLength(0);
  wrapper.unmount();
});

// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { setLocale } from './i18n';
import { invoke } from '@tauri-apps/api/core';
import BlueprintsPage from './BlueprintsPage.vue';
import { pendingBlueprint } from './blueprints';
import type { ResolvedCommand } from './extensions/contributions';
import type { Game } from './games';
const textureRenderer=vi.hoisted(()=>({setTextures:vi.fn(async()=>true),snapshot:vi.fn<() => string | null>(()=>null)}));

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('./blueprint-viewer/renderer', () => ({ createBlueprintRenderer: async () => ({ addChunk: async () => true,...textureRenderer }) }));
afterEach(() => { pendingBlueprint.value = null; vi.clearAllMocks(); textureRenderer.snapshot.mockReset().mockReturnValue(null); });
const command = (id: string): ResolvedCommand => ({ pluginId: 'viewer', commandId: 'open', label: 'Open', workflow: 'viewer', blueprintId: id });
const opened = (id: string) => ({ resourceId: `resource-${id}`, name: id, format: 'schem', scene: { width: 1, height: 1, length: 1, materials: [], palette: [], chunk_count: 0 } });

it('refreshes the saved thumbnail when late native textures finish after local display', async () => {
  let finish: (value: unknown) => void = () => {};
  const response = { source: 'game', textures: [{ id: 'stone', dataUrl: 'data:image/png;base64,synthetic-texture' }], paletteFaces: [Array(6).fill('stone')], resolvedBlocks: 1 };
  textureRenderer.snapshot.mockReturnValueOnce('data:image/png;base64,plain-preview').mockReturnValue('data:image/png;base64,textured-preview');
  vi.mocked(invoke).mockImplementation(async name => {
    if (name === 'open_blueprint') return { ...opened('A'), scene: { ...opened('A').scene, palette: ['minecraft:stone'], chunk_count: 1 } };
    if (name === 'read_viewer_chunk') return { blocks: [{ x: 0, y: 0, z: 0, palette_index: 0 }] };
    if (name === 'load_viewer_textures') return new Promise(resolve => { finish = resolve; });
    if (name === 'list_blueprints') return [{ id: 'A', name: 'A', pluginId: 'viewer' }];
    return [];
  });
  const wrapper = mount(BlueprintsPage, { props: { selectedCommand: command('A'), games: [{ id: 'game', name: 'Local game', status: 'installed' } as Game], initialGameId: 'game' } });
  await flushPromises();
  expect(wrapper.find('canvas').exists()).toBe(true);
  expect(invoke).toHaveBeenCalledWith('save_blueprint_thumbnail', { id: 'A', png: 'data:image/png;base64,plain-preview' });
  finish(response); await flushPromises();
  expect(textureRenderer.setTextures).toHaveBeenCalledWith(response);
  expect(invoke).toHaveBeenCalledWith('save_blueprint_thumbnail', { id: 'A', png: 'data:image/png;base64,textured-preview' });
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'save_blueprint_thumbnail')).toHaveLength(2);
  wrapper.unmount();
});

it('does not recapture a saved thumbnail when late native textures belong to a closed preview', async () => {
  let finish: (value: unknown) => void = () => {};
  const response = { source: 'game', textures: [{ id: 'stone', dataUrl: 'data:image/png;base64,obsolete-texture' }], paletteFaces: [Array(6).fill('stone')], resolvedBlocks: 1 };
  textureRenderer.snapshot.mockReturnValue('data:image/png;base64,plain-preview');
  vi.mocked(invoke).mockImplementation(async name => {
    if (name === 'open_blueprint') return { ...opened('A'), scene: { ...opened('A').scene, palette: ['minecraft:stone'], chunk_count: 1 } };
    if (name === 'read_viewer_chunk') return { blocks: [{ x: 0, y: 0, z: 0, palette_index: 0 }] };
    if (name === 'load_viewer_textures') return new Promise(resolve => { finish = resolve; });
    if (name === 'list_blueprints') return [{ id: 'A', name: 'A', pluginId: 'viewer' }];
    return [];
  });
  const wrapper = mount(BlueprintsPage, { props: { selectedCommand: command('A'), view: 'viewer', games: [{ id: 'game', name: 'Local game', status: 'installed' } as Game], initialGameId: 'game' } });
  await flushPromises();
  expect(textureRenderer.snapshot).toHaveBeenCalledTimes(1);
  await wrapper.setProps({ selectedCommand: undefined, view: 'library' }); await flushPromises();
  finish(response); await flushPromises();
  expect(textureRenderer.setTextures).not.toHaveBeenCalledWith(response);
  expect(textureRenderer.snapshot).toHaveBeenCalledTimes(1);
  expect(vi.mocked(invoke).mock.calls.filter(([name]) => name === 'save_blueprint_thumbnail')).toHaveLength(1);
  wrapper.unmount();
});

it('shows the color preview before textures finish and ignores textures disabled during loading',async()=>{
 let finish:(value:unknown)=>void=()=>{};
 vi.mocked(invoke).mockImplementation(async name=>name==='open_blueprint'?{...opened('A'),scene:{...opened('A').scene,palette:['minecraft:stone'],chunk_count:1}}:name==='read_viewer_chunk'?{blocks:[{x:0,y:0,z:0,palette_index:0}]}:name==='load_viewer_textures'?new Promise(resolve=>{finish=resolve;}):[]);
 const game={id:'game',name:'Local game',status:'installed'} as Game;
 const wrapper=mount(BlueprintsPage,{props:{selectedCommand:command('A'),games:[game],initialGameId:'game'}});await flushPromises();
 expect(wrapper.find('canvas').exists()).toBe(true);expect(invoke).toHaveBeenCalledWith('load_viewer_textures',{resourceId:'resource-A',gameId:'game'});
 await wrapper.get('.texture-controls [role=switch]').trigger('click');await flushPromises();
 const response={source:'game',textures:[{id:'stone',dataUrl:'data:image/png;base64,test'}],paletteFaces:[Array(6).fill('stone')],resolvedBlocks:1};finish(response);await flushPromises();
 expect(textureRenderer.setTextures).toHaveBeenCalledWith(null);expect(textureRenderer.setTextures).not.toHaveBeenCalledWith(response);wrapper.unmount();
});

it('keeps the newer Viewer selection when an earlier native open resolves late', async () => {
  const pending = new Map<string, (value: unknown) => void>();
  vi.mocked(invoke).mockImplementation(async (name, args) => name === 'open_blueprint' ? new Promise(resolve => pending.set((args as { id: string }).id, resolve)) : []);
  const wrapper = mount(BlueprintsPage, { props: { selectedCommand: command('A') } });
  await flushPromises(); await wrapper.setProps({ selectedCommand: command('B') }); await flushPromises();
  pending.get('B')!(opened('B')); await flushPromises();
  expect(wrapper.get('h2').text()).toBe('B');
  pending.get('A')!(opened('A')); await flushPromises();
  expect(wrapper.get('h2').text()).toBe('B');
  expect(invoke).toHaveBeenCalledWith('close_viewer', { resourceId: 'resource-A' });
  expect(vi.mocked(invoke).mock.calls.some(([name, args]) => name === 'close_viewer' && (args as { resourceId: string }).resourceId === 'resource-B')).toBe(false);
  wrapper.unmount();
});

it('releases a pending native resource after its selected command is cleared', async () => {
  let finish: (value: unknown) => void = () => {};
  vi.mocked(invoke).mockImplementation(async name => name === 'open_blueprint' ? new Promise(resolve => { finish = resolve; }) : []);
  const wrapper = mount(BlueprintsPage, { props: { selectedCommand: command('A') } });
  await flushPromises(); await wrapper.setProps({ selectedCommand: undefined });
  finish(opened('A')); await flushPromises();
  expect(wrapper.find('canvas').exists()).toBe(false);
  expect(invoke).toHaveBeenCalledWith('close_viewer', { resourceId: 'resource-A' });
  wrapper.unmount();
});

it('shows saved content and search immediately without opening the online provider', async () => {
  vi.mocked(invoke).mockImplementation(async name => name === 'list_blueprints' ? [{ id: 'saved', name: 'Workshop', pluginId: 'viewer' }] : []);
  const wrapper = mount(BlueprintsPage, { global: { stubs: { OnlineBlueprints: true } } });
  await flushPromises();
  expect(wrapper.find('input[type="search"]').exists()).toBe(true);
  expect(wrapper.text()).toContain('Workshop');
  expect(wrapper.find('online-blueprints-stub').exists()).toBe(false);
  expect(invoke).not.toHaveBeenCalledWith('get_blueprint_provider_availability');
  expect(wrapper.get('[role="tab"][aria-selected="true"]').attributes('id')).toBe('blueprint-library-tab');
  wrapper.unmount();
});

it('offers a local file action in an empty library and keyboard navigation to the optional online catalog', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const wrapper = mount(BlueprintsPage, { global: { stubs: { OnlineBlueprints: true } } });
  await flushPromises();
  expect(wrapper.find('.blueprint-empty button.primary-button').exists()).toBe(true);
  await wrapper.get('#blueprint-library-tab').trigger('keydown', { key: 'ArrowRight' });
  expect(wrapper.find('online-blueprints-stub').exists()).toBe(true);
  expect(wrapper.emitted('navigate')?.at(-1)).toEqual([{ view: 'online' }]);
  wrapper.unmount();
});

it('closes the current viewer when unified navigation returns to the library without emitting another navigation', async () => {
  vi.mocked(invoke).mockImplementation(async name => name === 'open_blueprint' ? opened('A') : []);
  const wrapper = mount(BlueprintsPage, { props: { selectedCommand: command('A'), view: 'viewer' } });
  await flushPromises();
  expect(wrapper.find('canvas').exists()).toBe(true);
  const before = wrapper.emitted('navigate')?.length;
  await wrapper.setProps({ selectedCommand: undefined, view: 'library' });
  await flushPromises();
  expect(wrapper.find('canvas').exists()).toBe(false);
  expect(invoke).toHaveBeenCalledWith('close_viewer', { resourceId: 'resource-A' });
  expect(wrapper.emitted('navigate')?.length).toBe(before);
  wrapper.unmount();
});

const localChoices = { viewer_id: 'local-viewer', viewers: [{ id: 'local-viewer', name: 'Local Viewer', file_extensions: ['schem', 'litematic'] }], interface: { id: 'launcher:viewer', version: '^1.0.0', implementations: [], candidates: [] } };
it('opens a local file directly with the single compatible viewer and no provider lookup', async () => {
  vi.mocked(invoke).mockImplementation(async name => name === 'get_blueprint_viewer_choices' ? localChoices : name === 'open_schematic' ? null : []);
  const wrapper = mount(BlueprintsPage); await flushPromises();
  expect(wrapper.find('.local-viewer-picker').exists()).toBe(false);
  await wrapper.get('.blueprint-empty button').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_schematic', { pluginId: 'local-viewer' });
  expect(invoke).not.toHaveBeenCalledWith('get_blueprint_provider_availability');
  wrapper.unmount();
});

it('asks only when multiple local viewers match and forwards the explicit choice', async () => {
  vi.mocked(invoke).mockImplementation(async name => name === 'get_blueprint_viewer_choices' ? { ...localChoices, viewer_id: null, viewers: [...localChoices.viewers, { id: 'other-viewer', name: 'Other Viewer', file_extensions: ['schem'] }] } : name === 'open_schematic' ? null : []);
  const wrapper = mount(BlueprintsPage); await flushPromises();
  await wrapper.get('.blueprint-empty button').trigger('click'); await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('open_schematic', expect.anything());
  await wrapper.get('.local-viewer-picker select').setValue('other-viewer'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_schematic', { pluginId: 'other-viewer' });
  wrapper.unmount();
});

it('uses a saved blueprint format to choose its only matching local viewer', async () => {
  vi.mocked(invoke).mockImplementation(async name => name === 'get_blueprint_viewer_choices' ? { ...localChoices, viewer_id: null, viewers: [{ id: 'schem-viewer', name: 'Schem Viewer', file_extensions: ['schem'] }, { id: 'litematic-viewer', name: 'Litematic Viewer', file_extensions: ['litematic'] }] } : name === 'list_blueprints' ? [{ id: 'saved-litematic', name: 'Saved design', pluginId: 'old-viewer', format: 'litematic' }] : name === 'open_blueprint' ? null : []);
  const wrapper = mount(BlueprintsPage); await flushPromises();
  await wrapper.findAll('.blueprint-library-main > button')[0]!.trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('open_blueprint', { id: 'saved-litematic', pluginId: 'litematic-viewer' });
  wrapper.unmount();
});

it('keeps saved content available when no viewer is enabled and routes to Extensions', async () => {
  vi.mocked(invoke).mockImplementation(async name => name === 'get_blueprint_viewer_choices' ? { ...localChoices, viewer_id: null, viewers: [], disabled_viewers: localChoices.viewers } : name === 'list_blueprints' ? [{ id: 'saved', name: 'Saved design', pluginId: 'old-viewer', format: 'schem' }] : []);
  const wrapper = mount(BlueprintsPage); await flushPromises();
  expect(wrapper.text()).toContain('Saved design');
  expect(wrapper.text()).toContain('Local Viewer');
  await wrapper.findAll('.local-viewer-choice button')[0]!.trigger('click');
  expect(wrapper.emitted('extensions')).toHaveLength(1);
  expect(invoke).not.toHaveBeenCalledWith('open_blueprint', expect.anything());
  wrapper.unmount();
});

it('pending OS file opens with the sole viewer using its exact queue ID', async () => {
  pendingBlueprint.value = { queueId: 'queue-a', name: 'queued.schem', format: 'schem' };
  vi.mocked(invoke).mockImplementation(async name => name === 'get_blueprint_viewer_choices' ? localChoices : name === 'take_pending_schematic' ? opened('Queued') : []);
  const wrapper = mount(BlueprintsPage); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('take_pending_schematic', { pluginId: 'local-viewer', expectedQueueId: 'queue-a' });
  expect(invoke).not.toHaveBeenCalledWith('open_schematic', expect.anything());
  expect(pendingBlueprint.value).toBeNull();
  expect(wrapper.get('h2').text()).toBe('Queued');
  wrapper.unmount();
});

it('pending OS file survives cancellation of a multiple-viewer choice and can be retried', async () => {
  pendingBlueprint.value = { queueId: 'queue-a', name: 'queued.schem', format: 'schem' };
  vi.mocked(invoke).mockImplementation(async name => name === 'get_blueprint_viewer_choices' ? { ...localChoices, viewer_id: null, viewers: [...localChoices.viewers, { id: 'other-viewer', name: 'Other Viewer', file_extensions: ['schem'] }] } : name === 'take_pending_schematic' ? opened('Queued') : []);
  const first = mount(BlueprintsPage); await flushPromises();
  expect(invoke).not.toHaveBeenCalledWith('take_pending_schematic', expect.anything());
  first.unmount();
  expect(pendingBlueprint.value?.queueId).toBe('queue-a');
  const retry = mount(BlueprintsPage); await flushPromises();
  await retry.get('.local-viewer-picker select').setValue('other-viewer'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('take_pending_schematic', { pluginId: 'other-viewer', expectedQueueId: 'queue-a' });
  expect(pendingBlueprint.value).toBeNull();
  retry.unmount();
});

it('pending OS file remains available after a failed open and retries the same queued file', async () => {
  pendingBlueprint.value = { queueId: 'queue-a', name: 'queued.schem', format: 'schem' };
  let attempts = 0;
  vi.mocked(invoke).mockImplementation(async name => {
    if (name === 'get_blueprint_viewer_choices') return localChoices;
    if (name === 'take_pending_schematic') { if (++attempts === 1) throw 'Viewer could not open this file'; return opened('Queued'); }
    return [];
  });
  const wrapper = mount(BlueprintsPage); await flushPromises();
  expect(pendingBlueprint.value?.queueId).toBe('queue-a');
  await wrapper.get('[role="alert"] button').trigger('click'); await flushPromises();
  expect(attempts).toBe(2);
  expect(invoke).toHaveBeenLastCalledWith('take_pending_schematic', { pluginId: 'local-viewer', expectedQueueId: 'queue-a' });
  expect(pendingBlueprint.value).toBeNull();
  wrapper.unmount();
});

it('sorts material counts and copies only the filtered rows', async () => {
  setLocale('en-US');
  vi.mocked(invoke).mockImplementation(async name => {
    if (name === 'open_blueprint') return { ...opened('materials'), scene: { ...opened('materials').scene, materials: [{ state: 'minecraft:stone', count: 2 }, { state: 'minecraft:dirt', count: 5 }] } };
    return [];
  });
  const wrapper = mount(BlueprintsPage, { props: { selectedCommand: command('materials') } });
  await flushPromises();
  expect(wrapper.findAll('tbody tr').map(row => row.text())).toEqual(['minecraft:dirt5', 'minecraft:stone2']);
  await wrapper.get('#material-search').setValue('stone');
  await wrapper.findAll('button').find(button => button.text() === 'Copy shown materials')!.trigger('click');
  expect(writeText).toHaveBeenCalledWith('2\tminecraft:stone');
  wrapper.unmount(); setLocale('zh-CN');
});

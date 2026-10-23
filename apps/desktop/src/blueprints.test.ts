// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { invoke } from '@tauri-apps/api/core';
import BlueprintsPage from './BlueprintsPage.vue';
import SelectField from './ui/SelectField.vue';
import BlueprintImportPanel from './BlueprintImportPanel.vue';
import { canImportBlueprint } from './blueprints';
import type { Game } from './games';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const graphics = vi.hoisted(() => ({ render: vi.fn(), dispose: vi.fn() }));
vi.mock('./blueprint-viewer/renderer', () => ({ createBlueprintRenderer: async (_canvas: unknown, _value: unknown, signal: AbortSignal) => { graphics.render(); signal.addEventListener('abort', () => graphics.dispose()); return { addChunk: async () => true, setTextures: async () => true }; } }));
vi.mock('three', () => {
  class Graphic {
    position = { set: vi.fn() };
    instanceMatrix = { needsUpdate: false };
    instanceColor = { needsUpdate: false };
    add = vi.fn(); remove = vi.fn(); lookAt = vi.fn(); updateProjectionMatrix = vi.fn(); setPixelRatio = vi.fn(); setSize = vi.fn();
    setMatrixAt = vi.fn(); setColorAt = vi.fn(); makeTranslation = vi.fn(); setHSL = vi.fn();
    render = graphics.render; dispose = graphics.dispose;
  }
  return Object.fromEntries(['WebGLRenderer', 'Scene', 'PerspectiveCamera', 'AmbientLight', 'DirectionalLight', 'BoxGeometry', 'MeshLambertMaterial', 'InstancedMesh', 'Matrix4', 'Color'].map(name => [name, Graphic]));
});
vi.mock('three/examples/jsm/controls/OrbitControls.js', () => ({ OrbitControls: class { addEventListener = vi.fn(); removeEventListener = vi.fn(); dispose = graphics.dispose; } }));
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); vi.clearAllMocks(); setLocale('zh-CN'); });
describe('saved blueprint import', () => {
  const game = { id: 'game-1', name: 'Creative', status: 'installed', launch_state: 'idle' } as Game;
  it.each([undefined, 'schem', 'litematic'] as const)('keeps import copy and the reported destination in the source format (%s)', async format => {
    setLocale('en-US');
    const extension = format ?? 'schem';
    const destination = `C:/Games/Creative/schematics/house.${extension}`;
    vi.mocked(invoke).mockImplementation(async command => command === 'list_games' ? [game] : { destination });
    const wrapper = mount(BlueprintImportPanel, { props: { blueprintId: 'saved-original', format } });
    await flushPromises();
    expect(wrapper.get('.secondary-button').text()).toBe(`Import .${extension}`);
    expect(wrapper.text()).toContain(format === 'litematic' ? 'Requires Litematica.' : 'Requires a mod that supports Sponge .schem files.');
    await wrapper.findComponent(SelectField).vm.$emit('update:modelValue', game.id); await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('import_blueprint', { id: game.id, blueprintId: 'saved-original', duplicatePolicy: 'error' });
    expect(wrapper.get('[role="status"]').text()).toContain(destination);
    wrapper.unmount();
  });
  it('requires an explicit stopped game and reports the real destination', async () => {
    vi.mocked(invoke).mockImplementation(async command => {
      if (command === 'list_games') return [game, { ...game, id: 'running', launch_state: 'running' }];
      if (command === 'import_blueprint') return { destination: 'C:/Games/Creative/schematics/house.schem' };
      return null;
    });
    const wrapper = mount(BlueprintImportPanel, { props: { blueprintId: 'saved-1' } });
    await flushPromises();
    expect(wrapper.get('.secondary-button').attributes('disabled')).toBeDefined();
    expect(wrapper.findComponent(SelectField).props('options').find(option => option.value === 'running')?.disabled).toBe(true);
    expect(invoke).not.toHaveBeenCalledWith('import_blueprint', expect.anything());
    await wrapper.findComponent(SelectField).vm.$emit('update:modelValue', 'game-1');
    await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('import_blueprint', { id: 'game-1', blueprintId: 'saved-1', duplicatePolicy: 'error' });
    expect(wrapper.get('[role="status"]').text()).toContain('C:/Games/Creative/schematics/house.schem');
    wrapper.unmount();
  });
  it('keeps an import failure visible without reporting success', async () => {
    vi.mocked(invoke).mockImplementation(async command => {
      if (command === 'list_games') return [game];
      throw new Error('game is running');
    });
    const wrapper = mount(BlueprintImportPanel, { props: { blueprintId: 'saved-1' } });
    await flushPromises(); await wrapper.findComponent(SelectField).vm.$emit('update:modelValue', 'game-1');
    await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('game is running');
    expect(wrapper.find('[role="status"]').exists()).toBe(false);
    wrapper.unmount();
  });
  it('detects an enabled matching Mod and keeps duplicate handling explicit', async () => {
    setLocale('en-US');
    vi.mocked(invoke).mockImplementation(async name => name === 'list_games' ? [game] : name === 'list_game_mods' ? [{ name: 'Litematica', file_name: 'litematica.jar', enabled: true }] : { destination: 'schematics/house-1.litematic' });
    const wrapper = mount(BlueprintImportPanel, { props: { blueprintId: 'saved', format: 'litematic', initialGameId: game.id } });
    await flushPromises();
    expect(wrapper.text()).toContain('Litematica detected');
    expect(wrapper.text()).not.toContain('Find Litematica in Market');
    await wrapper.findAllComponents(SelectField)[1]!.vm.$emit('update:modelValue', 'keep_both');
    await wrapper.get('.secondary-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('import_blueprint', { id: game.id, blueprintId: 'saved', duplicatePolicy: 'keep_both' });
    await wrapper.findAll('button').find(button => button.text() === 'Open schematics folder')!.trigger('click');
    expect(invoke).toHaveBeenCalledWith('open_blueprint_destination', { id: game.id });
    wrapper.unmount();
  });
  it('offers a contextual market search for missing Mods without installation', async () => {
    setLocale('en-US');
    vi.mocked(invoke).mockImplementation(async name => name === 'list_games' ? [game] : []);
    const wrapper = mount(BlueprintImportPanel, { props: { blueprintId: 'saved', format: 'litematic', initialGameId: game.id } });
    await flushPromises();
    await wrapper.findAll('button').find(button => button.text() === 'Find Litematica in Market')!.trigger('click');
    expect(wrapper.emitted('market')).toEqual([[{ gameId: game.id, query: 'Litematica' }]]);
    expect(invoke).not.toHaveBeenCalledWith('import_blueprint', expect.anything());
    wrapper.unmount();
  });
  it('excludes running, preparing, installing and busy games', () => {
    expect(canImportBlueprint(game)).toBe(true);
    for (const patch of [{ launch_state: 'running' }, { launch_state: 'preparing' }, { status: 'installing' }, { content_installing: true }]) {
      expect(canImportBlueprint({ ...game, ...patch } as Game)).toBe(false);
    }
  });
});
describe('blueprint UI resource lifecycle without WebGL', () => {
  it('loads host chunks, filters materials and releases the opaque resource on close', async () => {
    vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
    vi.spyOn(HTMLCanvasElement.prototype, 'getBoundingClientRect').mockReturnValue({ width: 400, height: 320 } as DOMRect);
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === 'list_blueprints') return [];
      if (command === 'get_blueprint_viewer_choices') return { viewer_id: 'viewer', viewers: [{ id: 'viewer', name: 'Viewer', file_extensions: ['schem'] }], disabled_viewers: [] };
      if (command === 'open_schematic') return { resourceId: 'owned-resource', name: 'Test schematic', scene: { width: 2, height: 1, length: 1, palette: ['minecraft:stone', 'minecraft:dirt'], materials: [{ state: 'minecraft:stone', count: 1 }, { state: 'minecraft:dirt', count: 1 }], chunk_count: 1 } };
      if (command === 'read_viewer_chunk') return { blocks: [{ x: 0, y: 0, z: 0, palette_index: 0 }, { x: 1, y: 0, z: 0, palette_index: 1 }] };
      return null;
    });
    const wrapper = mount(BlueprintsPage);
    await flushPromises();
    expect(invoke).not.toHaveBeenCalledWith('open_schematic', expect.anything());
    await wrapper.get('.blueprint-empty .primary-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('read_viewer_chunk', { resourceId: 'owned-resource', index: 0 });
    expect(wrapper.get('table').text()).toContain('minecraft:stone');
    await wrapper.get('input[type="search"]').setValue('dirt');
    expect(wrapper.get('tbody').text()).toContain('minecraft:dirt');
    expect(wrapper.get('tbody').text()).not.toContain('minecraft:stone');
    await vi.waitFor(() => expect(wrapper.get('.blueprint-title button').attributes('disabled')).toBeUndefined());
    await wrapper.get('.blueprint-title button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('save_blueprint', { resourceId: 'owned-resource' });
    await wrapper.get('.blueprint-toolbar .text-button').trigger('click'); await flushPromises();
    expect(invoke).toHaveBeenCalledWith('close_viewer', { resourceId: 'owned-resource' });
    expect(graphics.render).toHaveBeenCalled();
    expect(graphics.dispose).toHaveBeenCalled();
    expect(wrapper.find('canvas').exists()).toBe(false);
    wrapper.unmount();
  });
});

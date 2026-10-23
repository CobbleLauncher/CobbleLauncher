// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from './App.vue';
import ToolsPage from './ToolsPage.vue';
import BlueprintsPage from './BlueprintsPage.vue';
import ToolCatalog from './ToolCatalog.vue';
import { incomingBlueprint, pendingBlueprint, type OpenedBlueprint, type PendingBlueprintRequest } from './blueprints';
import { setLocale } from './i18n';
vi.mock('@tauri-apps/api/core', () => ({invoke: vi.fn(), isTauri: () => true}));
vi.mock('@tauri-apps/api/event', () => ({listen: vi.fn()}));
vi.mock('@tauri-apps/api/window', () => ({getCurrentWindow: () => ({isMaximized: vi.fn().mockResolvedValue(false), onCloseRequested: vi.fn().mockResolvedValue(() => {})})}));
vi.mock('./theme', () => ({readTheme: () => 'light', applyTheme: vi.fn()}));
vi.mock('./blueprint-viewer/renderer', () => ({createBlueprintRenderer: vi.fn().mockResolvedValue({addChunk: vi.fn().mockResolvedValue(true)})}));
let pending: PendingBlueprintRequest | null;
const callbacks = new Map<string, () => void>();
const stubs = {LibraryHome: true, AppNavigation: true, ProfileSwitcher: true, CreateGamePage: true, ImportPackPage: true, ImportLauncherPage: true, CfPackInstallPage: true, SettingsPage: true, PluginsPanel: true, ExtensionSettingsPanel: true, GameDetailsPage: true, GameActionsPage: true, MeshPage: true, ProfilesPage: true, AddProfilePage: true, BlueprintLibrary: true, MaterialExportButton: true};
function schematic(id: string): OpenedBlueprint {return {resourceId: id, name: `OS ${id}`, scene: {width: 1, height: 1, length: 1, palette: [], materials: [], chunk_count: 0}};}
function queue(id: string): PendingBlueprintRequest {return {queueId: `queue-${id}`, name: `Queued ${id}`, format: 'schem'};}
beforeEach(() => {
  pending = null; callbacks.clear(); incomingBlueprint.value = null; pendingBlueprint.value = null; setLocale('en-US'); window.scrollTo = vi.fn();
  vi.mocked(listen).mockImplementation(async (event, handler) => {callbacks.set(event, () => handler({event, id: 1, payload: undefined})); return () => {callbacks.delete(event);};});
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'peek_pending_schematic') return pending;
    if (command === 'take_pending_schematic') {
      const expectedQueueId = (args as { expectedQueueId?: string } | undefined)?.expectedQueueId;
      if (!pending || expectedQueueId !== pending.queueId) return null;
      const consumed = pending; pending = null; return schematic(consumed.queueId.replace('queue-', ''));
    }
    if (command === 'get_blueprint_viewer_choices') return {viewer_id: 'schematic-viewer', viewers: [{id: 'schematic-viewer', name: 'Schematic Viewer', file_extensions: ['schem', 'litematic']}], disabled_viewers: []};
    if (command === 'list_player_profiles') return {profiles: [], active_id: null};
    if (command.startsWith('list_')) return [];
    return null;
  });
});
afterEach(() => {incomingBlueprint.value = null; pendingBlueprint.value = null; vi.mocked(invoke).mockReset(); setLocale('zh-CN');});
it('peeks queued OS opens without consuming them and consumes each only when Viewer opens', async () => {
  const wrapper = mount(App, {global: {stubs}}); await flushPromises();
  pending = queue('first'); callbacks.get('schematic-open')!(); await flushPromises();
  await vi.waitFor(() => expect(wrapper.findComponent(ToolsPage).exists()).toBe(true));
  await vi.waitFor(() => expect(wrapper.get('.blueprint-viewer h2').text()).toBe('OS first'));
  expect(wrapper.findComponent(ToolsPage).exists()).toBe(true);
  expect(wrapper.findComponent(BlueprintsPage).exists()).toBe(true);
  expect(wrapper.get('.blueprint-viewer h2').text()).toBe('OS first');
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'peek_pending_schematic').length).toBeGreaterThanOrEqual(3);
  expect(vi.mocked(invoke)).toHaveBeenCalledWith('take_pending_schematic', {pluginId: 'schematic-viewer', expectedQueueId: 'queue-first'});
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'open_schematic')).toBe(false);
  expect(pending).toBeNull();
  expect(incomingBlueprint.value).toBeNull();
  await wrapper.get('.page-header .back-button').trigger('click'); await flushPromises();
  expect(wrapper.findComponent(BlueprintsPage).exists()).toBe(true);
  expect(wrapper.find('#blueprint-library-panel').exists()).toBe(true);
  expect(invoke).toHaveBeenCalledWith('close_viewer', {resourceId: 'first'});
  await wrapper.get('.page-header .back-button').trigger('click'); await flushPromises();
  expect(wrapper.findComponent(ToolCatalog).exists()).toBe(true);
  pending = queue('second'); callbacks.get('schematic-open')!(); await flushPromises();
  await vi.waitFor(() => expect(wrapper.get('.blueprint-viewer h2').text()).toBe('OS second'));
  expect(wrapper.get('.blueprint-viewer h2').text()).toBe('OS second');
  expect(wrapper.findComponent(ToolCatalog).exists()).toBe(false);
  expect(vi.mocked(invoke)).toHaveBeenCalledWith('take_pending_schematic', {pluginId: 'schematic-viewer', expectedQueueId: 'queue-second'});
  expect(pending).toBeNull();
  expect(incomingBlueprint.value).toBeNull();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'resolve_extension_command')).toBe(false);
  wrapper.unmount(); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('close_viewer', {resourceId: 'second'});
});

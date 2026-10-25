// SPDX-License-Identifier: MIT
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { ContextMenuRoot, ContextMenuContent } from 'reka-ui';
import GameModsPanel from './GameModsPanel.vue';
import { setLocale } from './i18n';
import { clearPageWorkspacesForTests } from './use-page-workspace';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
afterEach(() => { document.body.innerHTML = ''; vi.mocked(invoke).mockReset(); setLocale('zh-CN'); clearPageWorkspacesForTests(); });
it('uses the existing removal confirmation from the mod context menu', async () => {
  setLocale('en-US');
  const mod = { file_name: 'example.jar', name: 'Example', version: '1', authors: [], loader: 'fabric', enabled: true, description: null };
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mods' ? [mod] : []);
  const wrapper = mount(GameModsPanel, { props: { game: { id: 'game', name: 'Game', version: '1.21.1', loader_version: '0.16', directory: 'private', status: 'installed', error: null, progress: null }, locked: false }, attachTo: document.body, global: { stubs: { ModUpdates: true, ModDetailsPanel: true } } });
  await flushPromises();
  await wrapper.get('.mod-details-link').trigger('contextmenu', { button: 2, clientX: 80, clientY: 80 }); await flushPromises();
  const menu = new DOMWrapper(document.body.querySelector('[role="menu"]')!);
  expect(menu.text()).toContain('Disable Example');
  await menu.findAll('[role="menuitem"]').find(item => item.text() === 'Remove Example')!.trigger('click'); await flushPromises();
  expect(wrapper.text()).toContain('Confirm removal');
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'remove_game_mod')).toBe(false);
  await wrapper.findAll('button').find(button => button.text() === 'Confirm removal')!.trigger('click');
  expect(invoke).toHaveBeenCalledWith('remove_game_mod', { id: 'game', fileName: 'example.jar' });
  wrapper.unmount();
});

const game = { id: 'game', name: 'Game', version: '1.21.1', loader_version: '0.16', directory: 'private', status: 'installed' as const, error: null, progress: null };
function localMods(count: number) {
  return Array.from({ length: count }, (_, index) => ({ file_name: `mod-${String(index).padStart(4, '0')}.jar`, name: `Fixture Mod ${String(index).padStart(4, '0')}`, version: '1', authors: [], loader: 'fabric', enabled: true, description: null }));
}

it('renders one page and can search for a mod beyond the first page', async () => {
  setLocale('en-US');
  const mods = localMods(1000);
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mods' ? mods : []);
  const wrapper = mount(GameModsPanel, { props: { game, locked: false }, global: { stubs: { ModUpdates: true, InstalledModDetails: true } } });
  await flushPromises();
  expect(wrapper.get('input[type="search"]').attributes('data-list-search')).toBe('');
  expect(wrapper.findAll('.mod-row')).toHaveLength(50);
  expect(wrapper.findAllComponents(ContextMenuRoot)).toHaveLength(1);
  expect(wrapper.findAllComponents(ContextMenuContent)).toHaveLength(0);
  await wrapper.get('input[type="search"]').setValue('Fixture Mod 0999');
  expect(wrapper.findAll('.mod-row')).toHaveLength(1);
  expect(wrapper.text()).toContain('Fixture Mod 0999');
  await wrapper.get('input[type="search"]').setValue('');
  expect(wrapper.findAll('.mod-row')).toHaveLength(50);
  expect(wrapper.findAllComponents(ContextMenuContent)).toHaveLength(0);
  wrapper.unmount();
});

it('restores the mod list search, page, and row selection after leaving the game', async () => {
  setLocale('en-US');
  const mods = localMods(51);
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mods' ? mods : []);
  const first = mount(GameModsPanel, { props: { game, locked: false }, global: { stubs: { ModUpdates: true, InstalledModDetails: true } } });
  await flushPromises();
  await first.get('.plugin-list-pagination button:last-child').trigger('click');
  const selection = first.get('[aria-label="Select Fixture Mod 0050"]');
  (selection.element as HTMLInputElement).checked = true;
  await selection.trigger('change');
  expect(first.text()).toContain('1 selected');
  first.unmount();
  const reopened = mount(GameModsPanel, { props: { game, locked: false }, global: { stubs: { ModUpdates: true, InstalledModDetails: true } } });
  await flushPromises();
  expect(reopened.text()).toContain('1 selected');
  expect(reopened.findAll('.mod-row')).toHaveLength(1);
  expect(reopened.get('.mod-row').text()).toContain('Fixture Mod 0050');
  expect(reopened.get('[aria-label="Select Fixture Mod 0050"]').element).toHaveProperty('checked', true);
  reopened.unmount();
});

it('moves between mod rows with arrows and opens the row context menu with Shift+F10', async () => {
  setLocale('en-US');
  const mods = localMods(2);
  vi.mocked(invoke).mockImplementation(async command => command === 'list_game_mods' ? mods : []);
  const wrapper = mount(GameModsPanel, { props: { game, locked: false }, attachTo: document.body, global: { stubs: { ModUpdates: true, InstalledModDetails: true } } });
  await flushPromises();
  const details = wrapper.findAll('.mod-details-link');
  (details[0]!.element as HTMLElement).focus();
  await details[0]!.trigger('keydown', { key: 'ArrowDown' });
  expect(document.activeElement).toBe(details[1]!.element);
  await details[1]!.trigger('keydown', { key: 'F10', shiftKey: true });
  await flushPromises();
  expect(document.body.querySelector('[role="menu"]')?.textContent).toContain('Disable Fixture Mod 0001');
  wrapper.unmount();
});

it('clamps the current page after deleting its last item', async () => {
  setLocale('en-US');
  let mods = localMods(51);
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'list_game_mods') return mods;
    if (command === 'remove_game_mod') { mods = mods.filter(mod => mod.file_name !== (args as { fileName: string }).fileName); return mods; }
    return [];
  });
  const wrapper = mount(GameModsPanel, { props: { game, locked: false }, global: { stubs: { ModUpdates: true, InstalledModDetails: true } } });
  await flushPromises();
  await wrapper.get('.plugin-list-pagination button:last-child').trigger('click');
  expect(wrapper.findAll('.mod-row')).toHaveLength(1);
  await wrapper.get('[aria-label="Remove Fixture Mod 0050"]').trigger('click');
  await wrapper.findAll('button').find(button => button.text() === 'Confirm removal')!.trigger('click');
  await flushPromises();
  expect(wrapper.find('.plugin-list-pagination').exists()).toBe(false);
  expect(wrapper.get('.mod-row').text()).toContain('Fixture Mod 0000');
  expect(wrapper.findAll('.mod-row')).toHaveLength(50);
  wrapper.unmount();
});

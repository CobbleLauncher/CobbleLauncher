// SPDX-License-Identifier: MIT
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import BlueprintLibrary from './BlueprintLibrary.vue';
import type { SavedBlueprint } from './blueprints';
import ExtensionMenuItems from './ExtensionMenuItems.vue';
import { setLocale } from './i18n';
import { clearPageWorkspacesForTests } from './use-page-workspace';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
beforeEach(() => setLocale('en-US'));
afterEach(() => { vi.clearAllMocks(); setLocale('zh-CN'); document.body.innerHTML=''; clearPageWorkspacesForTests(); });

async function chooseAction(wrapper: ReturnType<typeof mount>, label: string) {
  await wrapper.get('[aria-label="More actions for Workshop"]').trigger('keydown',{key:'Enter'});
  await flushPromises();
  await new DOMWrapper(document.body).findAll('[role="menuitem"]').find(item=>item.text()===label)!.trigger('click');
  await flushPromises();
}
const item: SavedBlueprint = { id: 'blueprint-1', name: 'Workshop', pluginId: 'viewer' };

it('reveals a newly downloaded item through active filters and focuses its action', async () => {
  const wrapper = mount(BlueprintLibrary, { attachTo: document.body, props: { items: [item], busy: false, selectedId: null }, global: { stubs: { ExtensionMenuItems: true } } });
  expect(wrapper.get('input[type="search"]').attributes('data-list-search')).toBe('');
  await wrapper.get('input[type="search"]').setValue('hidden');
  await wrapper.findAll('button').find(button => button.text() === 'Favorites')!.trigger('click');
  await wrapper.setProps({ selectedId: item.id });
  await flushPromises();
  expect(wrapper.get('input[type="search"]').element).toHaveProperty('value', '');
  expect(wrapper.get('.blueprint-located').attributes('data-blueprint-id')).toBe(item.id);
  expect(document.activeElement).toBe(wrapper.get('.blueprint-open').element);
  wrapper.unmount();
});

it('persists favorites and filters by author without losing source attribution', async () => {
  const source = { provider: 'mcs-provider', id: 'online', version: 1, author: 'Builder', attribution: 'Original author', license: 'CC-BY-4.0' };
  const saved = { ...item, source };
  vi.mocked(invoke).mockResolvedValue([{ ...saved, favorite: true }]);
  const wrapper = mount(BlueprintLibrary, { props: { items: [saved], busy: false }, global: { stubs: { ExtensionMenuItems: true } } });
  await wrapper.get('input[type="search"]').setValue('builder');
  expect(wrapper.text()).toContain('Workshop');
  expect(wrapper.text()).toContain('CC-BY-4.0');
  await wrapper.get('[aria-label="Add to favorites"]').trigger('click');
  await flushPromises();
  expect(invoke).toHaveBeenCalledWith('set_blueprint_favorite', { id: item.id, favorite: true });
  expect(wrapper.emitted('changed')?.[0]).toEqual([[{ ...saved, favorite: true }]]);
  await wrapper.findAll('button').find(button => button.text() === 'Favorites')!.trigger('click');
  expect(wrapper.findAll('.blueprint-library li')).toHaveLength(0);
  await wrapper.setProps({ items: [{ ...saved, favorite: true }] });
  expect(wrapper.findAll('.blueprint-library li')).toHaveLength(1);
  wrapper.unmount();
});

it('uses a saved litematic format for import instead of the legacy schem default', async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  const wrapper = mount(BlueprintLibrary, { global:{stubs:{ExtensionMenuItems:true}}, props: { items: [{ ...item, format: 'litematic' }], busy: false } });
  await wrapper.findAll('button').find(button => button.text() === 'Import to game')!.trigger('click');
  await flushPromises();
  expect(wrapper.get('.blueprint-import .secondary-button').text()).toBe('Import .litematic');
  expect(wrapper.get('.blueprint-import').text()).toContain('Requires Litematica.');
  wrapper.unmount();
});

it('filters saved blueprints by name and restores the list when search is cleared', async () => {
  const second: SavedBlueprint = { id: 'blueprint-2', name: 'Mountain base', pluginId: 'viewer' };
  const wrapper = mount(BlueprintLibrary, { global:{stubs:{ExtensionMenuItems:true}}, props: { items: [item, second], busy: false } });

  await wrapper.get('input[type="search"]').setValue('MOUNTAIN');
  expect(wrapper.text()).toContain('Mountain base');
  expect(wrapper.text()).not.toContain('Workshop');

  await wrapper.get('input[type="search"]').setValue('unknown');
  expect(wrapper.text()).toContain('No blueprints found.');
  expect(wrapper.findAll('.blueprint-library li')).toHaveLength(0);

  await wrapper.get('button').trigger('click');
  expect(wrapper.get('input[type="search"]').element).toHaveProperty('value', '');
  expect(wrapper.findAll('.blueprint-library li')).toHaveLength(2);
  await wrapper.findAll('.blueprint-library-main > button').at(0)!.trigger('click');
  expect(wrapper.emitted('open')?.[0]).toEqual(['blueprint-1']);
  wrapper.unmount();
});

it('restores the saved blueprint search and favorites filter after leaving the page', async () => {
  setLocale('en-US');
  const favorite = { ...item, favorite: true };
  const first = mount(BlueprintLibrary, { props: { items: [favorite, { ...item, id: 'other', name: 'Other' }], busy: false, initialGameId: 'blueprint-game' }, global: { stubs: { ExtensionMenuItems: true } } });
  await first.get('input[type="search"]').setValue('Workshop');
  await first.findAll('button').find(button => button.text() === 'Favorites')!.trigger('click');
  first.unmount();
  const reopened = mount(BlueprintLibrary, { props: { items: [favorite, { ...item, id: 'other', name: 'Other' }], busy: false, initialGameId: 'blueprint-game' }, global: { stubs: { ExtensionMenuItems: true } } });
  expect(reopened.get('input[type="search"]').element).toHaveProperty('value', 'Workshop');
  expect(reopened.findAll('.blueprint-library li')).toHaveLength(1);
  expect(reopened.get('button[aria-pressed="true"]').attributes('aria-pressed')).toBe('true');
  reopened.unmount();
});

it('moves between blueprints with arrows and opens row actions with Shift+F10', async () => {
  setLocale('en-US');
  const second = { ...item, id: 'blueprint-2', name: 'Mountain base' };
  const wrapper = mount(BlueprintLibrary, { props: { items: [item, second], busy: false }, attachTo: document.body, global: { stubs: { ExtensionMenuItems: true } } });
  const first = wrapper.get('.blueprint-open');
  const next = wrapper.findAll('.blueprint-open')[1]!;
  (first.element as HTMLElement).focus();
  await first.trigger('keydown', { key: 'ArrowDown' });
  expect(document.activeElement).toBe(next.element);
  await next.trigger('keydown', { key: 'F10', shiftKey: true });
  await flushPromises();
  expect(document.body.querySelector('[role="menu"]')?.textContent).toContain('Rename');
  wrapper.unmount();
});

it('opens the existing blueprint menu with right click without opening or changing the resource', async () => {
  const wrapper = mount(BlueprintLibrary, {props: {items: [item], busy: false}, attachTo: document.body, global: {stubs: {ExtensionMenuItems: true}}});
  await wrapper.get('[data-blueprint-id="blueprint-1"]').trigger('contextmenu', {button: 2}); await flushPromises();
  expect(document.body.querySelector('[role=menu]')?.textContent).toContain('Rename');
  expect(wrapper.emitted('open')).toBeUndefined();
  expect(invoke).not.toHaveBeenCalled();
  wrapper.unmount();
});

it('cancels a rename without calling the backend', async () => {
  const wrapper = mount(BlueprintLibrary, { global:{stubs:{ExtensionMenuItems:true}}, props: { items: [item], busy: false } });

  await chooseAction(wrapper,'Rename');
  await wrapper.get('input:not([type="search"])').setValue('Changed name');
  await wrapper.findAll('button').find(button => button.text() === 'Cancel')!.trigger('click');

  expect(invoke).not.toHaveBeenCalled();
  expect(wrapper.text()).toContain('Workshop');
  expect(wrapper.find('input:not([type="search"])').exists()).toBe(false);
  wrapper.unmount();
});

it('renames a blueprint and emits the returned library', async () => {
  const renamed = [{ ...item, name: 'New workshop' }];
  vi.mocked(invoke).mockResolvedValue(renamed);
  const wrapper = mount(BlueprintLibrary, { global:{stubs:{ExtensionMenuItems:true}}, props: { items: [item], busy: false } });

  await chooseAction(wrapper,'Rename');
  await wrapper.get('input:not([type="search"])').setValue('  New workshop  ');
  expect(wrapper.findAll('button').find(button => button.text() === 'Save name')!.attributes('disabled')).toBeUndefined();
  await wrapper.get('form').trigger('submit');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('rename_blueprint', { id: item.id, name: 'New workshop' });
  expect(wrapper.emitted('changed')?.[0]).toEqual([renamed]);
  wrapper.unmount();
});

it('keeps the blueprint listed when removal fails', async () => {
  vi.mocked(invoke).mockRejectedValue(new Error('permission denied'));
  const wrapper = mount(BlueprintLibrary, { global:{stubs:{ExtensionMenuItems:true}}, props: { items: [item], busy: false } });

  await chooseAction(wrapper,'Remove');
  await wrapper.findAll('button').find(button => button.text() === 'Remove blueprint')!.trigger('click');
  await flushPromises();

  expect(invoke).toHaveBeenCalledWith('remove_blueprint', { id: item.id });
  expect(wrapper.text()).toContain('Workshop');
  expect(wrapper.get('[role="alert"]').text()).toContain('permission denied');
  expect(wrapper.emitted('changed')).toBeUndefined();
  wrapper.unmount();
});

it('places blueprint extension items in the native action menu with the selected context',async()=>{
 const wrapper=mount(BlueprintLibrary,{props:{items:[item],busy:false},global:{stubs:{ExtensionMenuItems:true}}});
 await wrapper.get('[aria-label="More actions for Workshop"]').trigger('keydown',{key:'Enter'});
 await flushPromises();
 const extension=new DOMWrapper(document.body).get('extension-menu-items-stub');
 expect(extension.attributes('position')).toBe('blueprint');
 const component=wrapper.findComponent(ExtensionMenuItems);
 expect(component.props('context')).toEqual({blueprintId:item.id});
 component.vm.$emit('selected',{pluginId:'viewer',commandId:'open',label:'Open',workflow:'viewer',blueprintId:item.id});
 await flushPromises();
 expect(wrapper.emitted('command')?.[0]?.[0]).toMatchObject({blueprintId:item.id});
 wrapper.unmount();
});

it('keeps an instance-context selection when importing a saved blueprint', async () => {
  vi.mocked(invoke).mockImplementation(async command => command === 'list_games' ? [{ id: 'context-game', name: 'Workshop world', status: 'installed', launch_state: 'idle' }] : []);
  const wrapper = mount(BlueprintLibrary, { props: { items: [item], busy: false, initialGameId: 'context-game' } });
  await wrapper.findAll('button').find(button => button.text() === 'Import to game')!.trigger('click');
  await flushPromises();
  expect(wrapper.get('#blueprint-target-blueprint-1').text()).toContain('Workshop world');
  wrapper.unmount();
});

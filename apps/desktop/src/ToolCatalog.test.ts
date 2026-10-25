// SPDX-License-Identifier: MIT
import { afterEach, expect, it } from 'vitest';
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import ToolCatalog, { type ToolCatalogPlugin } from './ToolCatalog.vue';
import { setLocale } from './i18n';
import { clearPageWorkspacesForTests } from './use-page-workspace';

const plugin: ToolCatalogPlugin = {
  manifest: { id: 'example.viewer', name: 'Map Viewer', description: 'Preview schematic maps', tags: ['schematic', 'preview'] },
  commands: [{ id: 'open', label: 'Open map' }],
};
afterEach(() => { setLocale('zh-CN'); clearPageWorkspacesForTests(); });

it('searches built-in and extension tools by name, description, and tags', async () => {
  setLocale('en-US');
  const wrapper = mount(ToolCatalog, { props: { plugins: [plugin] } });
  expect(wrapper.text()).toContain('Blueprint library');
  expect(wrapper.text()).toContain('Map Viewer');
  expect(wrapper.get('input[type="search"]').attributes('data-list-search')).toBe('');

  await wrapper.get('input[type="search"]').setValue('Map Viewer');
  expect(wrapper.text()).toContain('Map Viewer');
  expect(wrapper.text()).not.toContain('Blueprint library');
  await wrapper.get('input[type="search"]').setValue('preview');
  expect(wrapper.text()).toContain('Map Viewer');
  await wrapper.get('input[type="search"]').setValue('schematic');
  expect(wrapper.text()).toContain('Map Viewer');
  expect(wrapper.text()).toContain('Blueprint library');
  await wrapper.get('input[type="search"]').setValue('missing');
  expect(wrapper.text()).toContain('No tools found.');
  await wrapper.get('button[aria-label="Clear search"]').trigger('click');
  expect(wrapper.text()).toContain('Blueprint library');
  wrapper.unmount();
});

it('matches each search term across tool metadata and command labels', async () => {
  setLocale('en-US');
  const wrapper = mount(ToolCatalog, { props: { plugins: [plugin] } });
  await wrapper.get('input[type="search"]').setValue('open maps');
  expect(wrapper.text()).toContain('Map Viewer');
  expect(wrapper.text()).not.toContain('Blueprint library');
  wrapper.unmount();
});

it('routes the built-in library and a contribution action from the catalogue', async () => {
  setLocale('en-US');
  const multipleCommands = { ...plugin, manifest: { ...plugin.manifest, id: 'multi.tool', name: 'Multi Tool' }, commands: [{ id: 'first', label: 'First action' }, { id: 'second', label: 'Second action' }] };
  const legacyTool = { ...plugin, manifest: { ...plugin.manifest, id: 'legacy.tool', name: 'Legacy Tool' }, commands: [] };
  const wrapper = mount(ToolCatalog, { props: { plugins: [plugin, multipleCommands, legacyTool] } });
  await wrapper.findAll('button').find(button => button.text().includes('Blueprint library'))!.trigger('click');
  await wrapper.get('button[aria-label="Map Viewer"]').trigger('click');
  await wrapper.get('button[aria-label="Multi Tool"]').trigger('keydown', {key: 'Enter'}); await flushPromises();
  const menu = new DOMWrapper(document.body.querySelector('[role=menu]')!);
  await menu.findAll('[role=menuitem]').find(item => item.text() === 'First action')!.trigger('click'); await flushPromises();
  await wrapper.get('button[aria-label="Legacy Tool"]').trigger('click');
  expect(wrapper.emitted('blueprints')).toHaveLength(1);
  expect(wrapper.emitted('plugin')).toEqual([['legacy.tool']]);
  expect(wrapper.emitted('command')).toEqual([['example.viewer', 'open'], ['multi.tool', 'first']]);
  wrapper.unmount();
});

it('moves through tool entries with arrow keys and keeps Enter activation native', async () => {
  setLocale('en-US');
  const wrapper = mount(ToolCatalog, { props: { plugins: [plugin] }, attachTo: document.body });
  const cards = wrapper.findAll('.tool-list > li > button');
  (cards[0]!.element as HTMLElement).focus();
  await cards[0]!.trigger('keydown', { key: 'ArrowDown' });
  expect(document.activeElement).toBe(cards[1]!.element);
  const enter = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
  cards[1]!.element.dispatchEvent(enter);
  expect(enter.defaultPrevented).toBe(false);
  await cards[1]!.trigger('click');
  expect(wrapper.emitted('command')).toEqual([['example.viewer', 'open']]);
  wrapper.unmount();
});

it('keeps the tool list bounded and resets its page when the query changes', async () => {
  setLocale('en-US');
  const plugins = Array.from({ length: 10 }, (_, index) => ({
    ...plugin,
    manifest: { ...plugin.manifest, id: `tool.${index}`, name: `Tool ${index}`, publisher: index === 9 ? 'Searchable Publisher' : 'Other' },
    commands: [{ id: `open-${index}`, label: `Open ${index}` }],
  }));
  const wrapper = mount(ToolCatalog, { props: { plugins } });
  expect(wrapper.findAll('.tool-list > li')).toHaveLength(8);
  await wrapper.get('nav[aria-label="Plugin list pages"] button:last-child').trigger('click');
  expect(wrapper.findAll('.tool-list > li')).toHaveLength(3);
  await wrapper.get('button[aria-label="Tool 9"]').trigger('click');
  expect(wrapper.emitted('command')).toContainEqual(['tool.9', 'open-9']);
  await wrapper.get('input[type="search"]').setValue('Searchable Publisher');
  expect(wrapper.findAll('.tool-list > li')).toHaveLength(1);
  expect(wrapper.text()).toContain('Tool 9');
  wrapper.unmount();
});

it('restores the catalog search and page after leaving and reopening it for the same game', async () => {
  setLocale('en-US');
  const plugins = Array.from({ length: 10 }, (_, index) => ({ ...plugin, manifest: { ...plugin.manifest, id: `retained.${index}`, name: `Retained ${index}` } }));
  const first = mount(ToolCatalog, { props: { plugins, gameId: 'game-retained' } });
  await first.get('input[type="search"]').setValue('Retained');
  await first.get('nav[aria-label="Plugin list pages"] button:last-child').trigger('click');
  first.unmount();
  const reopened = mount(ToolCatalog, { props: { plugins, gameId: 'game-retained' } });
  expect(reopened.get('input[type="search"]').element).toHaveProperty('value', 'Retained');
  expect(reopened.findAll('.tool-list > li')).toHaveLength(2);
  expect(reopened.get('nav[aria-label="Plugin list pages"]').text()).toContain('Page 2 of 2');
  reopened.unmount();
});

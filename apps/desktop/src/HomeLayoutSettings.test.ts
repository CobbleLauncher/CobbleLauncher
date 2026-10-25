// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import HomeLayoutSettings from './HomeLayoutSettings.vue';
import { defaultHomeLayout, readHomeLayout, writeHomeLayout } from './home-layout';
import { setLocale } from './i18n';
beforeEach(() => { localStorage.clear(); setLocale('en-US'); });
afterEach(() => { localStorage.clear(); setLocale('zh-CN'); });
it('edits the home preview by drag and keyboard without changing the real home until save', async () => {
  const changed = vi.fn(); window.addEventListener('cobble-home-layout-changed', changed);
  const wrapper = mount(HomeLayoutSettings);
  await wrapper.get('[data-section="other"] .home-layout-drag').trigger('keydown', { key: 'ArrowDown' });
  expect(wrapper.findAll('[data-section]').map(row => row.attributes('data-section'))).toEqual(['quick', 'other', 'favorites']);
  expect(readHomeLayout()).toEqual(defaultHomeLayout); expect(changed).not.toHaveBeenCalled();
  const transfer = { setData: vi.fn(), effectAllowed: '' };
  await wrapper.get('[data-section="quick"] .home-layout-drag').trigger('dragstart', { dataTransfer: transfer });
  await wrapper.get('[data-section="favorites"]').trigger('drop');
  expect(wrapper.findAll('[data-section]').map(row => row.attributes('data-section'))).toEqual(['other', 'favorites', 'quick']);
  await wrapper.get('[role="switch"][aria-label="Show Saved projects"]').trigger('click');
  wrapper.vm.save(); await flushPromises();
  expect(readHomeLayout()).toMatchObject({ order: ['other', 'favorites', 'quick'], hidden: ['favorites'] });
  expect(changed).toHaveBeenCalledOnce();
  wrapper.unmount(); window.removeEventListener('cobble-home-layout-changed', changed);
});
it('confirms defaults before staging them and lets discard restore saved layout', async () => {
  writeHomeLayout({ ...defaultHomeLayout, order: ['favorites', 'quick', 'other'], hidden: ['other'], quickIds: ['tools.open'] });
  const wrapper = mount(HomeLayoutSettings, { attachTo: document.body });
  await wrapper.get('.home-layout-reset').trigger('click'); await flushPromises();
  expect(wrapper.vm.dirty).toBe(false);
  document.querySelector<HTMLButtonElement>('.confirmation-cancel')!.click(); await flushPromises();
  expect(wrapper.vm.dirty).toBe(false);
  await wrapper.get('.home-layout-reset').trigger('click'); await flushPromises();
  document.querySelector<HTMLButtonElement>('.confirmation-confirm')!.click(); await flushPromises();
  expect(wrapper.vm.dirty).toBe(true); expect(readHomeLayout().order[0]).toBe('favorites');
  wrapper.vm.cancel(); await flushPromises();
  expect(wrapper.findAll('[data-section]')[0]!.attributes('data-section')).toBe('favorites');
  expect(wrapper.vm.dirty).toBe(false); wrapper.unmount();
});

it('selects and orders quick actions as drafts without running them', async () => {
  const run = vi.fn();
  const wrapper = mount(HomeLayoutSettings, { props: { commands: [{ id: 'tools', label: 'Tools', run }, { id: 'market', label: 'Market', run }] } });
  await wrapper.get('.settings-disclosure-trigger').trigger('click');
  await wrapper.get('.home-quick-candidates label:first-child input').setValue(true);
  await wrapper.get('.home-quick-candidates label:last-child input').setValue(true);
  await wrapper.get('button[aria-label="Move Market up"]').trigger('click');
  expect(readHomeLayout().quickIds).toEqual([]); expect(run).not.toHaveBeenCalled();
  wrapper.vm.save();
  expect(readHomeLayout().quickIds).toEqual(['market', 'tools']); wrapper.unmount();
});

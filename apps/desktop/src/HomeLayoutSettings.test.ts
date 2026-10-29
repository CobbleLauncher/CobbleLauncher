// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import HomeLayoutSettings from './HomeLayoutSettings.vue';
import { defaultHomeLayout, readHomeLayout, writeHomeLayout } from './home-layout';
import { setLocale } from './i18n';
beforeEach(() => { localStorage.clear(); setLocale('en-US'); });
afterEach(() => { localStorage.clear(); setLocale('zh-CN'); });
it('stages visibility until save and preserves legacy ordering', async () => {
  writeHomeLayout({ ...defaultHomeLayout, order: ['favorites', 'other', 'quick'] });
  const changed = vi.fn(); window.addEventListener('cobble-home-layout-changed', changed);
  const wrapper = mount(HomeLayoutSettings);
  await wrapper.get('[role="switch"][aria-label="Show Saved projects"]').trigger('click');
  expect(readHomeLayout().hidden).toEqual([]); expect(changed).not.toHaveBeenCalled();
  wrapper.vm.save(); await flushPromises();
  expect(readHomeLayout()).toMatchObject({ order: ['favorites', 'other', 'quick'], hidden: ['favorites'] });
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
  expect(wrapper.get('[data-section="other"] [role="switch"]').attributes('data-state')).toBe('unchecked');
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

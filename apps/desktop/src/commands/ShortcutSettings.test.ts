// SPDX-License-Identifier: MIT
import { afterEach, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import ShortcutSettings from './ShortcutSettings.vue';
import { readShortcuts, resetShortcuts, saveShortcut } from './shortcuts';
import { setLocale } from '../i18n';

afterEach(() => { resetShortcuts(); localStorage.clear(); setLocale('zh-CN'); });

it('shows a saved binding for an unavailable action and clears it individually', async () => {
  setLocale('en-US');
  saveShortcut('plugin:disabled:open', 'Ctrl+Shift+P');
  const wrapper = mount(ShortcutSettings);
  expect(wrapper.text()).toContain('plugin:disabled:open');
  await wrapper.get('[aria-label="Clear Unavailable action: plugin:disabled:open"]').trigger('click');
  expect(wrapper.text()).not.toContain('plugin:disabled:open');
  wrapper.unmount();
});

it('stages bindings and conflict checks until the settings owner saves or discards', async () => {
  setLocale('en-US');
  const wrapper = mount(ShortcutSettings, { props: { staged: true } });
  const binding = wrapper.get('button[aria-label="Search actions: Ctrl+K"]');
  await binding.trigger('click'); await binding.trigger('keydown', { key: 'P', ctrlKey: true });
  expect(readShortcuts()['palette.open']).toBe('Ctrl+K'); expect(wrapper.vm.dirty).toBe(true);
  const search = wrapper.get('button[aria-label="Search current list: Ctrl+F"]');
  await search.trigger('click'); await search.trigger('keydown', { key: 'P', ctrlKey: true });
  expect(wrapper.get('[role="alert"]').text()).toBe('Shortcut already in use');
  wrapper.vm.cancel(); expect(readShortcuts()['palette.open']).toBe('Ctrl+K');
  await binding.trigger('click'); await binding.trigger('keydown', { key: 'P', ctrlKey: true });
  expect(wrapper.vm.save()).toBe(true); expect(readShortcuts()['palette.open']).toBe('Ctrl+P'); wrapper.unmount();
});

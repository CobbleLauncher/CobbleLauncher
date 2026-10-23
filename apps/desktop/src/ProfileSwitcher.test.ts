// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { afterEach, expect, it } from 'vitest';
import { flushPromises } from '@vue/test-utils';
import ProfileSwitcher from './ProfileSwitcher.vue';
import { setLocale } from './i18n';

const profile = { id: 'offline-1', kind: 'offline' as const, name: 'Luna', uuid: 'offline-uuid', note: null };
const library = { profiles: [profile], active_id: profile.id };

afterEach(() => { setLocale('zh-CN'); document.body.innerHTML = ''; });

it('shows translated profile menu actions and emits their events', async () => {
  setLocale('en-US');
  const wrapper = mount(ProfileSwitcher, {
    attachTo: document.body,
    props: { library, busy: false, ready: true },
  });

  expect(document.body.querySelector('button[aria-label="Switch player profile"]')).not.toBeNull();
  expect(wrapper.text()).toContain('Offline');

  const trigger = document.body.querySelector('button[aria-label="Switch player profile"]');
  trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  await flushPromises();
  expect(document.body.textContent).toContain('Manage profiles');
  expect(document.body.textContent).toContain('Add profile');

  const manage = [...document.body.querySelectorAll('[role="menuitem"]')].find((item) => item.textContent?.includes('Manage profiles'));
  manage?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  await flushPromises();
  expect(wrapper.emitted('manage')).toHaveLength(1);

  const addTrigger = document.body.querySelector('button[aria-label="Switch player profile"]');
  addTrigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  await flushPromises();
  const add = [...document.body.querySelectorAll('[role="menuitem"]')].find((item) => item.textContent?.includes('Add profile'));
  add?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  await flushPromises();
  expect(wrapper.emitted('add')).toHaveLength(1);
  wrapper.unmount();
});
it('waits for the native view suspension before opening the profile menu', async () => {
  let finish!: () => void;
  const beforeOpen = () => new Promise<void>(resolve => { finish = resolve; });
  const wrapper = mount(ProfileSwitcher, { attachTo: document.body, props: { library, busy: false, ready: true, beforeOpen } });
  await wrapper.find('button').trigger('click');
  await flushPromises();
  expect(wrapper.find('button').attributes('aria-expanded')).toBe('false');
  finish();
  await flushPromises();
  expect(wrapper.find('button').attributes('aria-expanded')).toBe('true');
  expect(wrapper.emitted('open-change')).toEqual([[true]]);
  wrapper.unmount();
});

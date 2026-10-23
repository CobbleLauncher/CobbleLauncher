// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import MarketGallery from './MarketGallery.vue';
import { setLocale } from './i18n';

vi.mock('./market-images', () => ({ marketImage: vi.fn(async () => 'data:image/png;base64,fixture') }));
const urls = ['https://cdn.modrinth.com/a.png', 'https://cdn.modrinth.com/b.png', 'https://cdn.modrinth.com/c.png'];
afterEach(() => { document.body.innerHTML = ''; setLocale('zh-CN'); });

it('moves thumbnail selection with a single tab stop and restores focus after enlarging', async () => {
  setLocale('en-US');
  const wrapper = mount(MarketGallery, { attachTo: document.body, props: { provider: 'modrinth', urls, title: 'Workshop' } });
  await flushPromises();
  const choices = () => wrapper.findAll('.market-gallery-choice');
  expect(choices().map(choice => choice.attributes('tabindex'))).toEqual(['0', '-1', '-1']);
  await choices()[0]!.trigger('keydown', { key: 'ArrowRight' });
  expect(choices()[1]!.attributes('aria-pressed')).toBe('true');
  expect(document.activeElement).toBe(choices()[1]!.element);
  await choices()[1]!.trigger('keydown', { key: 'End' });
  expect(choices()[2]!.attributes('tabindex')).toBe('0');
  const trigger = wrapper.get('.market-gallery-open');
  (trigger.element as HTMLButtonElement).focus();
  await trigger.trigger('click'); await flushPromises();
  const dialog = document.querySelector<HTMLElement>('[role="dialog"]')!;
  expect(dialog).not.toBeNull();
  expect(dialog.textContent).toContain('3 / 3');
  dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await flushPromises();
  expect(dialog.textContent).toContain('1 / 3');
  dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await flushPromises();
  expect(document.querySelector('[role="dialog"]')).toBeNull();
  expect(document.activeElement).toBe(trigger.element);
  wrapper.unmount();
});

it('drops failed images and closes the preview when the project changes', async () => {
  const wrapper = mount(MarketGallery, { attachTo: document.body, props: { provider: 'modrinth', urls: [urls[0]!, urls[0]!, urls[1]!], title: 'Workshop' } });
  await flushPromises();
  expect(wrapper.findAll('.market-gallery-choice')).toHaveLength(2);
  await wrapper.get('.market-gallery-open img').trigger('error'); await flushPromises();
  expect(wrapper.findAll('.market-gallery-choice')).toHaveLength(0);
  expect(wrapper.get('.market-gallery-open img').attributes('alt')).toContain('1');
  await wrapper.get('.market-gallery-open').trigger('click'); await flushPromises();
  expect(document.querySelector('[role="dialog"]')).not.toBeNull();
  await wrapper.setProps({ provider: 'curseforge', urls: ['https://media.forgecdn.net/new.png'], title: 'New project' });
  await flushPromises();
  expect(document.querySelector('[role="dialog"]')).toBeNull();
  expect(wrapper.get('.market-gallery-open').attributes('aria-label')).toContain('New project');
  wrapper.unmount();
});

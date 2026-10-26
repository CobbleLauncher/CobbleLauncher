// SPDX-License-Identifier: MIT
import { afterEach, expect, it } from 'vitest';
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils';
import MarketToolbar from './MarketToolbar.vue';
import { setLocale } from '../i18n';
afterEach(() => { document.body.innerHTML=''; setLocale('zh-CN'); });
it('keeps real version filters visible and puts only source selection in a compact popover', async () => {
  setLocale('en-US');
  const wrapper = mount(MarketToolbar,{attachTo:document.body,props:{kind:'modpack',categories:[],targets:[],gameId:'',provider:'all',query:'',minecraftVersion:'',loader:'',busy:false,cfAvailable:false}});
  expect(wrapper.find('.settings-disclosure').exists()).toBe(false);
  expect(wrapper.find('.market-pack-filters input').exists()).toBe(false);
  expect(wrapper.get('.market-version-trigger').isVisible()).toBe(true);
  expect(document.querySelector('.market-source-popover')).toBeNull();
  await wrapper.get('button[aria-label=Sources]').trigger('click'); await flushPromises();
  const popup = new DOMWrapper(document.body.querySelector('.market-source-popover')!);
  expect(popup.get('[data-state=checked]').text()).toBe('All sources');
  expect(popup.findAll('[role=radio]').find(item => item.text().includes('CurseForge'))!.attributes('disabled')).toBeDefined();
  await popup.findAll('[role=radio]').find(item => item.text() === 'Modrinth')!.trigger('click');
  expect(wrapper.emitted('provider')).toEqual([['modrinth']]);
  wrapper.unmount();
});

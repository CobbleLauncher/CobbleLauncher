// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { afterEach, expect, it } from 'vitest';
import FirstRunPage from './FirstRunPage.vue';
import { FIRST_RUN_CONSENT_VERSION, nextFirstRunStep } from './first-run';
import { locale } from './i18n';
afterEach(() => { locale.value = 'zh-CN'; });
const base = { step: 'welcome' as const, consentAccepted: false, meshChoice: null };
it('requires an explicit unchecked-by-default consent action', async () => {
  const wrapper = mount(FirstRunPage, { props: { ...base, step: 'consent' } });
  const accept = wrapper.findAll('button').find(item => item.text().includes('同意并继续'))!;
  expect(accept.attributes('disabled')).toBeDefined();
  await wrapper.get('input').setValue(true);
  await accept.trigger('click');
  expect(wrapper.emitted('acceptConsent')).toEqual([[FIRST_RUN_CONSENT_VERSION]]);
  await wrapper.setProps({ step: 'profile', consentAccepted: true });
  await wrapper.setProps({ step: 'consent' });
  expect((wrapper.get('input').element as HTMLInputElement).checked).toBe(false);
  wrapper.unmount();
});
it('allows deferring only the guide, never consent or a Mesh decision', async () => {
  const wrapper = mount(FirstRunPage, { props: { ...base, consentAccepted: true } });
  await wrapper.findAll('button').find(item => item.text() === '稍后设置')!.trigger('click');
  expect(wrapper.emitted('defer')).toHaveLength(1);
  expect(wrapper.emitted('acceptConsent')).toBeUndefined();
  expect(wrapper.emitted('meshChoice')).toBeUndefined();
  wrapper.unmount();
});
it('routes account creation through the existing page only after consent', async () => {
  const wrapper = mount(FirstRunPage, { props: { ...base, step: 'profile' } });
  expect(wrapper.findAll('button').some(item => item.text() === '添加档案')).toBe(false);
  expect(wrapper.findAll('button').some(item => item.text() === '稍后设置')).toBe(false);
  await wrapper.setProps({ consentAccepted: true, profileReady: true });
  await wrapper.get('[aria-label="离线档案"]').trigger('click');
  await wrapper.get('#guided-player-name').setValue('Builder');
  await wrapper.get('form').trigger('submit');
  expect(wrapper.emitted('addOffline')).toEqual([['Builder', null]]);
  wrapper.unmount();
});
it('offers an explicit off decision and no preselected Mesh opt-in', async () => {
  const wrapper = mount(FirstRunPage, { props: { ...base, step: 'mesh', consentAccepted: true } });
  expect(wrapper.emitted('meshChoice')).toBeUndefined();
  await wrapper.findAll('button').find(item => item.text() === '保持关闭')!.trigger('click');
  expect(wrapper.emitted('meshChoice')).toEqual([[false]]);
  wrapper.unmount();
});
it('shows readable local privacy details in English', async () => {
  locale.value = 'en-US';
  const wrapper = mount(FirstRunPage, { props: { ...base, step: 'consent' } });
  await wrapper.findAll('button').find(item => item.text() === 'Privacy notice')!.trigger('click');
  expect(wrapper.get('article').text()).toContain('seed or relay services');
  expect(wrapper.get('article').text()).toContain('network addresses');
  expect(nextFirstRunStep('consent')).toBe('welcome');
  expect(nextFirstRunStep('mesh')).toBe('mesh');
  wrapper.unmount();
});
it('keeps consent outside the four-step guide, including legacy welcome state', async () => {
  const wrapper = mount(FirstRunPage, { props: base });
  expect(wrapper.find('.guide-progress').exists()).toBe(false);
  expect(wrapper.find('.consent-document').exists()).toBe(true);
  expect(wrapper.findAll('button').some(item => /稍后|上一步/.test(item.text()))).toBe(false);
  await wrapper.setProps({ consentAccepted: true });
  expect(wrapper.get('.guide-count').text()).toBe('1 / 4');
  await wrapper.findAll('button').find(item => item.text() === '开始设置')!.trigger('click');
  expect(wrapper.emitted('step')).toEqual([['profile']]);
  wrapper.unmount();
});

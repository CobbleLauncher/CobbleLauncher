// SPDX-License-Identifier: MIT
import { afterEach, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import MinecraftVersionTiles from './MinecraftVersionTiles.vue';
import { setLocale } from './i18n';
const versions = [
  { id: '26w01a', kind: 'snapshot' },
  ...Array.from({ length: 40 }, (_, index) => ({ id: `1.21.${40-index}`, kind: 'release' })),
  { id: 'b1.7.3', kind: 'old_beta' },
];
afterEach(() => setLocale('zh-CN'));
it('shows recent releases as radios and retains the selected older version without fetching data', async () => {
  const wrapper = mount(MinecraftVersionTiles, { props: { id:'mc', modelValue:'1.21.1', versions } });
  expect(wrapper.findAll('[role=radio]')).toHaveLength(6);
  expect(wrapper.get('[data-state=checked]').text()).toContain('1.21.1');
  expect(wrapper.text()).not.toContain('26w01a');
  await wrapper.findAll('[role=radio]')[0]!.trigger('click');
  expect(wrapper.emitted('update:modelValue')).toEqual([['1.21.40']]);
  wrapper.unmount();
});
it('searches all loaded releases, browses snapshots and legacy versions, and keeps the selected tile visible', async () => {
  setLocale('en-US');
  const wrapper = mount(MinecraftVersionTiles, { props:{ id:'mc',modelValue:'1.21.40',versions } });
  await wrapper.findAll('button').find(button => button.text().includes('More versions'))!.trigger('click');
  expect(wrapper.findAll('[role=radio]')).toHaveLength(20);
  await wrapper.get('input[type=search]').setValue('1.21.1');
  expect(wrapper.findAll('[role=radio]').some(radio => radio.text().includes('1.21.1'))).toBe(true);
  await wrapper.findAll('.minecraft-version-kinds button').find(button => button.text() === 'Snapshots')!.trigger('click');
  expect(wrapper.text()).toContain('26w01a');
  expect(wrapper.get('[data-state=checked]').text()).toContain('1.21.40');
  await wrapper.findAll('.minecraft-version-kinds button').find(button => button.text() === 'Older editions')!.trigger('click');
  expect(wrapper.text()).toContain('b1.7.3');
  await wrapper.findAll('button').find(button => button.text() === 'Show fewer')!.trigger('click');
  expect(wrapper.find('input[type=search]').exists()).toBe(false);
  expect(wrapper.findAll('[role=radio]')).toHaveLength(5);
  wrapper.unmount();
});

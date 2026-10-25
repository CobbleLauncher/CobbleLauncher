// SPDX-License-Identifier: MIT
import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import GameDirectoryMigration from './GameDirectoryMigration.vue';
it('keeps settings to one move entry without controls or native reads', async () => {
  const wrapper = mount(GameDirectoryMigration);
  expect(wrapper.findAll('button')).toHaveLength(1);
  expect(wrapper.find('progress').exists()).toBe(false);
  await wrapper.get('button').trigger('click'); expect(wrapper.emitted('open')).toHaveLength(1);
  wrapper.unmount();
});
it('honors pending settings drafts', async () => {
  const wrapper = mount(GameDirectoryMigration, { props: { disabled: true } });
  await wrapper.get('button').trigger('click'); expect(wrapper.emitted('open')).toBeUndefined(); wrapper.unmount();
});

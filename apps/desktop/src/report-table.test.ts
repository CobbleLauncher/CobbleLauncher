// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import ReportTable from './ReportTable.vue';

it('bounds rendered rows, searches across pages, and resets on a new report', async () => {
  const table = { columns: ['File'], rows: Array.from({ length: 121 }, (_, index) => [`file-${index}.jar`]) };
  const wrapper = mount(ReportTable, { props: { table, index: 1 } });
  expect(wrapper.findAll('tbody tr')).toHaveLength(50);
  const next = wrapper.findAll('button')[1]!;
  await next.trigger('click');
  expect(wrapper.find('tbody').text()).toContain('file-50.jar');
  await next.trigger('click');
  expect(wrapper.findAll('tbody tr')).toHaveLength(21);
  expect(next.attributes('disabled')).toBeDefined();
  await wrapper.find('input').setValue('FILE-120');
  expect(wrapper.findAll('tbody tr')).toHaveLength(1);
  expect(wrapper.find('tbody').text()).toBe('file-120.jar');
  await wrapper.find('input').setValue('absent');
  expect(wrapper.findAll('tbody tr')).toHaveLength(0);
  expect(wrapper.findAll('button').every(button => button.attributes('disabled') !== undefined)).toBe(true);
  await wrapper.setProps({ table: { columns: ['File'], rows: [['<script>untrusted</script>']] } });
  expect(wrapper.find('input').exists()).toBe(false);
  expect(wrapper.find('tbody').text()).toBe('<script>untrusted</script>');
  expect(wrapper.find('script').exists()).toBe(false);
  wrapper.unmount();
});

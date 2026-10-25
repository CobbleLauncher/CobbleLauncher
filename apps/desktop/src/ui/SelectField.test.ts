// SPDX-License-Identifier: MIT
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it } from 'vitest';
import SelectField from './SelectField.vue';

const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => { for (const wrapper of wrappers.splice(0)) wrapper.unmount(); });

it('renders an accessible trigger, grouped choices, and emits a selected value', async () => {
  const wrapper = mount(SelectField, {
    attachTo: document.body,
    props: {
      id: 'runtime-mode',
      label: 'Java runtime',
      modelValue: 'inherit',
      options: [
        { value: 'inherit', label: 'Use default' },
        { value: 'automatic', label: 'Automatic', group: 'Choose Java' },
        { value: 'unavailable', label: 'Unavailable', group: 'Choose Java', disabled: true },
      ],
    },
  });
  wrappers.push(wrapper);

  const trigger = wrapper.get('button');
  expect(trigger.attributes()).toMatchObject({ id: 'runtime-mode', 'aria-label': 'Java runtime', role: 'combobox' });
  expect(trigger.text()).toContain('Use default');
  await trigger.trigger('keydown', { key: 'Enter' });
  await flushPromises();
  expect(trigger.attributes('aria-expanded')).toBe('true');
  expect(document.body.textContent).toContain('Choose Java');
  expect(document.body.querySelector('[role="option"][data-disabled]')?.textContent).toContain('Unavailable');
  const automatic = [...document.body.querySelectorAll<HTMLElement>('[role="option"]')].find(option => option.textContent?.includes('Automatic'))!;
  automatic.dispatchEvent(new MouseEvent('pointerup', { bubbles: true }));
  await flushPromises();
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['automatic']);
});

it('supports selecting with the keyboard and disables the trigger', async () => {
  const wrapper = mount(SelectField, {
    attachTo: document.body,
    props: {
      id: 'memory-mode',
      label: 'Memory mode',
      modelValue: 'inherit',
      disabled: true,
      options: [{ value: 'inherit', label: 'Use default' }, { value: 'custom', label: 'Custom' }],
    },
  });
  wrappers.push(wrapper);
  expect((wrapper.get('button').element as HTMLButtonElement).disabled).toBe(true);
  await wrapper.setProps({ disabled: false });
  const trigger = wrapper.get('button');
  await trigger.trigger('keydown', { key: 'Enter' });
  await flushPromises();
  expect(trigger.attributes('aria-expanded')).toBe('true');
  expect(document.body.querySelectorAll('[role="option"]')).toHaveLength(2);
});

it('supports an empty public option without colliding with real option IDs', async () => {
  const wrapper = mount(SelectField, { attachTo: document.body, props: { id: 'loader', label: 'Loader', modelValue: 'fabric', options: [
    { value: '', label: 'Any loader' }, { value: 'fabric', label: 'Fabric' }, { value: '__cobble_empty_selection__', label: 'Custom loader' },
  ] } });
  wrappers.push(wrapper);
  const trigger = wrapper.get('button');
  await trigger.trigger('keydown', { key: 'Enter' }); await flushPromises();
  const any = [...document.body.querySelectorAll<HTMLElement>('[role="option"]')].find(option => option.textContent?.includes('Any loader'))!;
  any.dispatchEvent(new MouseEvent('pointerup', { bubbles: true })); await flushPromises();
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['']);
  await wrapper.setProps({ modelValue: '' });
  expect(trigger.text()).toContain('Any loader');
  await trigger.trigger('keydown', { key: 'Enter' }); await flushPromises();
  const custom = [...document.body.querySelectorAll<HTMLElement>('[role="option"]')].find(option => option.textContent?.includes('Custom loader'))!;
  custom.dispatchEvent(new MouseEvent('pointerup', { bubbles: true })); await flushPromises();
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['__cobble_empty_selection__']);
});

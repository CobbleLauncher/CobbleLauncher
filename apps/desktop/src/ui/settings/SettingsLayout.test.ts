// SPDX-License-Identifier: MIT
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, h, ref } from 'vue';
import { describe, expect, it } from 'vitest';
import SettingsRow from './SettingsRow.vue';
import SettingsSection from './SettingsSection.vue';
import SettingsDisclosure from './SettingsDisclosure.vue';
import SettingsActions from './SettingsActions.vue';

describe('shared settings layout', () => {
  it('associates a label, help and live validation with its control group', async () => {
    const row = mount(SettingsRow, { props: { label: 'Download limit', inputId: 'limit', help: 'At most four per origin', error: 'Choose a positive number' }, slots: { default: '<input id="limit" type="number">' } });
    expect(row.get('label').attributes('for')).toBe('limit');
    const group = row.get('[role="group"]');
    expect(group.attributes('aria-labelledby')).toBe(row.get('label').attributes('id'));
    expect(group.attributes('aria-describedby')?.split(' ')).toEqual([row.get('.setting-hint').attributes('id'), row.get('[role="alert"]').attributes('id')]);
    expect(group.attributes('aria-invalid')).toBe('true');
    await row.setProps({ error: undefined });
    expect(row.find('[role="alert"]').exists()).toBe(false);
    expect(group.attributes('aria-invalid')).toBeUndefined();
    expect(group.attributes('aria-describedby')).toBe(row.get('.setting-hint').attributes('id'));
    row.unmount();
  });

  it('keeps distinct row associations and does not invent headings or help', () => {
    const component = defineComponent({ render: () => h(SettingsSection, {}, { default: () => [h(SettingsRow, { label: 'Theme' }), h(SettingsRow, { label: 'Language' })] }) });
    const wrapper = mount(component);
    const groups = wrapper.findAll('[role="group"]');
    expect(groups).toHaveLength(2);
    expect(groups[0]!.attributes('aria-labelledby')).not.toBe(groups[1]!.attributes('aria-labelledby'));
    expect(wrapper.find('h3').exists()).toBe(false);
    expect(wrapper.find('.setting-hint').exists()).toBe(false);
    expect(groups.every(group => group.attributes('aria-describedby') === undefined)).toBe(true);
    wrapper.unmount();
  });

  it('uses the existing accessible disclosure button and preserves collapsed content', async () => {
    const draft = ref('retained');
    const wrapper = mount(SettingsDisclosure, { attachTo: document.body, props: { title: 'Advanced', bordered: true }, slots: { default: () => h('input', { 'aria-label': 'Arguments', value: draft.value, onInput: (event: Event) => draft.value = (event.target as HTMLInputElement).value }) } });
    await flushPromises();
    const trigger = wrapper.get('button');
    expect(trigger.attributes('type')).toBe('button');
    expect(trigger.attributes('aria-expanded')).toBe('false');
    const region = wrapper.get('.settings-disclosure-region');
    expect(region.attributes('id')).toBe(trigger.attributes('aria-controls'));
    expect(region.attributes('inert')).toBeDefined();
    expect(wrapper.get('input').isVisible()).toBe(false);
    expect(wrapper.find('summary').exists()).toBe(false);
    expect(wrapper.find('.settings-disclosure-leading').exists()).toBe(false);
    expect(wrapper.find('.settings-disclosure-meta').exists()).toBe(false);
    await trigger.trigger('click');
    await flushPromises();
    expect(trigger.attributes('aria-expanded')).toBe('true');
    expect(wrapper.get('input').isVisible()).toBe(true);
    expect(wrapper.get('input').element.value).toBe('retained');
    const input = wrapper.get('input').element;
    await wrapper.get('input').setValue('edited draft');
    wrapper.get('input').element.focus();
    await trigger.trigger('click');
    await flushPromises();
    expect(trigger.attributes('aria-expanded')).toBe('false');
    expect(wrapper.get('input').isVisible()).toBe(false);
    expect(wrapper.get('input').element.value).toBe('edited draft');
    expect(wrapper.get('input').element).toBe(input);
    expect(document.activeElement).toBe(trigger.element);
    expect(region.attributes('inert')).toBeDefined();
    await trigger.trigger('click');
    await flushPromises();
    expect(region.attributes('inert')).toBeUndefined();
    expect(wrapper.get('input').element.value).toBe('edited draft');
    expect(wrapper.emitted('update:open')).toEqual([[true], [false], [true]]);
    wrapper.unmount();
  });

  it('keeps optional icon and state metadata inside one accessible disclosure trigger', async () => {
    const wrapper = mount(SettingsDisclosure, { attachTo: document.body, props: { title: 'Downloads', icon: 'download', value: '12 / 4' }, slots: { meta: '<span>12 active</span>', default: '<input aria-label="Limit" value="12">' } });
    const trigger = wrapper.get('button');
    expect(trigger.text()).toContain('Downloads');
    expect(wrapper.get('.settings-disclosure-leading').attributes('aria-hidden')).toBe('true');
    expect(wrapper.get('.settings-disclosure-meta').text()).toBe('12 active');
    expect(wrapper.findAll('button')).toHaveLength(1);
    await trigger.trigger('click'); await flushPromises();
    expect(trigger.attributes('aria-expanded')).toBe('true');
    expect(wrapper.get('input').isVisible()).toBe(true);
    wrapper.unmount();
  });

  it('keeps explicit apply/cancel actions and blocks duplicate work while busy', async () => {
    const wrapper = mount(SettingsActions, { props: { dirty: false } });
    expect(wrapper.find('button').exists()).toBe(false);
    await wrapper.setProps({ dirty: true });
    await wrapper.get('.secondary-button').trigger('click');
    await wrapper.get('.text-button').trigger('click');
    expect(wrapper.emitted('apply')).toHaveLength(1);
    expect(wrapper.emitted('cancel')).toHaveLength(1);
    await wrapper.setProps({ busy: true });
    expect(wrapper.findAll('button').every(button => button.attributes('disabled') !== undefined)).toBe(true);
    await wrapper.get('.secondary-button').trigger('click');
    expect(wrapper.emitted('apply')).toHaveLength(1);
    wrapper.unmount();
  });
});

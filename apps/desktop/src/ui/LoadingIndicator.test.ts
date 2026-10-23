// SPDX-License-Identifier: MIT
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import LoadingIndicator from './LoadingIndicator.vue';

describe('LoadingIndicator', () => {
  it('announces its label as a polite status and keeps the icon decorative', () => {
    const wrapper = mount(LoadingIndicator, { props: { label: 'Searching catalog' } });
    expect(wrapper.get('[role="status"]').attributes('aria-live')).toBe('polite');
    expect(wrapper.get('[role="status"]').text()).toBe('Searching catalog');
    expect(wrapper.get('svg').attributes('aria-hidden')).toBe('true');
    expect(wrapper.classes()).not.toContain('compact');
  });

  it('supports a compact refresh status without adding extra copy', () => {
    const wrapper = mount(LoadingIndicator, { props: { label: 'Refreshing', compact: true } });
    expect(wrapper.classes()).toContain('compact');
    expect(wrapper.get('[role="status"]').text()).toBe('Refreshing');
  });

  it('announces an initial skeleton while hiding placeholder rows from assistive technology', () => {
    const wrapper = mount(LoadingIndicator, { props: { label: 'Loading catalog', skeleton: true } });
    expect(wrapper.get('[role="status"]').text()).toBe('Loading catalog');
    expect(wrapper.findAll('.skeleton-row')).toHaveLength(3);
    expect(wrapper.findAll('[aria-hidden="true"]')).toHaveLength(3);
  });
});

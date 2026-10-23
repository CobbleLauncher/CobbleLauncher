// SPDX-License-Identifier: MIT
import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import VersionSelector from './VersionSelector.vue';
import { preferredRelease, suggestedGameName, visibleVersions } from './version-selector';
const options = Array.from({ length: 60 }, (_, index) => ({ value: String(index), label: `Version ${index}` }));
describe('version selector', () => {
  it('prefers the newest compatible release and falls back to previews when necessary', () => {
    const preview = { version_type: 'beta', version_id: 'new-beta' };
    const release = { version_type: 'release', version_id: 'latest-release' };
    expect(preferredRelease([preview, release, { ...release, version_id: 'old' }])).toBe(release);
    expect(preferredRelease([preview])).toBe(preview);
    expect(preferredRelease([])).toBeUndefined();
  });
  it('shows five recent versions and preserves selected/current older entries', () => {
    const result = visibleVersions(options, '', 5, ['40', '50']);
    expect(result.options.map(option => option.value)).toEqual(['0', '1', '2', '3', '4', '40', '50']);
    expect(result.more).toBe(true);
    expect(visibleVersions(options, '', 25, []).options).toHaveLength(25);
  });
  it('searches versions outside the displayed page and hides unrelated pinned entries', () => {
    expect(visibleVersions(options, 'version 59', 5, ['40']).options).toEqual([options[59]]);
    expect(visibleVersions(options, 'missing', 5, ['40']).options).toEqual([]);
  });
  it('opens an accessible combobox and loads twenty additional options', async () => {
    HTMLElement.prototype.scrollIntoView = vi.fn();
    const wrapper = mount(VersionSelector, { props: { id: 'test-version', modelValue: '0', options }, attachTo: document.body });
    await wrapper.get('button[aria-label="展开版本"]').trigger('click');
    await nextTick();
    expect(wrapper.find('[role="combobox"]').attributes('aria-expanded')).toBe('true');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(5);
    await wrapper.get('.version-selector-more').trigger('click');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(25);
    await wrapper.get('input').setValue('Version 59');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(1);
    expect(wrapper.get('[role="option"]').text()).toBe('Version 59');
    await wrapper.get('input').trigger('keydown', { key: 'ArrowDown' });
    await wrapper.get('input').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual(['59']);
    wrapper.unmount();
  });
  it('builds names from Minecraft and loader versions', () => {
    expect(suggestedGameName('1.21.1', 'vanilla', '')).toBe('Minecraft 1.21.1');
    expect(suggestedGameName('1.21.1', 'fabric', '0.16')).toBe('Minecraft 1.21.1 Fabric 0.16');
  });
});

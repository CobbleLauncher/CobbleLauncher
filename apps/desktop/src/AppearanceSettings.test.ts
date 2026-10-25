// SPDX-License-Identifier: MIT
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import AppearanceSettings from './AppearanceSettings.vue';
import GameCard from './GameCard.vue';
import { applyAppearance, defaultAppearance, readAppearance } from './theme';
import { setLocale } from './i18n';

const wrappers: ReturnType<typeof mount>[] = [];
function editor() { const wrapper = mount(AppearanceSettings, { attachTo: document.body }); wrappers.push(wrapper); return wrapper; }
beforeEach(() => {
  localStorage.clear(); setLocale('en-US'); applyAppearance(defaultAppearance);
  vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} });
});
afterEach(() => { for (const wrapper of wrappers.splice(0)) wrapper.unmount(); localStorage.clear(); vi.unstubAllGlobals(); setLocale('zh-CN'); });

describe('appearance draft editor', () => {
  it('labels the sample and blocks its shared launch and menu controls', async () => {
    const wrapper = editor();
    expect(wrapper.get('.appearance-preview').text()).toContain('Display only');
    expect(wrapper.get('.appearance-preview-content').attributes('inert')).toBeDefined();
    expect(wrapper.get('.appearance-preview-content').attributes('aria-hidden')).toBe('true');
    await wrapper.get('.appearance-preview .library-game-play').trigger('click');
    await wrapper.get('.appearance-preview .game-menu-trigger').trigger('click');
    await flushPromises();
    expect(wrapper.findComponent(GameCard).emitted('play')).toBeUndefined();
    expect(document.querySelector('[role="menu"]')).toBeNull();
  });
  it('uses real shared components and keeps changes inside the preview until saved', async () => {
    const wrapper = editor();
    expect(wrapper.findComponent(GameCard).exists()).toBe(true);
    expect(wrapper.find('.appearance-save').exists()).toBe(false);
    const rootStyle = document.documentElement.style.cssText;
    await wrapper.get('input[name="appearance-accent"][value="blue"]').setValue();
    await wrapper.get('input[name="appearance-theme"][value="dark"]').setValue();
    await wrapper.get('#appearance-scale').setValue('105');
    await wrapper.get('#appearance-text-size').setValue('17');
    const density = wrapper.get('[role="slider"][aria-label="Density"]');
    await density.trigger('keydown', { key: 'End' });
    const preview = wrapper.get('.appearance-preview-canvas');
    expect(preview.attributes()).toMatchObject({ 'data-theme': 'dark', 'data-accent': 'blue', 'data-density': 'comfortable' });
    expect((preview.element as HTMLElement).style.getPropertyValue('--font-size-base')).toBe('17px');
    expect((preview.element as HTMLElement).style.zoom).toBe('calc(1.05 / var(--appearance-scale, 1))');
    expect(density.attributes('aria-valuetext')).toBe('Comfortable');
    expect(readAppearance()).toEqual(defaultAppearance);
    expect(document.documentElement.style.cssText).toBe(rootStyle);
    expect(document.documentElement.dataset.accent).toBe('green');
    wrapper.vm.save(); await flushPromises();
    expect(readAppearance()).toMatchObject({ theme: 'dark', accent: 'blue', scale: 105, textSize: 17, density: 'comfortable' });
    expect(wrapper.find('.appearance-save').exists()).toBe(false);
    expect(wrapper.emitted('dirty')?.at(-1)).toEqual([false]);
  });

  it('reverts the draft and stages defaults without persisting them', async () => {
    applyAppearance({ ...defaultAppearance, theme: 'dark', accent: 'purple', textSize: 18, scale: 125 });
    const wrapper = editor();
    await wrapper.get('.appearance-reset').trigger('click');
    expect(readAppearance().accent).toBe('purple');
    document.querySelector<HTMLButtonElement>('.confirmation-actions .primary-button')!.click(); await flushPromises();
    expect(readAppearance()).toMatchObject({ theme: 'dark', accent: 'purple', textSize: 18, scale: 125 });
    expect(wrapper.get('.appearance-preview-canvas').attributes('data-accent')).toBe('green');
    expect((wrapper.get('#appearance-scale').element as HTMLInputElement).value).toBe('100');
    wrapper.vm.cancel(); await flushPromises();
    expect(wrapper.get('.appearance-preview-canvas').attributes('data-accent')).toBe('purple');
    expect((wrapper.get('#appearance-scale').element as HTMLInputElement).value).toBe('125');
    expect(wrapper.find('.appearance-save').exists()).toBe(false);
    await wrapper.get('.appearance-reset').trigger('click');
    expect(readAppearance().accent).toBe('purple');
    document.querySelector<HTMLButtonElement>('.confirmation-actions .primary-button')!.click(); await flushPromises();
    wrapper.vm.save(); await flushPromises();
    expect(readAppearance()).toEqual(defaultAppearance);
  });

  it('reopens saved choices and lets keyboard changes update only the preview', async () => {
    const wrapper = editor();
    const slider = wrapper.get('[role="slider"][aria-label="Interface size"]');
    await slider.trigger('keydown', { key: 'ArrowRight' });
    expect(slider.attributes('aria-valuenow')).toBe('105');
    expect(readAppearance().scale).toBe(100);
    wrapper.vm.save(); await flushPromises();
    const reopened = editor();
    expect((reopened.get('#appearance-scale').element as HTMLInputElement).value).toBe('105');
    expect(reopened.find('.appearance-save').exists()).toBe(false);
  });
});

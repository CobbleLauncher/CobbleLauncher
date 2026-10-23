// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import VersionSelector from '../VersionSelector.vue';
import { i18n, locale, localePreference, resolveLocale, setLocale, t } from './index';
import { messages } from './messages';
const english = structuredClone(messages['en-US']);
afterEach(() => { vi.restoreAllMocks(); i18n.global.setLocaleMessage('en-US', structuredClone(english)); setLocale('zh-CN'); localStorage.removeItem('cobble-language'); });
describe('feature localization', () => {
  it('switches rendered component strings immediately and persists the language', async () => {
    const wrapper = mount(VersionSelector, { props: { id: 'version', modelValue: '', options: [] } });
    expect(wrapper.get('input').attributes('placeholder')).toBe('搜索版本…');
    setLocale('en-US'); await nextTick();
    expect(wrapper.get('input').attributes('placeholder')).toBe('Search versions…');
    expect(document.documentElement.lang).toBe('en-US');
    expect(localStorage.getItem('cobble-language')).toBe('en-US');
    wrapper.unmount();
  });
  it('interpolates values and falls back to Chinese for untranslated keys', () => {
    setLocale('en-US');
    expect(t('market.installed', { name: 'Forest' })).toBe('Installed to Forest');
    const partial = structuredClone(english);
    Reflect.deleteProperty(partial.versions, 'more');
    i18n.global.setLocaleMessage('en-US', partial);
    expect(t('versions.more')).toBe('加载更多');
    expect(t('blueprints.loading', { loaded: 2, total: 5 })).toBe('Loading 2 / 5');
  });
  it('uses supported system languages with a Chinese fallback and ignores invalid preferences', () => {
    expect(resolveLocale('system', 'en-GB')).toBe('en-US');
    expect(resolveLocale('system', 'zh-TW')).toBe('zh-CN');
    expect(resolveLocale('system', 'fr-FR')).toBe('zh-CN');
    setLocale('zh-CN'); setLocale('untrusted-value');
    expect(locale.value).toBe('zh-CN');
    expect(localePreference.value).toBe('zh-CN');
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('denied'); });
    expect(() => setLocale('en-US')).not.toThrow();
    expect(locale.value).toBe('en-US');
  });
  it('keeps matching feature keys across bundled languages', () => {
    for (const feature of Object.keys(messages['zh-CN']) as (keyof typeof messages['zh-CN'])[]) {
      expect(Object.keys(messages['en-US'][feature]).sort()).toEqual(Object.keys(messages['zh-CN'][feature]).sort());
    }
  });
});

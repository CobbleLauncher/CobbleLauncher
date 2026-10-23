// SPDX-License-Identifier: MIT
import { ref } from 'vue';
import { createI18n } from 'vue-i18n';
import { messages, type MessageSchema } from './messages';
export type { TranslationKey } from './messages';
export const supportedLocales = [{ value: 'zh-CN', label: '简体中文' }, { value: 'en-US', label: 'English' }] as const;
export type SupportedLocale = typeof supportedLocales[number]['value'];
export type LocalePreference = SupportedLocale | 'system';
const storageKey = 'cobble-language';
export function resolveLocale(value: string, systemLanguage = typeof navigator === 'undefined' ? '' : navigator.language): SupportedLocale {
  if (value === 'en-US' || value === 'zh-CN') return value;
  if (value === 'system' && systemLanguage.toLowerCase().startsWith('en')) return 'en-US';
  return 'zh-CN';
}
function initialPreference(): LocalePreference {
  try { const value = localStorage.getItem(storageKey); if (value === 'en-US' || value === 'zh-CN' || value === 'system') return value; } catch { /* Use local defaults when storage is unavailable. */ }
  return 'zh-CN';
}
export const localePreference = ref<LocalePreference>(initialPreference());
export const i18n = createI18n<[MessageSchema], SupportedLocale, false>({
  legacy: false,
  locale: resolveLocale(localePreference.value),
  fallbackLocale: 'zh-CN',
  messages,
  missingWarn: false,
  fallbackWarn: false,
});
export const locale = i18n.global.locale;
export const t = i18n.global.t;
export function setLocale(value: string) {
  if (value !== 'zh-CN' && value !== 'en-US' && value !== 'system') return;
  localePreference.value = value;
  locale.value = resolveLocale(value);
  if (typeof document !== 'undefined') document.documentElement.lang = locale.value;
  try { localStorage.setItem(storageKey, value); } catch { /* Language still applies for this session. */ }
}
if (typeof document !== 'undefined') document.documentElement.lang = locale.value;

// SPDX-License-Identifier: MIT
import type { TranslationKey } from '../i18n';
export type SettingsCategory = { id: string; label: string; labelKey?: TranslationKey };
export const builtinSettingsCategories: readonly SettingsCategory[] = [
  { id: 'general', label: '', labelKey: 'settingsLayout.general' },
  { id: 'downloads', label: '', labelKey: 'settings.downloads' },
  { id: 'runtime', label: '', labelKey: 'settings.runtime' },
  { id: 'accounts', label: '', labelKey: 'settings.accounts' },
  { id: 'about', label: '', labelKey: 'settings.about' },
];
/** Trusted application contributions; plugin renderer UI is not a settings API. */
export function settingsCategories(contributions: readonly SettingsCategory[] = []): SettingsCategory[] {
  const result = [...builtinSettingsCategories];
  for (const category of contributions) if (!result.some(item => item.id === category.id)) result.push(category);
  return result;
}

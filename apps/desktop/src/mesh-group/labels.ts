// SPDX-License-Identifier: MIT
import { locale } from '../i18n';
import en from '../i18n/locales/en-US/mesh-group.json';
import zh from '../i18n/locales/zh-CN/mesh-group.json';
export type GroupTextKey = keyof typeof en;
export function groupText(key: GroupTextKey, values: Record<string, string | number> = {}): string {
  const text = (locale.value === 'zh-CN' ? zh : en)[key];
  return text.replace(/\{([a-zA-Z]+)\}/g, (token, name: string) => values[name] === undefined ? token : String(values[name]));
}

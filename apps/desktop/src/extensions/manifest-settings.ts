// SPDX-License-Identifier: MIT
import type { ExtensionSettingsForm } from './forms/settings-form';
export type ManifestSettingLabel = { en: string; 'zh-CN'?: string };
export type ManifestSettingField = { id?: string; label?: ManifestSettingLabel; scope: 'global' | 'instance' };
export type ManifestSettings = {
  categories?: { id: string; label?: ManifestSettingLabel; fields: ManifestSettingField[] }[];
  form?: ExtensionSettingsForm;
};
/** Presentation metadata only. Native schema/permission validation remains authoritative. */
export function manifestSettingFields(settings?: ManifestSettings | null): ManifestSettingField[] {
  if (!settings) return [];
  if (!settings.form) return (settings.categories ?? []).flatMap(category => category.fields);
  const { schema, scope, translations } = settings.form;
  return Object.entries(schema.properties ?? {}).map(([id, property]) => {
    const title = typeof property === 'object' && property !== null && typeof property.title === 'string' ? property.title : undefined;
    const en = translations?.en?.[`${id}.label`] ?? title;
    const zh = translations?.['zh-CN']?.[`${id}.label`];
    return { id, scope, ...(en ? { label: { en, ...(zh ? { 'zh-CN': zh } : {}) } } : zh ? { label: { en: zh, 'zh-CN': zh } } : {}) };
  });
}

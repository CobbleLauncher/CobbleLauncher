// SPDX-License-Identifier: MIT
import type { ExtensionSettingsForm } from './forms/settings-form';
export type ExtensionJsonValue = null | boolean | number | string;
export type ExtensionSettingType = 'boolean' | 'enum' | 'string' | 'number';
export type ExtensionSettingScope = 'global' | 'instance';

export type ExtensionSettingOption = {
  value: ExtensionJsonValue;
  label: string;
};

export type ExtensionSettingField = {
  id: string;
  type: ExtensionSettingType;
  label: string;
  scope: ExtensionSettingScope;
  advanced: boolean;
  default: ExtensionJsonValue;
  options: ExtensionSettingOption[];
  minimum?: number;
  maximum?: number;
  maxLength?: number;
};

export type ExtensionSettingCategory = {
  id: string;
  label: string;
  fields: ExtensionSettingField[];
};

export type ExtensionSettingsSnapshot = {
  pluginId: string;
  instanceId?: string;
  values: Record<string, ExtensionJsonValue>;
  categories: ExtensionSettingCategory[];
  form?: ExtensionSettingsForm;
};

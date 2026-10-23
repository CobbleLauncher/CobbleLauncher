// SPDX-License-Identifier: MIT
import type { JsonSchema, UISchemaElement } from '@jsonforms/core';
import type { ErrorObject } from 'ajv';
import { t } from '../../i18n';
export type ExtensionSettingsForm = {
  schema_version: 1;
  scope: 'global' | 'instance';
  schema: JsonSchema;
  ui_schema?: UISchemaElement;
  translations?: Record<string, Record<string, string>>;
};
export type SettingsFormError = ErrorObject;

export function localizeFormErrors(errors: SettingsFormError[]): SettingsFormError[] {
  return errors.map(error => {
    let message: string;
    switch (error.keyword) {
      case 'enum': message = t('extensionSettings.formEnumInvalid'); break;
      case 'required': message = t('extensionSettings.formRequired'); break;
      case 'additionalProperties': message = t('extensionSettings.formUnknown'); break;
      case 'minimum':
      case 'maximum': message = t('extensionSettings.rangeInvalid'); break;
      case 'minLength':
      case 'maxLength': message = t('extensionSettings.lengthInvalid'); break;
      default: message = t('extensionSettings.formTypeInvalid');
    }
    return { ...error, message };
  });
}

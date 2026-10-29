// SPDX-License-Identifier: MIT
import { t } from './i18n';
export function profileNameError(value: string): string | null {
  if (!value) return t('profiles.nameRequired');
  if (!/^[A-Za-z0-9_]+$/.test(value)) return t('profiles.nameCharacters');
  if (value.length < 3 || value.length > 16) return t('profiles.nameLength');
  return null;
}

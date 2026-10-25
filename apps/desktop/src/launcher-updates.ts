// SPDX-License-Identifier: MIT
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { TranslationKey } from './i18n';
const errorCodes = ['UPDATE_PREFERENCES_INVALID', 'UPDATE_PREFERENCES_READ_FAILED', 'UPDATE_STATE_UNAVAILABLE', 'UPDATE_BUSY', 'UPDATE_CHANNEL_INVALID', 'UPDATE_PREFERENCES_WRITE_FAILED', 'UPDATE_UNCONFIGURED', 'UPDATE_CONFIGURATION_INVALID', 'UPDATE_CHECK_FAILED', 'UPDATE_PORTABLE', 'UPDATE_CHECK_REQUIRED', 'UPDATE_DOWNLOAD_FAILED', 'UPDATE_INSTALL_FAILED'] as const;
export function launcherUpdateErrorKey(error: unknown): TranslationKey {
  const code = errorCodes.find((code) => String(error) === code) ?? 'UPDATE_UNKNOWN';
  return `launcherUpdates.errors.${code}`;
}
export interface LauncherUpdateStatus {
  version: string;
  background_check: boolean;
  channel: 'stable' | 'preview';
  configured: boolean;
  can_install: boolean;
  available_version: string | null;
}
export const launcherUpdateStatus = () => invoke<LauncherUpdateStatus>('launcher_update_status');
export const checkLauncherUpdate = (automatic = false) => invoke<LauncherUpdateStatus>('check_launcher_update', { automatic });
export const saveLauncherUpdatePreferences = (backgroundCheck: boolean, channel: string) => invoke<LauncherUpdateStatus>('set_launcher_update_preferences', { backgroundCheck, channel });
export const installLauncherUpdate = () => invoke<void>('install_launcher_update');
/** Call after the home screen has rendered. The host also enforces opt-in. */
export async function checkLauncherUpdateAfterHome(): Promise<LauncherUpdateStatus | null> {
  if (!isTauri()) return null;
  const status = await launcherUpdateStatus();
  if (!status.background_check || !status.configured) return null;
  return checkLauncherUpdate(true);
}

// SPDX-License-Identifier: MIT
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { AppInfo } from '../../../contracts/bootstrap';

export type Connection =
  | { kind: 'preview' }
  | { kind: 'connected'; info: AppInfo }
  | { kind: 'unavailable' };

/** A missing host or a rejected invoke must never turn into mock success. */
export async function connectDesktop(): Promise<Connection> {
  if (!isTauri()) return { kind: 'preview' };
  try {
    const info = await invoke<AppInfo>('get_app_info');
    if (
      !info || typeof info.name !== 'string' || typeof info.version !== 'string' ||
      info.stage !== 'development' ||
      Object.keys(info).some((key) => !['name', 'version', 'stage'].includes(key))
    ) return { kind: 'unavailable' };
    return { kind: 'connected', info };
  } catch {
    return { kind: 'unavailable' };
  }
}

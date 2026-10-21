// SPDX-License-Identifier: MIT
import { invoke, isTauri } from '@tauri-apps/api/core';

export type DownloadSettings = { source: 'official' | 'bmclapi' | 'custom'; mirror: string };
const previewKey = 'cobble.preview.download-source';
export const defaultDownloadSettings = (): DownloadSettings => ({ source: 'official', mirror: '' });

export function validateDownloadSettings(settings: DownloadSettings): string | null {
  if (!['official', 'bmclapi', 'custom'].includes(settings.source) || typeof settings.mirror !== 'string') return '下载源设置无效';
  if (settings.source !== 'custom') return null;
  try {
    const url = new URL(settings.mirror.trim());
    if (url.protocol === 'https:' && url.hostname && !url.username && !url.password && !url.search && !url.hash && !settings.mirror.includes('?') && !settings.mirror.includes('#')) return null;
  } catch { /* Show the same safe, actionable validation message. */ }
  return '请输入 HTTPS 镜像地址，不含账号、密码、查询参数或片段';
}

/** Browser previews keep a separate preference; only the native host shares it with CLI. */
export async function readDownloadSettings(): Promise<DownloadSettings> {
  const native = isTauri();
  const stored = native
    ? await invoke<DownloadSettings>('get_download_settings')
    : JSON.parse(localStorage.getItem(previewKey) ?? 'null') as DownloadSettings | null;
  if (!stored && !native) return defaultDownloadSettings();
  if (!stored) throw new Error('Invalid download settings');
  if (validateDownloadSettings(stored)) throw new Error('Invalid download settings');
  return stored;
}
export async function saveDownloadSettings(settings: DownloadSettings): Promise<void> {
  const error = validateDownloadSettings(settings);
  if (error) throw new Error(error);
  const normalized = { source: settings.source, mirror: settings.mirror.trim() };
  if (isTauri()) await invoke('set_download_settings', { settings: normalized });
  else localStorage.setItem(previewKey, JSON.stringify(normalized));
}

// SPDX-License-Identifier: MIT
import { invoke } from '@tauri-apps/api/core';
const cache = new Map<string, { value: string; bytes: number; time: number }>();
const pending = new Map<string, Promise<string | null>>();
let bytes = 0;
export function marketImage(provider: 'modrinth' | 'curseforge', url: string, variant: 'icon' | 'preview'): Promise<string | null> {
  const key = `${provider}:${variant}:${url}`;
  const hit = cache.get(key);
  if (hit) {
    cache.delete(key); cache.set(key, hit);
    if (Date.now() - hit.time >= 300_000) void refreshImage(key, provider, url, variant);
    return Promise.resolve(hit.value);
  }
  return refreshImage(key, provider, url, variant);
}
function refreshImage(key: string, provider: 'modrinth' | 'curseforge', url: string, variant: 'icon' | 'preview'): Promise<string | null> {
  const active = pending.get(key); if (active) return active;
  const request = invoke<string>('get_market_image', { provider, url, variant }).then(value => {
    if (!value.startsWith('data:image/png;base64,')) return null;
    const size = value.length * 2;
    bytes -= cache.get(key)?.bytes ?? 0;
    cache.delete(key);
    cache.set(key, { value, bytes: size, time: Date.now() }); bytes += size;
    while (bytes > 16 * 1024 * 1024 && cache.size) {
      const first = cache.keys().next().value!; bytes -= cache.get(first)!.bytes; cache.delete(first);
    }
    return value;
  }).catch(() => null).finally(() => pending.delete(key));
  pending.set(key, request); return request;
}

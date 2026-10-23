// SPDX-License-Identifier: MIT
import { reactive } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
export const avatars = reactive(new Map<string, string>());
const requested = new Set<string>();
export async function loadAvatar(id: string) {
  if (!isTauri() || requested.has(id)) return;
  requested.add(id);
  try { const appearance = await invoke<{ skin: { avatar: string } | null }>('get_profile_appearance', { id, refresh: false }); if (appearance?.skin) avatars.set(id, appearance.skin.avatar); }
  catch { requested.delete(id); }
}

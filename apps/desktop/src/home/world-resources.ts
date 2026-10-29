// SPDX-License-Identifier: MIT
import { invoke } from '@tauri-apps/api/core';
import type { LocalWorld } from '../games';

export type HomeWorldResources = { thumbnail: string | null; last_backup: string | null; backup_unavailable?: boolean };

/** Small session-local cache. A changed last-played stamp invalidates the world's icon. */
export function homeWorldResources() {
  const summaries = new Map<string, { at: number; value: Promise<HomeWorldResources> }>();
  return async (id: string, world: LocalWorld): Promise<HomeWorldResources> => {
    const key = JSON.stringify([id, world.folder_name, world.last_played]);
    const cached = summaries.get(key);
    if (cached && Date.now() - cached.at < 15_000) return cached.value;
    const value = invoke<HomeWorldResources>('get_home_world_summary', { id, folderName: world.folder_name })
      .then(summary => ({ ...summary, thumbnail: summary.thumbnail?.startsWith('data:image/png;base64,') ? summary.thumbnail : null }))
      .catch(error => { if (summaries.get(key)?.value === value) summaries.delete(key); throw error; });
    summaries.set(key, { at: Date.now(), value });
    if (summaries.size > 6) summaries.delete(summaries.keys().next().value!);
    return value;
  };
}

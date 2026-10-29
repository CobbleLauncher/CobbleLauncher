// SPDX-License-Identifier: MIT
import { reactive } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ManagedGameMod } from './games';

export type MarketInstallResult = {
  gameId: string;
  kind: 'mod' | 'resource_pack' | 'shader_pack';
  provider: 'modrinth' | 'curseforge';
  projectIds: string[];
  versionIds: string[];
  /** Canonical local filenames confirmed by native records or result snapshots. */
  contentIds: string[];
};

const pendingGames = reactive(new Set<string>());
export function marketInstallPending(gameId: string) { return pendingGames.has(gameId); }
export function beginMarketInstall(gameId: string) {
  if (pendingGames.has(gameId)) return false;
  pendingGames.add(gameId);
  return true;
}
export function endMarketInstall(gameId: string) { pendingGames.delete(gameId); }

export async function modInstallResult(gameId: string, provider: MarketInstallResult['provider'], mods: {project_id: string; version_id: string}[]): Promise<MarketInstallResult> {
  let records: ManagedGameMod[] = [];
  try {
    const value = await invoke<ManagedGameMod[]>(provider === 'modrinth' ? 'list_managed_game_mods' : 'list_game_curseforge_mods', { id: gameId });
    if (Array.isArray(value)) records = value;
  } catch { /* A successful install remains successful when result lookup is unavailable. */ }
  return {
    gameId, kind: 'mod', provider,
    projectIds: mods.map(mod => mod.project_id),
    versionIds: mods.map(mod => mod.version_id),
    contentIds: records.filter(record => mods.some(mod => mod.project_id === record.project_id && mod.version_id === record.version_id)).map(record => record.filename.replace(/\.disabled$/i, '')),
  };
}

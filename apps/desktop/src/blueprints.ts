// SPDX-License-Identifier: MIT
import { shallowRef } from 'vue';
import type { Game } from './games';
export type BlueprintFormat = 'schem' | 'litematic';
export type BlueprintSource = { provider: string; id: string; version: number; author: string; attribution: string; license: string | null };
export type SavedBlueprint = { id: string; name: string; pluginId: string; format?: BlueprintFormat; favorite?: boolean; hasThumbnail?: boolean; source?: BlueprintSource };
export type BlueprintImportResult = { destination: string };
export function canImportBlueprint(game: Game): boolean {
  return game.status === 'installed' && !game.content_installing && (!game.launch_state || game.launch_state === 'idle');
}
export type SceneBlock = { x: number; y: number; z: number; palette_index: number };
export type ViewerSummary = { width: number; height: number; length: number; palette: string[]; materials: { state: string; count: number }[]; chunk_count: number };
export type OpenedBlueprint = { resourceId: string; name: string; format?: BlueprintFormat; scene: ViewerSummary };
export const incomingBlueprint = shallowRef<OpenedBlueprint | null>(null);

export type PendingBlueprintRequest = { queueId: string; name: string; format: BlueprintFormat };
export const pendingBlueprint = shallowRef<PendingBlueprintRequest | null>(null);

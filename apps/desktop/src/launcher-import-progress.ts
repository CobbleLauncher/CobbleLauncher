// SPDX-License-Identifier: MIT
import type { Game } from './games';
import type { TaskSnapshot } from './tasks';
import { taskHasKnownTotal } from './tasks';
export type ImportSceneState = 'working' | 'waiting' | 'failed' | 'complete' | 'cancelled';
export function importSceneState(game: Game | null, task?: TaskSnapshot): ImportSceneState {
  if (game?.status === 'installed') return 'complete';
  if (task?.state === 'cancelled' || task?.state === 'interrupted') return 'cancelled';
  if (game?.status === 'failed' || task?.state === 'failed') return 'failed';
  if (task?.state === 'queued' || task?.state === 'cancelling' || !game) return 'waiting';
  return 'working';
}
export function importPhaseRatio(progress: TaskSnapshot['progress'] | Game['progress']): number | null {
  if (!progress || !taskHasKnownTotal(progress)) return null;
  return Math.max(0, Math.min(1, progress.completed / progress.total));
}

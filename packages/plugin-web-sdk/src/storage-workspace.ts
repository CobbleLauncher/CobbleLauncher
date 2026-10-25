// SPDX-License-Identifier: MIT
import type { Transport } from './index';
export type StorageGame = { gameId: string; name: string; instanceBytes: number | null; backupBytes: number | null; activity: string; movable: boolean };
export type StorageOverview = { games: StorageGame[]; retained: {taskId: string; gameId: string; bytes: number; sourceLabel: string; canRecycle: boolean}[]; cache: { bytes: number; reclaimableBytes: number } };
export type MovePreview = { planId: string; targetLabel: string; requiredBytes: number; availableBytes: number | null; enoughSpace: boolean | null; sourceRetained: true };
export type MoveTask = { taskId: string; status: 'running' | 'succeeded' | 'failed' | 'cancelled'; phase: 'copying' | 'publishing' | 'complete'; completedBytes: number; totalBytes: number; error: string | null; sourceRetained: boolean };
function id(value: string): string {
  if (typeof value !== 'string' || !/^[a-zA-Z0-9_-]{1,128}$/.test(value)) throw Error('STORAGE_INVALID_ID');
  return value;
}
export function createStorageWorkspace(transport: Transport) {
  return Object.freeze({
    overview: (args: {gameId?: string} = {}) => transport({method: 'storageWorkspace.overview', params: args.gameId === undefined ? {} : {gameId: id(args.gameId)}}) as Promise<StorageOverview>,
    pickDestination: (args: {gameId: string}) => transport({method: 'storageWorkspace.pickDestination', params: {gameId: id(args.gameId)}}) as Promise<MovePreview | null>,
    cleanCache: () => transport({method: 'storageWorkspace.cleanCache', params: {}}) as Promise<{cleared: boolean; reclaimedBytes: number}>,
    startMove: (args: {planId: string}) => transport({method: 'storageWorkspace.startMove', params: {planId: id(args.planId)}}) as Promise<{taskId: string | null}>,
    task: (args: {taskId: string}) => transport({method: 'storageWorkspace.task', params: {taskId: id(args.taskId)}}) as Promise<MoveTask>,
    cancel: (args: {taskId: string}) => transport({method: 'storageWorkspace.cancel', params: {taskId: id(args.taskId)}}) as Promise<null>,
    openSource: (args: {taskId: string}) => transport({method: 'storageWorkspace.openSource', params: {taskId: id(args.taskId)}}) as Promise<null>,
    releaseSource: (args: {taskId: string}) => transport({method: 'storageWorkspace.releaseSource', params: {taskId: id(args.taskId)}}) as Promise<{released: boolean; recycledBytes: number}>,
  });
}

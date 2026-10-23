// SPDX-License-Identifier: MIT
import { t } from './i18n';
export type DownloadProgressDetail = { bytesCompleted: number; bytesTotal: number; bytesReceived: number; bytesPerSecond: number; activeDownloads: number; items: { source: string; item: string; bytesReceived: number; bytesTotal: number }[] };
export type TaskProgress = { phase: string; completed: number; total: number; reused: number; detail?: DownloadProgressDetail | null };
export type TaskSnapshot = {
  id: string;
  game_id: string | null;
  world_folder?: string | null;
  game_name: string;
  title: string;
  kind: 'game_install' | 'game_repair' | 'mod_install' | 'mod_update' | 'resource_pack' | 'shader_pack' | 'modpack_download' | 'curseforge_pack_download' | 'world_restore' | 'world_automatic_backup' | 'world_snapshot' | 'world_snapshot_restore';
  state: 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted';
  progress: TaskProgress | null;
  error: string | null;
  can_retry: boolean;
  created_at: number;
  updated_at: number;
  has_result: boolean;
};
export function isActiveTask(task: TaskSnapshot): boolean { return task.state === 'running' || task.state === 'cancelling'; }
export function taskStateLabel(task: TaskSnapshot): string {
  return t(`tasks.states.${task.state}`);
}
export function taskPhaseLabel(progress: TaskProgress): string {
  if (['world-snapshot-scan', 'world-snapshot-write', 'world-snapshot-verify', 'world-snapshot-restore', 'world-snapshot-publish', 'world-snapshot-complete', 'world-extract', 'world-copy', 'world-commit', 'world-complete'].includes(progress.phase)) return t(`tasks.phases.${progress.phase}`);
  const phase = progress.phase === 'pack-base-complete' ? progress.phase : progress.phase.replace(/^pack-base-/, '');
  const known = ['metadata', 'files', 'assets', 'fabric', 'forge-installer', 'neoforge-installer', 'pack-download', 'pack-overrides', 'pack-install', 'mod-download', 'mod-install', 'import-copy', 'complete', 'pack-base-complete'];
  return t(`tasks.phases.${known.includes(phase) ? phase : 'processing'}`);
}
export function taskHasKnownTotal(progress: TaskProgress): boolean {
  const phase = progress.phase.replace(/^pack-base-/, '');
  return progress.total > 0 && phase !== 'forge-installer' && phase !== 'neoforge-installer';
}

export function taskTitleLabel(task: TaskSnapshot): string {
  return task.kind === 'game_repair' ? t('tasks.repairGame') : task.kind === 'world_automatic_backup' ? t('tasks.automaticBackup') : task.kind === 'world_snapshot' ? t('tasks.snapshotSave') : task.kind === 'world_snapshot_restore' ? t('tasks.snapshotRestore') : task.title;
}
export function isWorldSnapshotTask(task: TaskSnapshot): boolean { return task.kind === 'world_snapshot' || task.kind === 'world_snapshot_restore' || task.kind === 'world_automatic_backup'; }

export function taskProgressUsesBytes(progress: TaskProgress): boolean { return progress.phase.startsWith('world-snapshot-'); }

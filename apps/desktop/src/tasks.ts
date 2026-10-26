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
  kind: 'change_share' | 'game_transfer' | 'directory_migration' | 'game_copy' | 'game_readiness' | 'game_install' | 'game_repair' | 'game_java' | 'mod_install' | 'mod_update' | 'resource_pack' | 'shader_pack' | 'modpack_download' | 'curseforge_pack_download' | 'world_restore' | 'world_automatic_backup' | 'world_snapshot' | 'world_snapshot_restore' | 'trial_copy' | 'storage_move' | 'pack_update_copy';
  state: 'queued' | 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted';
  progress: TaskProgress | null;
  error: string | null;
  can_retry: boolean;
  created_at: number;
  updated_at: number;
  has_result: boolean;
};
export function isActiveTask(task: TaskSnapshot): boolean { return task.state === 'queued' || task.state === 'running' || task.state === 'cancelling'; }
export function taskStateLabel(task: TaskSnapshot): string {
  return t(`tasks.states.${task.state}`);
}
export function taskPhaseLabel(progress: TaskProgress): string {
  if (['receiving', 'verifying', 'preparing'].includes(progress.phase)) return t(`tasks.phases.${progress.phase}`);
  if (['world-snapshot-scan', 'world-snapshot-write', 'world-snapshot-verify', 'world-snapshot-restore', 'world-snapshot-publish', 'world-snapshot-complete', 'world-extract', 'world-copy', 'world-commit', 'world-complete', 'java-metadata', 'java-download', 'java-install', 'java-verify'].includes(progress.phase)) return t(`tasks.phases.${progress.phase}`);
  const phase = progress.phase === 'pack-base-complete' ? progress.phase : progress.phase.replace(/^pack-base-/, '');
  const known = ['metadata', 'files', 'assets', 'fabric', 'forge-installer', 'neoforge-installer', 'pack-download', 'pack-overrides', 'pack-install', 'mod-download', 'mod-install', 'import-copy', 'copying', 'personal-options', 'complete', 'pack-base-complete'];
  return t(`tasks.phases.${known.includes(phase) ? phase : 'processing'}`);
}
export function taskHasKnownTotal(progress: TaskProgress): boolean {
  const phase = progress.phase.replace(/^pack-base-/, '');
  return progress.total > 0 && phase !== 'forge-installer' && phase !== 'neoforge-installer';
}

export function taskTitleLabel(task: TaskSnapshot): string {
  if (task.kind === 'change_share') return t('tasks.sharedChanges');
  if (task.kind === 'game_transfer') return t('tasks.gameTransfer');
  if (task.kind === 'directory_migration') return t('tasks.storageMove');
  if (task.kind === 'game_copy') return t('tasks.gameCopy');
  return task.kind === 'game_readiness' ? t('readiness.title') : task.kind === 'game_java' ? t('tasks.javaRuntime') : task.kind === 'game_repair' ? t('tasks.repairGame') : task.kind === 'world_automatic_backup' ? t('tasks.automaticBackup') : task.kind === 'world_snapshot' ? t('tasks.snapshotSave') : task.kind === 'world_snapshot_restore' ? t('tasks.snapshotRestore') : task.kind === 'trial_copy' ? t('tasks.trialCopy') : task.kind === 'storage_move' ? t('tasks.storageMove') : task.kind === 'pack_update_copy' ? t('tasks.packUpdateCopy') : task.title;
}
export function taskFailureMessage(task: TaskSnapshot): string {
  if (task.error && /SPACE_INSUFFICIENT|GAME_COPY_NO_SPACE|DIRECTORY_MIGRATION_NO_SPACE/u.test(task.error)) return t('tasks.copyErrors.space');
  if (task.error?.includes('PERSONAL_OPTIONS_')) {
    if (task.error.includes('PERSONAL_OPTIONS_CONFLICT')) return t('tasks.personalOptionsConflict');
    if (/PERSONAL_OPTIONS_PLAN_CHANGED|PERSONAL_OPTIONS_TARGET_CHANGED|PERSONAL_OPTIONS_RECEIPT_INVALID/u.test(task.error)) return t('tasks.personalOptionsChanged');
    return t('tasks.personalOptionsFailed');
  }
  if (task.kind === 'mod_update' && task.error?.includes('请选择并验证作者要求手动下载的文件')) return t('tasks.updateManualFiles');
  if ((task.kind === 'game_copy' || task.kind === 'directory_migration') && task.error) {
    if (/GAME_COPY_SOURCE_CHANGED|GAME_COPY_LIBRARY_CHANGED|DIRECTORY_MIGRATION_CHANGED|DIRECTORY_MIGRATION_SOURCE_IN_USE/u.test(task.error)) return t('tasks.copyErrors.changed');
    if (/GAME_COPY_TARGET_INVALID|GAME_COPY_TARGET_UNOWNED|DIRECTORY_MIGRATION_TARGET|DIRECTORY_MIGRATION_EXISTS/u.test(task.error)) return t('tasks.copyErrors.target');
    if (/GAME_COPY_LINK_DENIED|DIRECTORY_MIGRATION_LINK/u.test(task.error)) return t('tasks.copyErrors.link');
  }
  return t(task.can_retry ? 'tasks.failureRetry' : 'tasks.failureReview');
}
export function isWorldSnapshotTask(task: TaskSnapshot): boolean { return task.kind === 'world_snapshot' || task.kind === 'world_snapshot_restore' || task.kind === 'world_automatic_backup'; }

export function taskProgressUsesBytes(progress: TaskProgress): boolean { return progress.phase.startsWith('world-snapshot-') || progress.phase === 'java-download' || progress.phase === 'copying' || progress.phase === 'receiving'; }

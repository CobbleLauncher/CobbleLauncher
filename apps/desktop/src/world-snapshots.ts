// SPDX-License-Identifier: MIT
export type SnapshotRestoreTarget = { worldId: string; snapshotId: string };
/** Stored Jiff timestamps may append a zone annotation to an ISO offset. */
export function snapshotTimestamp(value: string): number {
  return Date.parse(value.replace(/\[[^\]\r\n]+\]$/, ''));
}
export type PluginSnapshotRestoreRequest = {
  game_id: string; world_id: string; snapshot_id: string; plugin_id: string;
};
export type WorldSnapshot = {
  id: string;
  world_id: string;
  name: string;
  created_at: string;
  processed_bytes: number;
  added_bytes: number;
  protected?: boolean;
  automatic?: boolean;
};
export type SnapshotRetentionPolicy = { keep_last: number; keep_named: boolean };
export type SnapshotCleanupPreview = {
  previewId: string; policy: SnapshotRetentionPolicy; remove: WorldSnapshot[];
  keep_count: number; named_kept: number; estimated_reclaim_bytes: number; repack_bytes: number;
};
export type SnapshotCleanupResult = {
  removed_count: number; reclaimed_bytes: number; cleanup_complete: boolean; remaining: WorldSnapshot[];
};

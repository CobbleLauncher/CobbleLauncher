// SPDX-License-Identifier: MIT
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

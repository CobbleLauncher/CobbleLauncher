// SPDX-License-Identifier: MIT
import type { LocalWorld } from './games';
export type WorldBackupPreview = { preview_id: string; name: string; minecraft_version: string | null; file_count: number; total_bytes: number; suggested_folder: string; metadata_error?: boolean };
export type WorldBackup = { backup_id: string; folder_name: string; world_name: string; created_at: number };
export type WorldRestoreResult = { worlds: LocalWorld[]; rollback_id: string | null };

// SPDX-License-Identifier: MIT
export type GameVersion = { id: string; kind: string };
export type FabricVersion = { version: string; stable: boolean };
export type LocalMod = { file_name: string; name: string; version: string | null; authors: string[]; loader: string | null; enabled: boolean; description: string | null; pack_owned?: boolean };
export type Game = {
  id: string;
  name: string;
  version: string;
  loader_version?: string | null;
  pack_name?: string | null;
  pack_version?: string | null;
  directory: string;
  status: 'installing' | 'installed' | 'failed';
  error: string | null;
  progress: { phase: 'metadata' | 'files' | 'assets' | 'fabric' | 'pack-base-metadata' | 'pack-base-files' | 'pack-base-assets' | 'pack-base-fabric' | 'pack-base-complete' | 'pack-download' | 'pack-overrides' | 'pack-install' | 'complete'; completed: number; total: number; reused: number } | null;
  content_installing?: boolean;
  launch_state?: 'idle' | 'preparing' | 'running';
  launch_error?: string | null;
  log_path?: string | null;
};
export type PackPreview = { preview_id: string; name: string; version_id: string; summary: string | null; minecraft_version: string; loader_version: string | null; required_files: number; optional_files: { path: string; size: number }[]; overrides: number };
export type OfflineProfile = { name: string; uuid: string };
export type ModSearchHit = { project_id: string; slug: string; title: string; description: string; author: string; downloads: number };
export type ModSearchPage = { hits: ModSearchHit[]; offset: number; total_hits: number };
export type ModInstallPreview = { plan_id: string; project_id: string; version_id: string; mods: { project_id: string; version_id: string; name: string; version_number: string; required_by: string | null }[] };

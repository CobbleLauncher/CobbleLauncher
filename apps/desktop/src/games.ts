// SPDX-License-Identifier: MIT
export type GameVersion = { id: string; kind: string };
export type FabricVersion = { version: string; stable: boolean };
export type LocalMod = { file_name: string; name: string; version: string | null; authors: string[]; loader: string | null; enabled: boolean; description: string | null };
export type Game = {
  id: string;
  name: string;
  version: string;
  loader_version?: string | null;
  directory: string;
  status: 'installing' | 'installed' | 'failed';
  error: string | null;
  progress: { phase: 'metadata' | 'files' | 'assets' | 'fabric' | 'complete'; completed: number; total: number; reused: number } | null;
  launch_state?: 'idle' | 'preparing' | 'running';
  launch_error?: string | null;
  log_path?: string | null;
};
export type OfflineProfile = { name: string; uuid: string };

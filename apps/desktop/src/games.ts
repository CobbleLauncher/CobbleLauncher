// SPDX-License-Identifier: MIT
export type GameVersion = { id: string; kind: string };
export type Game = {
  id: string;
  name: string;
  version: string;
  directory: string;
  status: 'installing' | 'installed' | 'failed';
  error: string | null;
  progress: { phase: 'metadata' | 'files' | 'assets' | 'complete'; completed: number; total: number; reused: number } | null;
  launch_state?: 'idle' | 'preparing' | 'running';
  launch_error?: string | null;
  log_path?: string | null;
};
export type OfflineProfile = { name: string; uuid: string };

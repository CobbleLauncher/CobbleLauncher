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
};

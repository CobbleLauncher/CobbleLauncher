// SPDX-License-Identifier: MIT
export type GameVersion = { id: string; kind: string };
export type FabricVersion = { version: string; stable: boolean };
export type LoaderKind = 'fabric' | 'forge' | 'neoforge';
export type LoaderVersion = { version: string; recommended: boolean };
export type LocalMod = { file_name: string; name: string; version: string | null; authors: string[]; loader: string | null; enabled: boolean; description: string | null; pack_owned?: boolean };
export type JavaRuntimeInfo = { version: string; major: number; arch: string; vendor: string; executable: string };
export type JavaSelection = { mode: 'automatic' | 'custom'; executable: string | null; runtime: JavaRuntimeInfo | null };
export type LauncherPreferences = { memory_mb: number; game_directory: string; custom_game_directory: boolean };
export type RuntimeOverrides = { java: { mode: 'inherit' | 'automatic' | 'custom'; executable?: string }; memory_mb: number | null };
export type Game = {
  id: string;
  name: string;
  version: string;
  loader_version?: string | null;
  loader_kind?: LoaderKind | null;
  pack_name?: string | null;
  pack_version?: string | null;
  profile_id?: string | null;
  runtime_overrides?: RuntimeOverrides;
  directory: string;
  status: 'installing' | 'installed' | 'failed';
  error: string | null;
  progress: { phase: 'metadata' | 'files' | 'assets' | 'fabric' | 'forge-installer' | 'neoforge-installer' | 'pack-base-metadata' | 'pack-base-files' | 'pack-base-assets' | 'pack-base-fabric' | 'pack-base-forge-installer' | 'pack-base-neoforge-installer' | 'pack-base-complete' | 'pack-download' | 'pack-overrides' | 'pack-install' | 'complete'; completed: number; total: number; reused: number } | null;
  content_installing?: boolean;
  launch_state?: 'idle' | 'preparing' | 'running';
  launch_error?: string | null;
  log_path?: string | null;
};
export type PackPreview = { preview_id: string; name: string; version_id: string; summary: string | null; minecraft_version: string; loader_kind?: LoaderKind | null; loader_version: string | null; required_files: number; optional_files: { path: string; size: number }[]; overrides: number };
export type OfflineProfile = { name: string; uuid: string };
export type PlayerProfile = OfflineProfile & { id: string; kind: 'offline'; note: string | null };
export type ProfileLibrary = { profiles: PlayerProfile[]; active_id: string | null };
export function effectiveProfile(game: Game, library: ProfileLibrary): PlayerProfile | null {
  return library.profiles.find((profile) => profile.id === (game.profile_id ?? library.active_id)) ?? null;
}
export function gameLoaderLabel(game: Game): string {
  if (!game.loader_version) return '原版';
  return `${loaderLabel(game.loader_kind ?? 'fabric')} ${game.loader_version}`;
}
export function loaderLabel(kind: string): string { return ({ fabric: 'Fabric', forge: 'Forge', neoforge: 'NeoForge' })[kind] ?? '原版'; }
export type ModSearchHit = { project_id: string; slug: string; title: string; description: string; author: string; downloads: number };
export type ModSearchPage = { hits: ModSearchHit[]; offset: number; total_hits: number };
export type ModInstallPreview = { plan_id: string; project_id: string; version_id: string; mods: { project_id: string; version_id: string; name: string; version_number: string; required_by: string | null }[] };

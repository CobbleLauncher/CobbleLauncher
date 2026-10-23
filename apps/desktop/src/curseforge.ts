// SPDX-License-Identifier: MIT
export interface CurseForgeHit { icon_url?: string | null; preview_url?: string | null; gallery?: string[]; project_id: string; slug: string; title: string; description: string; author: string; downloads: number; website_url: string | null; allow_distribution: boolean | null; available: boolean }
export interface CurseForgeFile { file_id: string; project_id: string; name: string; file_name: string; release_type: string; date_published: string; size_bytes: number; game_versions: string[]; dependencies: {project_id: string; relation: string; relation_type: number}[]; download_availability: 'available' | 'manual' | 'unavailable' }
export interface CurseForgeSearchPage { hits: CurseForgeHit[]; offset: number; total_hits: number }
export interface CurseForgeFilePage { files: CurseForgeFile[]; offset: number; total_hits: number }
export function mergeProjects<T extends { project_id: string }>(previous: T[], next: T[]) { return [...new Map([...previous, ...next].map(item => [item.project_id, item])).values()]; }
export function mergeFiles(previous: CurseForgeFile[], next: CurseForgeFile[]) { return [...new Map([...previous, ...next].map(item => [item.file_id, item])).values()]; }
export interface CurseForgeMissing {project_id: string; file_id: string; name: string; file_name: string; size_bytes: number; website_url: string | null; reason: 'manual' | 'unavailable'}
export interface CurseForgePlan {plan_id: string; project_id: string; version_id: string; mods: {project_id: string; version_id: string; name: string; version_number: string; required_by: string | null}[]; missing: CurseForgeMissing[]; ready: boolean}

export interface CurseForgePackPreview {
  preview_id: string; name: string; version_id: string; minecraft_version: string;
  loader_kind: string | null; loader_version: string | null; required_files: number;
  optional_files: {id: string; project_id: string; file_id: string}[];
  overrides: number; selected_optional_files: string[]; plan_id: string | null;
  files: {project_id: string; file_id: string; name: string; file_name: string; destination: string; size_bytes: number; required_by: string | null}[];
  missing: CurseForgeMissing[]; ready: boolean;
}
export function curseForgeLoaderLabel(value: string) { return ({fabric: 'Fabric', forge: 'Forge', neoforge: 'NeoForge', quilt: 'Quilt'} as Record<string, string>)[value] ?? value; }

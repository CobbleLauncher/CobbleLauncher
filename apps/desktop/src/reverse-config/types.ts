// SPDX-License-Identifier: MIT
import type { LoaderKind, ModInstallPreview } from '../games';
import type { CurseForgeMissing } from '../curseforge';

export type EnvironmentSource = 'modrinth' | 'curseforge';
export type EnvironmentProject = { source: EnvironmentSource; project_id: string };
export type EnvironmentMod = EnvironmentProject & { title: string; description: string; author: string; downloads: number; icon_url?: string | null; preview_url?: string | null };
export type EnvironmentSelection = EnvironmentProject & { version_id: string };
export type EnvironmentPlan = {
  game_version: string; loader: string; roots: EnvironmentSelection[];
  modrinth: Omit<ModInstallPreview, 'plan_id'> | null;
  curseforge: { graph: Omit<ModInstallPreview, 'plan_id'>; restricted: CurseForgeMissing[]; incompatible: string[] } | null;
  dependency_edges: { from: EnvironmentSelection; to: EnvironmentSelection }[];
};
export function projectKey(project: EnvironmentProject): string { return `${project.source}:${project.project_id}`; }
export function plannedMods(plan: EnvironmentPlan) {
  return [
    ...(plan.modrinth?.mods ?? []).map(mod => ({ ...mod, source: 'modrinth' as const })),
    ...(plan.curseforge?.graph.mods ?? []).map(mod => ({ ...mod, source: 'curseforge' as const })),
  ];
}

export type ReverseChoice = {
  plan_id: string;
  game_version: string;
  loader: LoaderKind;
  roots: EnvironmentSelection[];
  plan: EnvironmentPlan;
};
export type ReverseConfiguration = {
  choices: ReverseChoice[];
  conflicts: { game_version: string; loader: string; code: string; message: string; projects: EnvironmentProject[] }[];
  stats: { metadata_requests: number; candidate_versions: number; targets_considered: number; metadata_elapsed_ms: number; solve_elapsed_ms: number };
  limitations: string[];
};

export type EnvironmentRequirements = { ready: boolean; missing: CurseForgeMissing[] };
export type EnvironmentInstallationPreview = {
  loader_version: string;
  requirements: EnvironmentRequirements;
  space: { can_proceed: boolean; exact: boolean; volumes: { volume: { id: string; label: string; available_bytes: number | null }; additional_peak_bytes: number; unknown_sizes: number; verdict: 'sufficient' | 'insufficient' | 'unknown' }[] };
};

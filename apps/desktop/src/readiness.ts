// SPDX-License-Identifier: MIT
import type { CurseForgeMissing } from './curseforge';
export type ReadinessCondition = { state: 'ready' | 'missing' | 'unknown'; code: string };
export type ReadinessIssue = { category: string; path: string | null; state: string; size_bytes: number | null; repairable: boolean };
export type GameReadiness = {
  files: { ready: boolean; checked_files: number; ready_files: number; missing_files: number; changed_files: number; unknown: number; issues: ReadinessIssue[]; issues_truncated: boolean; required_java: number | null };
  java: ReadinessCondition; account: ReadinessCondition; server: ReadinessCondition;
};
export type SpaceEstimate = { can_proceed: boolean; exact: boolean; volumes: {
  volume: { id: string; label: string; available_bytes: number | null };
  additional_peak_bytes: number; unknown_sizes: number; verdict: 'sufficient' | 'insufficient' | 'unknown';
}[] };
export type EnvironmentRequirements = { ready: boolean; missing: CurseForgeMissing[] };
export type ReadinessPreparation = { space: SpaceEstimate; requirements: EnvironmentRequirements };
export function readinessTaskNeedsFiles(error: string | null): boolean {
  return !!error && /ENVIRONMENT_MANUAL_FILES_REQUIRED|CURSEFORGE_MANUAL_FILES_REQUIRED|ENVIRONMENT_FILE_REQUIRED/u.test(error);
}

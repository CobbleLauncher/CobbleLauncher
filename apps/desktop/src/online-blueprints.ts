// SPDX-License-Identifier: MIT
export type OnlineBlueprint = {
  id: string; title: string; description: string; version: number; format: string;
  author: string; attribution: string; license: string | null; java_release: string | null; namespaces: string[]; preview_url: string | null;
};
export type BlueprintFileProposal = { declared_bytes: number; original_filename: string; license: string | null };
export type OnlineBlueprintOutput = { search_scope?: 'all_versions'; items: OnlineBlueprint[]; next_cursor: string | null; detail: OnlineBlueprint | null; download: BlueprintFileProposal | null };
export type BlueprintProviderChoice = { id: string; name: string; origins: string[] };
export type ProviderCandidateSelection = { sourceId: string; pluginId: string };
export type BlueprintProviderCandidate = { source_id: string; plugin_id: string; name: string; version: string };
export type BlueprintProviderInterface = { id: string; version: string; implementations: { plugin_id: string; name: string; version: string }[]; candidates: BlueprintProviderCandidate[] };
export type BlueprintViewerChoice = { id: string; name: string; file_extensions: string[] };
export type BlueprintViewerChoices = { viewer_id: string | null; viewers: BlueprintViewerChoice[]; interface: BlueprintProviderInterface; disabled_viewers?: BlueprintViewerChoice[] };
export type BlueprintProviderAvailability = { available: boolean; provider_id: string | null; providers: BlueprintProviderChoice[]; viewer_available: boolean; interface?: BlueprintProviderInterface };
export type BlueprintProviderProgress = { request_id: string; phase: 'search' | 'details' | 'download' | 'viewer'; completed: number; total: number };
export function canPreviewOnlineBlueprint(item: OnlineBlueprint): boolean { return item.format === 'schem' || item.format === 'litematic'; }

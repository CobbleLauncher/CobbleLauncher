// SPDX-License-Identifier: MIT
import type { CatalogManifest, PluginInterface } from './plugin-catalogs';
import { manifestSettingFields } from './extensions/manifest-settings';
export type PluginDetailsData = {
  signature?: { status: 'unsigned' | 'verified'; rootId?: string; publisher?: string } | null;
  plugin: { manifest: CatalogManifest; enabled: boolean; problem?: string | null };
  provides: PluginInterface[];
  companions?: { plugin_id: string; name: string; version: string; requirement: PluginInterface; optional: boolean; installed: boolean; enabled: boolean; available: boolean; source_id: string | null; source_name: string | null; project_url: string | null }[];
  companions_truncated?: boolean;
  dependencies: {
    requirement: PluginInterface;
    optional: boolean;
    state: 'available' | 'selection_required' | 'optional_missing' | 'required_missing' | 'unsupported';
    implementations: { plugin_id: string; name: string; version: string; enabled: boolean; available: boolean; problem?: string | null }[];
    candidates_truncated?: boolean;
    candidates: { source_id: string; source_name: string; plugin_id: string; name: string; version: string; project_url: string | null }[];
  }[];
};
export function hasGlobalPluginSettings(manifest: CatalogManifest) {
  return manifestSettingFields(manifest.contributions?.settings).some(field => field.scope === 'global');
}

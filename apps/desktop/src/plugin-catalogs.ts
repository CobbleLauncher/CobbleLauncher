// SPDX-License-Identifier: MIT
import type { PermissionNode } from './plugin-permissions';
import type { ManifestSettings } from './extensions/manifest-settings';
export type PluginInterface = { id: string; version: string };
export type PluginFrontendManifest = { api_version: '0.1.0'; entry: 'ui/index.html'; assets_sha256: string };
export type CatalogManifest = {
  frontend?: PluginFrontendManifest;
  id: string;
  name: string;
  description?: string | { en: string; 'zh-CN'?: string };
  tags?: string[];
  icon?: { file: string; sha256: string };
  version: string;
  api_version: string;
  publisher: string;
  capabilities: string[];
  permissions?: PermissionNode[];
  file_extensions: string[];
  component_sha256: string;
  report_protocols?: string[];
  provider_origins?: string[];
  composer_permissions?: ('draft' | 'public_identity' | 'selected_peer' | 'selected_session')[];
  interfaces?: { provides?: PluginInterface[]; requires?: PluginInterface[]; optional?: PluginInterface[] };
  contributions?: { schema_version: number; commands?: unknown[]; settings?: ManifestSettings | null };
};

export type CatalogPlugin = {
  manifest: CatalogManifest;
  description: string;
  component_url?: string;
  package?: { url: string; sha256: string; size: number };
  icon_png_base64?: string;
  project_url?: string;
};

export type CatalogPreviewManifest = CatalogManifest;

export type PluginCatalogView = {
  source: { id: string; name: string; url: string; enabled: boolean };
  snapshot: null | {
    source_id: string;
    source_url: string;
    retrieved_at: number;
    index: { name: string; schema_version: number; plugins: CatalogPlugin[] };
  };
  error?: string;
};

export function isSupportedCatalogPlugin(plugin: CatalogPlugin): boolean {
  const { manifest } = plugin;
  const knownInterfaces = ['launcher:viewer', 'launcher:report', 'launcher:blueprint-provider', 'launcher:message-composer', 'launcher:image-layout', 'launcher:web-frontend'];
  const implemented = manifest.capabilities.length === 1 ? ({ viewer: 'launcher:viewer', report: 'launcher:report', 'blueprint-provider': 'launcher:blueprint-provider', 'message-composer': 'launcher:message-composer', 'image-layout':'launcher:image-layout' } as Record<string, string>)[manifest.capabilities[0]!] : undefined;
  if (manifest.interfaces?.provides?.some(item => knownInterfaces.includes(item.id) && (item.id === 'launcher:web-frontend' ? !manifest.frontend || item.version !== manifest.frontend.api_version : item.id !== implemented || item.version !== manifest.api_version))) return false;
  const requested = manifest.permissions ?? [];
  if (manifest.frontend && (manifest.frontend.api_version !== '0.1.0' || manifest.frontend.entry !== 'ui/index.html' || !/^[a-fA-F0-9]{64}$/.test(manifest.frontend.assets_sha256) || !requested.some(node => node.permission === 'launcher:ui/webview'))) return false;
  if (!manifest.frontend && requested.some(node => ['launcher:ui/webview', 'launcher:game-config/read', 'launcher:game-diagnostics/read', 'launcher:game-inspection/read', 'launcher:game-keybindings/read', 'launcher:game-keybindings/write'].includes(node.permission))) return false;
  if (requested.some(node => node.permission === 'launcher:game-keybindings/write') && !requested.some(node => node.permission === 'launcher:game-keybindings/read')) return false;
  const inspection=requested.find(node=>node.permission==='launcher:game-inspection/read');
  if(inspection && (!('scopes'in inspection)||!inspection.scopes.length||inspection.scopes.length>10||new Set(inspection.scopes).size!==inspection.scopes.length||inspection.scopes.some(scope=>!['mods','packs','worlds','backups','screenshots','storage','runtime','options','datapacks','servers'].includes(scope))||manifest.api_version!=='0.2.0'||manifest.capabilities.join()!=='report'||(manifest.file_extensions.length>0&&manifest.file_extensions.join()!=='json')||!!manifest.report_protocols?.length))return false;
  if (requested.some(node => node.permission === 'launcher:game-diagnostics/read') && (manifest.capabilities.join() !== 'report' || !!manifest.report_protocols?.length || !manifest.file_extensions.every(extension=>['log','txt'].includes(extension)))) return false;
  if (requested.some(node => node.permission === 'launcher:game-config/read') && (manifest.api_version !== '0.2.0' || manifest.capabilities.join() !== 'report' || !(manifest.report_protocols ?? []).includes('cobble.config-folder@1'))) return false;
  if (manifest.api_version === '0.1.0' && manifest.capabilities.length === 1 && manifest.capabilities[0] === 'image-layout') {
    const permissions=(manifest.permissions??[]).filter(node=>node.permission!=='launcher:ui/webview');
    return manifest.file_extensions.length===0 && !(manifest.report_protocols?.length) && !(manifest.provider_origins?.length) && !(manifest.composer_permissions?.length)
      && permissions.length===2 && permissions.some(node=>node.permission==='launcher:images/selected/read') && permissions.some(node=>node.permission==='launcher:images/output/propose');
  }
  if (manifest.api_version === '0.2.0' && manifest.capabilities.length === 1
    && manifest.capabilities[0] === 'blueprint-provider') {
    const origins = manifest.provider_origins ?? [];
    return manifest.file_extensions.length === 0 && !(manifest.report_protocols?.length)
      && origins.length > 0 && origins.length <= 4 && origins.every(origin => {
        try { const url = new URL(origin); return url.protocol === 'https:' && url.origin === origin && !url.username && !url.password; }
        catch { return false; }
      });
  }
  if (['0.1.0', '0.2.0'].includes(manifest.api_version) && manifest.capabilities.length === 1
    && manifest.capabilities[0] === 'report') {
    const protocols = manifest.report_protocols ?? [];
    if (protocols.length > 1 || protocols.some(protocol => !['cobble.mrpack-page@1', 'cobble.pack-metadata@1', 'cobble.config-folder@1'].includes(protocol))) return false;
    if (protocols.includes('cobble.mrpack-page@1') && !manifest.file_extensions.some(extension => extension === 'json' || extension === 'mrpack')) return false;
    if (protocols.includes('cobble.pack-metadata@1') && !manifest.file_extensions.every(extension => ['json', 'mcmeta', 'zip'].includes(extension))) return false;
    if (protocols.includes('cobble.config-folder@1')) {
      const node=manifest.permissions?.find(value=>value.permission==='launcher:resource/selected-folder/read');
      if(manifest.api_version!=='0.2.0'||!manifest.file_extensions.every(extension=>['json','toml','properties','yaml','yml','txt'].includes(extension))||!node||!('extensions' in node)||node.extensions.length!==manifest.file_extensions.length||!manifest.file_extensions.every(extension=>node.extensions.includes(extension)))return false;
    }
    if (manifest.file_extensions.includes('zip') && !protocols.includes('cobble.pack-metadata@1')) return false;
    const extensions = manifest.file_extensions;
    return (extensions.length > 0 || !!inspection) && extensions.length <= 8 && new Set(extensions).size === extensions.length
      && extensions.every(extension => ['log', 'txt', 'json', 'mrpack', 'toml', 'properties', 'yaml', 'yml', 'mcmeta', 'zip'].includes(extension));
  }
  if (manifest.api_version === '0.1.0' && manifest.capabilities.length === 1
    && manifest.capabilities[0] === 'message-composer') {
    const permissions = manifest.composer_permissions ?? [];
    const allowed = ['draft', 'public_identity', 'selected_peer', 'selected_session'];
    return manifest.file_extensions.length === 0 && !(manifest.report_protocols?.length)
      && !(manifest.provider_origins?.length)
      && permissions.length > 0 && permissions.length <= allowed.length
      && new Set(permissions).size === permissions.length
      && permissions.every(permission => allowed.includes(permission))
      && permissions.includes('draft');
  }
  return !(manifest.report_protocols?.length) && manifest.api_version === '1.0.0'
    && manifest.capabilities.length === 1
    && manifest.capabilities[0] === 'viewer'
    && manifest.file_extensions.length > 0
    && new Set(manifest.file_extensions).size === manifest.file_extensions.length
    && manifest.file_extensions.every(extension => ['schem', 'litematic'].includes(extension));
}


export function matchesPluginMetadata(manifest: Pick<CatalogManifest, 'id' | 'name' | 'description' | 'tags'> & { publisher?: string }, query: string, fallbackDescription = ''): boolean {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const description = typeof manifest.description === 'string' ? manifest.description : manifest.description ? Object.values(manifest.description).join(' ') : fallbackDescription;
  const content = [manifest.id, manifest.name, description, manifest.publisher ?? '', ...(manifest.tags ?? [])].join(' ').toLocaleLowerCase();
  return words.every(word => content.includes(word));
}

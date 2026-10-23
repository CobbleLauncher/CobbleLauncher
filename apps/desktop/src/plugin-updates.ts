// SPDX-License-Identifier: MIT
import type { CatalogManifest, PluginCatalogView } from './plugin-catalogs';
import { isSupportedCatalogPlugin } from './plugin-catalogs';
import { addedPermissionNodes } from './plugin-permissions';

export type PluginUpdate = { manifest: CatalogManifest; previous: CatalogManifest; sourceName: string; sourceId?: string };
export function batchCompatible(next: CatalogManifest, previous: CatalogManifest): boolean {
  return next.publisher === previous.publisher && next.api_version === previous.api_version
    && JSON.stringify(next.capabilities) === JSON.stringify(previous.capabilities)
    && JSON.stringify(next.interfaces ?? {}) === JSON.stringify(previous.interfaces ?? {})
    && !addedPermissionNodes(next, previous).length;
}
export function sameUpdateManifest(left: CatalogManifest, right: CatalogManifest): boolean {
  function stable(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(stable);
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => [key, stable(item)]));
    return value;
  }
  return JSON.stringify(stable(left)) === JSON.stringify(stable(right));
}
export function availablePluginUpdates(installed: CatalogManifest[], bundled: CatalogManifest[], catalogs: PluginCatalogView[]): PluginUpdate[] {
  const candidates = [
    ...bundled.map(manifest => ({ manifest, sourceName: 'CobbleLauncher' })),
    ...catalogs.filter(catalog => catalog.source.enabled).flatMap(catalog => (catalog.snapshot?.index.plugins ?? [])
      .filter(isSupportedCatalogPlugin).map(plugin => ({ manifest: plugin.manifest, sourceName: catalog.source.name, sourceId: catalog.source.id }))),
  ];
  return candidates.flatMap(candidate => {
    const previous = installed.find(manifest => manifest.id === candidate.manifest.id);
    // Do not guess ordering for custom version strings or offer catalog downgrades.
    const version = (value: string) => /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value) ? value.split('.').map(Number) : null;
    const next = version(candidate.manifest.version), old = previous && version(previous.version);
    if (!previous || !next || !old) return [];
    const difference = next.map((part, index) => part - old[index]!).find(part => part !== 0) ?? 0;
    return difference > 0 ? [{ ...candidate, previous }] : [];
  });
}

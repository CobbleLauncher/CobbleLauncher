// SPDX-License-Identifier: MIT
export type VersionOption = { value: string; label: string; search?: string };

export function preferredRelease<T extends { version_type: string }>(versions: T[]): T | undefined {
  return versions.find((version) => version.version_type === 'release') ?? versions[0];
}

export function visibleVersions(options: VersionOption[], query: string, limit: number, pinned: string[]) {
  const term = query.trim().toLocaleLowerCase();
  const matches = term ? options.filter((option) => `${option.label} ${option.search ?? ''}`.toLocaleLowerCase().includes(term)) : options;
  return {
    options: term ? matches.slice(0, limit) : options.filter((option, index) => index < limit || pinned.includes(option.value)),
    more: matches.length > limit,
  };
}

export function suggestedGameName(version: string, loader: string, loaderVersion: string) {
  return ['Minecraft', version, loader === 'vanilla' ? '' : ({ fabric: 'Fabric', forge: 'Forge', neoforge: 'NeoForge' }[loader] ?? loader), loader === 'vanilla' ? '' : loaderVersion].filter(Boolean).join(' ');
}

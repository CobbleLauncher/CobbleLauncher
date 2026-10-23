// SPDX-License-Identifier: MIT
// Package IDs and their actual workspace crates. Frontends are discovered from manifests.
export const officialPlugins = [
  ['schematic-viewer', 'cobble-schematic-viewer'],
  ['crash-doctor', 'cobble-crash-doctor'],
  ['modpack-inspector', 'cobble-modpack-inspector-guest'],
  ['mcs-provider', 'cobble-mcs-provider'],
  ['session-card', 'cobble-session-card'],
  ['config-doctor', 'cobble-config-doctor-guest'],
  ['pack-metadata', 'cobble-pack-metadata-guest'],
  ['screenshot-sheet', 'cobble-screenshot-sheet'],
  ...['mod-conflicts', 'resource-pack-stack', 'world-dashboard', 'backup-auditor',
    'datapack-auditor', 'serverbook-inspector', 'screenshot-timeline', 'controls-auditor',
    'storage-lens', 'runtime-advisor'].map(id => [id, `cobble-${id}-guest`]),
];
export const officialPluginIds = officialPlugins.map(([id]) => id);

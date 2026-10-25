// SPDX-License-Identifier: MIT
// Package IDs and their actual workspace crates. Frontends are discovered from manifests.
export const officialPlugins = [
  ['schematic-viewer', 'cobble-schematic-viewer'],
  ['crash-doctor', 'cobble-crash-doctor'],
  ['modpack-inspector', 'cobble-modpack-inspector-guest'],
  ['world-chronicle', 'cobble-world-chronicle-guest'],
  ['command-pocket', 'cobble-command-pocket-guest'],
  ['seed-compass', 'cobble-seed-compass-guest'],
  ['coordinate-book', 'cobble-coordinate-book-guest'],
  ['material-planner', 'cobble-material-planner-guest'],
  ['play-manifest', 'cobble-play-manifest-guest'],
  ['lang-doctor', 'cobble-lang-doctor-guest'],
  ['trial-copy', 'cobble-trial-copy-guest'],
  ['file-inbox', 'cobble-file-inbox-guest'],
  ['ai-assistant', 'cobble-ai-assistant-guest'],
  ['anvil-planner', 'cobble-anvil-planner-guest'],
  ['geometry-builder', 'cobble-geometry-builder-guest'],
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

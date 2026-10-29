// SPDX-License-Identifier: MIT
import type { CommandAction } from '../commands/registry';
import type { AppIconName } from '../ui/icons';

// First-party command IDs are stable manifest identities; execution stays in the registry.
const toolIcons: Record<string, AppIconName> = {
  'seed-compass': 'compass', 'coordinate-book': 'pin', 'material-planner': 'blocks',
  'schematic-viewer': 'blocks', 'geometry-builder': 'blocks', 'command-pocket': 'keyboard',
  'anvil-planner': 'settings', 'storage-lens': 'hard-drive', 'config-doctor': 'sliders-horizontal',
  'crash-doctor': 'file-search', 'world-dashboard': 'folder', 'world-chronicle': 'refresh-cw',
  'mod-conflicts': 'network', 'runtime-advisor': 'cpu', 'lang-doctor': 'languages',
  'file-inbox': 'download', 'trial-copy': 'copy', 'play-manifest': 'share',
  'screenshot-timeline': 'monitor', 'screenshot-sheet': 'monitor', 'backup-auditor': 'shield',
};
export function toolPresentation(command: CommandAction) {
  const pluginId = /^plugin:([^:]+):/.exec(command.id)?.[1];
  return {
    label: pluginId ? command.label.replace(/^(?:Open\s+|打开)/i, '').trim() || command.label : command.label,
    icon: (pluginId && toolIcons[pluginId]) || command.icon || 'puzzle',
  };
}

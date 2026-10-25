// SPDX-License-Identifier: MIT
import type { CommandAction } from '../commands/registry';
export function workspaceActions(view: string, commands: readonly CommandAction[]): CommandAction[] {
  const ids = view === 'library' ? ['game.create', 'game.import', 'settings.home-layout', 'palette.open', 'settings.open']
    : view === 'details' || view === 'running' ? ['game.content', 'game.readiness', 'navigation.tools', 'palette.open', 'settings.open']
    : view === 'tools' || view === 'extension-detail' ? ['navigation.extensions', 'palette.open', 'settings.open']
    : view === 'extensions' ? ['navigation.tools', 'palette.open', 'settings.open']
    : view === 'settings' ? ['settings.appearance', 'settings.home-layout', 'palette.open']
    : view === 'discover-packs' ? ['game.create', 'library.search', 'palette.open', 'settings.open']
    : ['palette.open', 'navigation.library', 'settings.open'];
  return ids.flatMap(id => { const action = commands.find(candidate => candidate.id === id && candidate.enabled !== false); return action ? [action] : []; });
}

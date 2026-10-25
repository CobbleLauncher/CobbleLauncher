// SPDX-License-Identifier: MIT
import { invoke } from '@tauri-apps/api/core';
import type { CommandAction } from './commands/registry';
import type { HomeGame } from './library-home';
import { t } from './i18n';

export function createGameActions(game: HomeGame, callbacks: { manage: (id: string) => void; updated: (game: HomeGame) => void }): CommandAction[] {
  async function update(command: string, extra: Record<string, unknown> = {}) {
    const result = await invoke<HomeGame | null>(command, { id: game.id, ...extra });
    if (result) callbacks.updated(result);
  }
  return [
    ...(game.status === 'installed' ? [{ id: 'game.manage', icon: 'settings' as const, label: t('home.manage', { name: game.name }), run: () => callbacks.manage(game.id) }] : []),
    ...(game.status === 'installed' ? [{ id: 'game.shortcut', icon: 'external-link' as const, label: t('home.shortcutCreate'), run: async () => { await invoke('create_game_shortcut', { id: game.id }); } }] : []),
    { id: 'game.pin', icon: 'pin' as const, label: t(game.pinned ? 'home.unpin' : 'home.pin'), run: () => update('set_game_pinned', { pinned: !game.pinned }) },
    { id: 'game.cover', icon: 'palette' as const, label: t('home.chooseCover'), run: () => update('pick_game_cover') },
    ...(game.cover_ref ? [{ id: 'game.resetCover', icon: 'x' as const, label: t('home.removeCover'), run: () => update('reset_game_cover') }] : []),
  ];
}

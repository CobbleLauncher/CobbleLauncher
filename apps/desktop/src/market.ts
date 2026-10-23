// SPDX-License-Identifier: MIT
import type { Game } from './games';
export type MarketKind = 'modpack' | 'mod' | 'resource_pack' | 'shader_pack' | 'blueprint';
export function marketGames(games: Game[], kind: MarketKind) {
  return games.filter(game => game.status === 'installed' && (kind !== 'mod' || !!game.loader_version));
}
export function marketGameLocked(game: Game) {
  return !!game.content_installing || game.launch_state === 'preparing' || game.launch_state === 'running';
}

// SPDX-License-Identifier: MIT
import type { Game } from './games';

export type HomeGame = Game;
function launchTime(game: HomeGame): number {
  const value = game.last_launched_at;
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0;
}
/** Native launch times are Unix milliseconds. Older records retain library order. */
export function homeGames(games: readonly HomeGame[]) {
  const ordered = games.map((game, index) => ({ game, index }))
    .sort((a, b) => launchTime(b.game) - launchTime(a.game) || a.index - b.index).map(({ game }) => game);
  const featured = ordered.find(game => game.status === 'installed') ?? null;
  const others = ordered.filter(game => game.id !== featured?.id).sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned));
  return { featured, recent: others.slice(0, 4), ordered: [...(featured ? [featured] : []), ...others] };
}
export function searchHomeGames(games: readonly HomeGame[], query: string): HomeGame[] {
  const needle = query.trim().toLowerCase();
  return needle ? games.filter(game => [game.name, game.version, game.loader_kind, game.loader_version, game.pack_name]
    .some(value => value?.toLowerCase().includes(needle))) : [...games];
}

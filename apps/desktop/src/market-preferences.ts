// SPDX-License-Identifier: MIT
import type { Game } from './games';
import { marketGames, type MarketKind } from './market';

const key = 'cobble.market.targets.v1';
function read(): Partial<Record<MarketKind, string>> {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? '{}');
    if (value && typeof value === 'object' && !Array.isArray(value)) return value as Partial<Record<MarketKind, string>>;
  } catch { /* Browsing remains usable when local preferences are unavailable. */ }
  return {};
}

/** Restore only a previously selected, currently eligible installed game. */
export function marketTarget(games: Game[], kind: MarketKind, explicit?: string): string {
  const value = explicit ?? read()[kind];
  return typeof value === 'string' && marketGames(games, kind).some(game => game.id === value) ? value : '';
}

export function rememberMarketTarget(kind: MarketKind, id: string): void {
  try { localStorage.setItem(key, JSON.stringify({ ...read(), [kind]: id })); }
  catch { /* Local preferences must not prevent target selection or installation. */ }
}

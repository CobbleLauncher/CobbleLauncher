// SPDX-License-Identifier: MIT
export const gameDetailTabs = ['content', 'runtime', 'worlds', 'screenshots', 'configs', 'servers'] as const;
export type GamePageTab = typeof gameDetailTabs[number];
export function isGamePageTab(value: unknown): value is GamePageTab {
  return typeof value === 'string' && gameDetailTabs.some(tab => tab === value);
}

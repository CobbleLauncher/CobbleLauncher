// SPDX-License-Identifier: MIT
import { beforeEach, expect, it } from 'vitest';
import { isRunningToolPinned, rankRunningTools, readRunningToolPins, toggleRunningToolPin } from './running-tool-pins';

beforeEach(() => localStorage.clear());
it('keeps pins scoped to a game and ranks only currently eligible tools', () => {
  const first = { pluginId: 'first', commandId: 'open' }, second = { pluginId: 'second', commandId: 'open' };
  const pins = toggleRunningToolPin({}, 'game-one', second);
  expect(isRunningToolPinned(readRunningToolPins(), 'game-one', second)).toBe(true);
  expect(isRunningToolPinned(pins, 'game-two', second)).toBe(false);
  expect(rankRunningTools([first, second], pins['game-one'] ?? [])).toEqual([second, first]);
  expect(rankRunningTools([first], pins['game-one'] ?? [])).toEqual([first]);
  expect(isRunningToolPinned(toggleRunningToolPin(pins, 'game-one', second), 'game-one', second)).toBe(false);
});

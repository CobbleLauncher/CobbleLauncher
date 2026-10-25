// SPDX-License-Identifier: MIT
import { beforeEach, expect, it } from 'vitest';
import { readCoverFocus, writeCoverFocus } from './cover-focus';
beforeEach(() => localStorage.clear());
it('retains focal point only for the matching cover image', () => {
  writeCoverFocus('game-one', 'cover-a', { x: 25, y: 73 });
  expect(readCoverFocus('game-one', 'cover-a')).toEqual({ x: 25, y: 73 });
  expect(readCoverFocus('game-one', 'cover-b')).toEqual({ x: 50, y: 50 });
  expect(readCoverFocus('game-two', 'cover-a')).toEqual({ x: 50, y: 50 });
});

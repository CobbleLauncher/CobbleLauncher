// SPDX-License-Identifier: MIT
import { afterEach, expect, it, vi } from 'vitest';
import { createWorkspaceMotion } from './workspace-motion';
afterEach(() => vi.unstubAllGlobals());
it('cancels the previous transition immediately and honors both reduced-motion preferences', () => {
  const media = vi.fn(() => ({ matches: false }));
  vi.stubGlobal('matchMedia', media);
  const node = document.createElement('div');
  const cancel = vi.fn();
  node.animate = vi.fn(() => ({ cancel }) as unknown as Animation);
  const motion = createWorkspaceMotion();
  motion.play(node, 'forward'); motion.play(node, 'back');
  expect(cancel).toHaveBeenCalledTimes(1);
  expect(node.animate).toHaveBeenLastCalledWith(expect.arrayContaining([expect.objectContaining({ transform: 'translateX(-8px)' })]), expect.objectContaining({ duration: 160 }));
  node.dataset.motion = 'reduced'; motion.play(node, 'peer');
  expect(cancel).toHaveBeenCalledTimes(2); expect(node.animate).toHaveBeenCalledTimes(2);
  delete node.dataset.motion; media.mockReturnValue({ matches: true }); motion.play(node, 'peer');
  expect(node.animate).toHaveBeenCalledTimes(2);
});

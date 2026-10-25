// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { afterHomePaint } from './after-home-paint';

afterEach(() => vi.useRealTimers());
describe('deferred home work', () => {
  it('leaves a paint between readiness and background work', () => {
    vi.useFakeTimers();
    const run = vi.fn();
    afterHomePaint(run);
    expect(run).not.toHaveBeenCalled();
    vi.advanceTimersToNextFrame();
    expect(run).not.toHaveBeenCalled();
    vi.advanceTimersToNextFrame();
    expect(run).toHaveBeenCalledOnce();
  });
  it('cancels either pending frame when the app is disposed', () => {
    vi.useFakeTimers();
    const run = vi.fn();
    afterHomePaint(run)();
    vi.advanceTimersToNextFrame();
    const cancel = afterHomePaint(run);
    vi.advanceTimersToNextFrame();
    cancel();
    vi.runAllTimers();
    expect(run).not.toHaveBeenCalled();
  });
});

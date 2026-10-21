// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { applyTheme, readTheme } from './theme';

afterEach(() => { localStorage.clear(); vi.restoreAllMocks(); });

describe('local appearance preference', () => {
  it('uses system theme for an unknown persisted value', () => {
    localStorage.setItem('cobble.appearance', 'unexpected');
    expect(readTheme()).toBe('system');
  });

  it('works for this session when local storage is unavailable', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('blocked'); });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('full'); });
    expect(readTheme()).toBe('system');
    expect(() => applyTheme('dark')).not.toThrow();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });
});

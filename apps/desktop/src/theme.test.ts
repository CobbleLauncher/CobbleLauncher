// SPDX-License-Identifier: MIT
import { afterEach, describe, expect, it, vi } from 'vitest';
import { appearancePreviewStyle, applyAppearance, applyTheme, defaultAppearance, readAppearance, readTheme, resetAppearance, resolvedTheme } from './theme';

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
  it('keeps an existing theme when new choices are first read', () => {
    localStorage.setItem('cobble.appearance', 'dark');
    expect(readAppearance()).toEqual({ ...defaultAppearance, theme: 'dark' });
    applyAppearance({ theme: 'dark', accent: 'blue', density: 'compact', scale: 110, textSize: 14, motion: 'reduced' });
    expect(document.documentElement.dataset.accent).toBe('blue');
    expect(document.documentElement.style.getPropertyValue('--appearance-scale')).toBe('1.1');
    expect(readAppearance().density).toBe('compact');
    applyTheme('light');
    expect(readAppearance()).toMatchObject({ theme: 'light', accent: 'blue', scale: 110 });
    expect(resetAppearance()).toEqual(defaultAppearance);
  });
  it('migrates older records and round trips intermediate scale, text size and comfortable spacing', () => {
    localStorage.setItem('cobble.appearance', 'dark');
    localStorage.setItem('cobble.appearance.preferences', JSON.stringify({ accent: 'purple', density: 'compact', scale: 110, motion: 'reduced' }));
    expect(readAppearance()).toEqual({ theme: 'dark', accent: 'purple', density: 'compact', scale: 110, textSize: 14, motion: 'reduced' });
    applyAppearance({ ...readAppearance(), scale: 105, textSize: 18, density: 'comfortable' });
    expect(readAppearance()).toMatchObject({ scale: 105, textSize: 18, density: 'comfortable' });
    expect(document.documentElement.style.getPropertyValue('--font-size-base')).toBe('18px');
  });
  it('builds local preview styles without storage or root changes and resolves system theme', () => {
    const previous = document.documentElement.style.cssText;
    expect(appearancePreviewStyle({ ...defaultAppearance, scale: 95, textSize: 16 })).toEqual({ '--font-size-base': '16px', zoom: 'calc(0.95 / var(--appearance-scale, 1))' });
    expect(document.documentElement.style.cssText).toBe(previous);
    expect(localStorage.getItem('cobble.appearance.preferences')).toBeNull();
    expect(resolvedTheme('system', true)).toBe('dark');
    expect(resolvedTheme('system', false)).toBe('light');
    expect(resolvedTheme('light', true)).toBe('light');
  });
  it('rejects malformed and unsupported stored appearance values', () => {
    localStorage.setItem('cobble.appearance.preferences', JSON.stringify({ accent: 'neon', density: 'tiny', scale: 999, motion: 'off' }));
    expect(readAppearance()).toEqual(defaultAppearance);
    localStorage.setItem('cobble.appearance.preferences', '{');
    expect(readAppearance()).toEqual(defaultAppearance);
  });
});

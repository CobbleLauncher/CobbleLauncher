// SPDX-License-Identifier: MIT
export type Theme = 'system' | 'light' | 'dark';
const key = 'cobble.appearance';

export function readTheme(): Theme {
  try {
    const theme = localStorage.getItem(key);
    return theme === 'light' || theme === 'dark' ? theme : 'system';
  } catch {
    return 'system';
  }
}

export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(key, theme);
  } catch {
    // Appearance still works for this session when storage is unavailable.
  }
}

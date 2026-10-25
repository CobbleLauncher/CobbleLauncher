// SPDX-License-Identifier: MIT
export type Theme = 'system' | 'light' | 'dark';
export type Accent = 'green' | 'blue' | 'amber' | 'purple';
export type Density = 'standard' | 'compact' | 'comfortable';
export type Motion = 'system' | 'reduced';
export type Scale = 90 | 95 | 100 | 105 | 110 | 115 | 120 | 125;
export type TextSize = 13 | 14 | 15 | 16 | 17 | 18;
export type Appearance = { theme: Theme; accent: Accent; density: Density; scale: Scale; textSize: TextSize; motion: Motion };

const themeKey = 'cobble.appearance';
const detailKey = 'cobble.appearance.preferences';
export const defaultAppearance: Appearance = { theme: 'system', accent: 'green', density: 'standard', scale: 100, textSize: 14, motion: 'system' };

function choice<T extends string | number>(value: unknown, choices: readonly T[], fallback: T): T {
  return choices.includes(value as T) ? value as T : fallback;
}

export function readTheme(): Theme {
  try { return choice(localStorage.getItem(themeKey), ['system', 'light', 'dark'], 'system'); }
  catch { return 'system'; }
}

export function readAppearance(): Appearance {
  let stored: Partial<Appearance> = {};
  try {
    const value = localStorage.getItem(detailKey);
    if (value) {
      const parsed: unknown = JSON.parse(value);
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) stored = parsed as Partial<Appearance>;
    }
  } catch { /* Storage and malformed records fall back to defaults. */ }
  return {
    theme: readTheme(),
    accent: choice(stored.accent, ['green', 'blue', 'amber', 'purple'], 'green'),
    density: choice(stored.density, ['standard', 'compact', 'comfortable'], 'standard'),
    scale: choice(stored.scale, [90, 95, 100, 105, 110, 115, 120, 125], 100),
    textSize: choice(stored.textSize, [13, 14, 15, 16, 17, 18], 14),
    motion: choice(stored.motion, ['system', 'reduced'], 'system'),
  };
}

export function applyAppearance(preference: Appearance): void {
  const root = document.documentElement;
  root.dataset.theme = preference.theme;
  root.dataset.accent = preference.accent;
  root.dataset.density = preference.density;
  root.dataset.motion = preference.motion;
  root.style.setProperty('--appearance-scale', String(preference.scale / 100));
  root.style.setProperty('--font-size-base', `${preference.textSize}px`);
  try {
    localStorage.setItem(themeKey, preference.theme);
    localStorage.setItem(detailKey, JSON.stringify({ accent: preference.accent, density: preference.density, scale: preference.scale, textSize: preference.textSize, motion: preference.motion }));
  } catch { /* Appearance still works for this session. */ }
}

/** Preview styles are local to the sample; editing never mutates the app root. */
export function appearancePreviewStyle(preference: Appearance) {
  return { '--font-size-base': `${preference.textSize}px`, zoom: `calc(${preference.scale / 100} / var(--appearance-scale, 1))` };
}

export function resolvedTheme(theme: Theme, systemDark = typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches): 'light' | 'dark' {
  return theme === 'system' ? (systemDark ? 'dark' : 'light') : theme;
}

export function applyTheme(theme: Theme): void {
  applyAppearance({ ...readAppearance(), theme });
}

export function resetAppearance(): Appearance {
  applyAppearance(defaultAppearance);
  return { ...defaultAppearance };
}

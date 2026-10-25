// SPDX-License-Identifier: MIT
import type { Context } from './index.ts';

/** Opt-in for bundled plugin frontends. Third-party frontends keep control of their own UI. */
export function applyHostAppearance(context: Context, root: HTMLElement = document.documentElement): Context {
  const theme = context.theme === 'system'
    ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
    : context.theme;
  root.dataset.theme = theme;
  root.dataset.hostAppearance = 'true';
  if (context.appearance) {
    root.dataset.accent = context.appearance.accent;
    root.dataset.density = context.appearance.density;
    root.dataset.motion = context.appearance.motion;
    root.dataset.hostScale = String(context.appearance.scale);
    const textSize = context.appearance.textSize ?? 14;
    root.dataset.hostTextSize = String(textSize);
    root.style.setProperty('--font-size-base', `${textSize}px`);
    root.style.setProperty('--appearance-scale', String(context.appearance.scale / 100));
  } else {
    delete root.dataset.accent;
    delete root.dataset.density;
    delete root.dataset.motion;
    delete root.dataset.hostScale;
    delete root.dataset.hostTextSize;
    root.style.removeProperty('--font-size-base');
    root.style.removeProperty('--appearance-scale');
  }
  return context;
}

export async function hostContext(pending: Promise<Context>): Promise<Context> {
  return applyHostAppearance(await pending);
}

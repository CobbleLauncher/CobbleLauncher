// SPDX-License-Identifier: MIT
import { nextTick } from 'vue';

const states = new Map<string, unknown>();
const scrolls = new Map<string, number>();

/** Session-only browse state; project data and filters never leave this process. */
export function rememberMarketWorkstate<T>(key: string, state: T, root?: HTMLElement) {
  states.set(key, state);
  const scroll = root?.closest<HTMLElement>('.page-scroll, .app-content, main') ?? document.scrollingElement;
  if (scroll) scrolls.set(key, scroll.scrollTop);
}

export function marketWorkstate<T>(key: string): T | undefined {
  return states.get(key) as T | undefined;
}

export function restoreMarketScroll(key: string, root?: HTMLElement) {
  const top = scrolls.get(key);
  if (top === undefined) return;
  void nextTick(() => {
    const scroll = root?.closest<HTMLElement>('.page-scroll, .app-content, main') ?? document.scrollingElement;
    if (scroll) scroll.scrollTop = top;
  });
}

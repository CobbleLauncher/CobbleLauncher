// SPDX-License-Identifier: MIT
export type HomeSection = 'other' | 'quick' | 'favorites';
export type HomeLayout = { order: HomeSection[]; hidden: HomeSection[]; quickIds: string[]; libraryView: 'list' | 'cards' };
export const defaultHomeLayout: HomeLayout = { order: ['other', 'quick', 'favorites'], hidden: [], quickIds: [], libraryView: 'list' };
const key = 'cobble.home.layout.v1';
const sections = new Set<HomeSection>(defaultHomeLayout.order);
const validId = (value: unknown): value is string => typeof value === 'string' && /^[a-zA-Z0-9_.:-]{1,120}$/.test(value);
export function readHomeLayout(): HomeLayout {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? '{}');
    if (!value || typeof value !== 'object' || Array.isArray(value)) return structuredClone(defaultHomeLayout);
    const raw = value as Record<string, unknown>;
    const order = Array.isArray(raw.order) ? [...new Set(raw.order.filter((item): item is HomeSection => sections.has(item as HomeSection)))] : [];
    const hidden = Array.isArray(raw.hidden) ? [...new Set(raw.hidden.filter((item): item is HomeSection => sections.has(item as HomeSection)))] : [];
    const quickIds = Array.isArray(raw.quickIds) ? [...new Set(raw.quickIds.filter(validId))].slice(0, 6) : [];
    return { order: [...order, ...defaultHomeLayout.order.filter(item => !order.includes(item))], hidden, quickIds, libraryView: raw.libraryView === 'cards' ? 'cards' : 'list' };
  } catch { return structuredClone(defaultHomeLayout); }
}
export function writeHomeLayout(value: HomeLayout) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* Changes remain in this session. */ }
  if (typeof window !== 'undefined') window.dispatchEvent(new Event('cobble-home-layout-changed'));
}
export function moveHomeSection(layout: HomeLayout, section: HomeSection, direction: -1 | 1): HomeLayout {
  const order = [...layout.order];
  const current = order.indexOf(section), target = current + direction;
  if (current < 0 || target < 0 || target >= order.length) return layout;
  [order[current], order[target]] = [order[target]!, order[current]!];
  return { ...layout, order };
}

// SPDX-License-Identifier: MIT
export type CoverFocus = { x: number; y: number };
const key = 'cobble.cover-focus.v1';
const center: CoverFocus = { x: 50, y: 50 };
function valid(value: unknown): value is number { return typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 100; }
export function readCoverFocus(gameId: string, coverRef: string | null | undefined): CoverFocus {
  if (!coverRef) return { ...center };
  try {
    const raw = localStorage.getItem(key);
    if (!raw || raw.length > 65_536) return { ...center };
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== 'object' || Array.isArray(value)) return { ...center };
    const entry = (value as Record<string, unknown>)[gameId];
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) return { ...center };
    const focus = entry as Record<string, unknown>;
    return focus.coverRef === coverRef && valid(focus.x) && valid(focus.y) ? { x: focus.x, y: focus.y } : { ...center };
  } catch { return { ...center }; }
}
export function writeCoverFocus(gameId: string, coverRef: string | null | undefined, focus: CoverFocus): void {
  if (!coverRef || !valid(focus.x) || !valid(focus.y) || !/^[a-zA-Z0-9_-]{1,100}$/.test(gameId)) return;
  try {
    const raw = localStorage.getItem(key);
    const parsed: unknown = raw && raw.length <= 65_536 ? JSON.parse(raw) : {};
    const entries = parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed as Record<string, unknown> : {};
    localStorage.setItem(key, JSON.stringify({ ...Object.fromEntries(Object.entries(entries).slice(-500)), [gameId]: { coverRef, ...focus } }));
  } catch { /* Keep the crop for this session. */ }
}

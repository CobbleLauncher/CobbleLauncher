// SPDX-License-Identifier: MIT
export const shortcutDefaults = {
  'palette.open': 'Ctrl+K',
  'library.search': 'Ctrl+F',
  'navigation.back': 'Alt+ArrowLeft',
  'game.launch': 'Ctrl+Enter',
  'settings.open': 'Ctrl+,',
} as const;
export type ShortcutId = keyof typeof shortcutDefaults;
export type ShortcutMap = Record<string, string | null>;
const storageKey = 'cobble-shortcuts-v1';
export const shortcutChangedEvent = 'cobble-shortcuts-changed';
const ids = Object.keys(shortcutDefaults) as ShortcutId[];

export function normalizeShortcut(value: string): string | null {
  const parts = value.split('+').map(part => part.trim()).filter(Boolean);
  const key = parts.pop();
  if (!key) return null;
  const modifiers = new Set(parts.map(part => part.toLowerCase()));
  if ([...modifiers].some(part => !['ctrl', 'alt', 'shift', 'meta'].includes(part)) || modifiers.size !== parts.length) return null;
  const special: Record<string, string> = {
    arrowleft: 'ArrowLeft', arrowright: 'ArrowRight', arrowup: 'ArrowUp', arrowdown: 'ArrowDown',
    enter: 'Enter', home: 'Home', end: 'End', pageup: 'PageUp', pagedown: 'PageDown',
    delete: 'Delete', backspace: 'Backspace', insert: 'Insert', tab: 'Tab', ',': ',',
  };
  const named = special[key.toLowerCase()] ?? key.toUpperCase();
  if (!/^[A-Z0-9,]$/.test(named) && !/^F(?:[1-9]|1\d|2[0-4])$/.test(named) && !Object.values(special).includes(named)) return null;
  if (!modifiers.size) return null;
  return [...(['ctrl', 'alt', 'shift', 'meta'] as const).filter(part => modifiers.has(part)).map(part => part[0]!.toUpperCase() + part.slice(1)), named].join('+');
}

export function shortcutFromEvent(event: KeyboardEvent): string | null {
  if (event.isComposing || event.key === 'Process' || event.key === 'Dead') return null;
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return null;
  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
  return normalizeShortcut(`${event.ctrlKey ? 'Ctrl+' : ''}${event.altKey ? 'Alt+' : ''}${event.shiftKey ? 'Shift+' : ''}${event.metaKey ? 'Meta+' : ''}${key}`);
}

export function readShortcuts(): ShortcutMap {
  let stored: Record<string, unknown> = {};
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(storageKey) ?? '{}');
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) stored = parsed as Record<string, unknown>;
  } catch { /* Defaults remain usable. */ }
  const result: ShortcutMap = { ...shortcutDefaults };
  for (const [id, value] of Object.entries(stored)) {
    if (value === null) result[id] = null;
    else if (typeof value === 'string') result[id] = normalizeShortcut(value) ?? result[id] ?? null;
  }
  return result;
}

export function shortcutConflict(id: string, value: string, shortcuts: ShortcutMap): string | null {
  const normalized = normalizeShortcut(value);
  return Object.keys(shortcuts).find(other => other !== id && shortcuts[other] === normalized) ?? null;
}

function persist(shortcuts: ShortcutMap): boolean {
  try { localStorage.setItem(storageKey, JSON.stringify(shortcuts)); } catch { return false; }
  window.dispatchEvent(new Event(shortcutChangedEvent));
  return true;
}

export function replaceShortcuts(shortcuts: ShortcutMap): boolean {
  for (const [id, value] of Object.entries(shortcuts)) {
    if (value !== null && (!normalizeShortcut(value) || shortcutConflict(id, value, shortcuts))) return false;
  }
  return persist({ ...shortcuts });
}

export function saveShortcut(id: string, value: string): 'invalid' | 'conflict' | 'saved' {
  const normalized = normalizeShortcut(value);
  if (!normalized) return 'invalid';
  const shortcuts = readShortcuts();
  if (shortcutConflict(id, normalized, shortcuts)) return 'conflict';
  shortcuts[id] = normalized;
  return persist(shortcuts) ? 'saved' : 'invalid';
}

export function clearShortcut(id: string): void {
  const shortcuts = readShortcuts();
  shortcuts[id] = null;
  persist(shortcuts);
}

export function resetShortcuts(): void {
  try { localStorage.removeItem(storageKey); } catch { /* In-memory defaults still apply on next read. */ }
  window.dispatchEvent(new Event(shortcutChangedEvent));
}

export function shortcutLabel(id: string, shortcuts: ShortcutMap = readShortcuts()): string | null { return shortcuts[id] ?? null; }

export function editableTarget(target: EventTarget | null): boolean {
  return target instanceof Element && !!target.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"], [role="textbox"]');
}

export function shortcutAction(event: KeyboardEvent, shortcuts: ShortcutMap, options: { paletteOpen?: boolean; modalOpen?: boolean } = {}): string | null {
  if (event.defaultPrevented || event.isComposing || options.modalOpen) return null;
  const shortcut = shortcutFromEvent(event);
  if (!shortcut) return null;
  const id = Object.keys(shortcuts).find(candidate => shortcuts[candidate] === shortcut) ?? null;
  if (!id || (editableTarget(event.target) && id !== 'palette.open') || (options.paletteOpen && id !== 'palette.open')) return null;
  return id;
}

export function defaultShortcutIds(): ShortcutId[] { return [...ids]; }

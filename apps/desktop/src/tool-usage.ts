// SPDX-License-Identifier: MIT
/** Local UI preference only. Call recordToolOpen after a successful open, never on hover/install. */
export type ToolCommandIdentity = { pluginId: string; commandId: string };
type Entry = ToolCommandIdentity & { count: number; lastOpenedAt: number };
const storageKey = 'cobble.tools.usage.v1';
const maxEntries = 64;
const maxCount = 10_000;
const halfLifeMs = 14 * 24 * 60 * 60 * 1000;
const validId = (value: unknown): value is string => typeof value === 'string' && /^[a-zA-Z0-9][a-zA-Z0-9._:-]{0,159}$/.test(value);
const identity = (value: ToolCommandIdentity) => JSON.stringify([value.pluginId, value.commandId]);
const timestamp = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
function read(now: number): Entry[] {
  try {
    const raw = localStorage.getItem(storageKey);
    if (!raw || raw.length > 32_768) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return [];
    const data = parsed as Record<string, unknown>;
    if (data.schemaVersion !== 1 || !Array.isArray(data.entries) || data.entries.length > maxEntries * 4) return [];
    const unique = new Map<string, Entry>();
    for (const value of data.entries) {
      if (!value || typeof value !== 'object' || Array.isArray(value)) continue;
      const item = value as Record<string, unknown>;
      if (!validId(item.pluginId) || !validId(item.commandId) || !timestamp(item.lastOpenedAt) || item.lastOpenedAt > now + 300_000 || typeof item.count !== 'number' || !Number.isSafeInteger(item.count) || item.count < 1) continue;
      // Copy only IDs and counters; never persist labels, game/account context, or arbitrary fields.
      const entry: Entry = { pluginId: item.pluginId, commandId: item.commandId, count: Math.min(item.count, maxCount), lastOpenedAt: item.lastOpenedAt };
      const key = identity(entry), previous = unique.get(key);
      if (!previous || entry.lastOpenedAt > previous.lastOpenedAt || entry.lastOpenedAt === previous.lastOpenedAt && entry.count > previous.count) unique.set(key, entry);
    }
    return trim([...unique.values()]);
  } catch { return []; /* Preference failures never prevent opening or listing tools. */ }
}
function trim(entries: Entry[]): Entry[] {
  return entries.sort((a, b) => b.lastOpenedAt - a.lastOpenedAt || b.count - a.count || identity(a).localeCompare(identity(b), 'en')).slice(0, maxEntries);
}
/** The caller owns success detection. Identity deliberately excludes instance/profile/account data. */
export function recordToolOpen(command: ToolCommandIdentity, now = Date.now()): void {
  if (!validId(command.pluginId) || !validId(command.commandId) || !timestamp(now)) return;
  const entries = read(now), key = identity(command), previous = entries.find(entry => identity(entry) === key);
  const next: Entry = { pluginId: command.pluginId, commandId: command.commandId, count: Math.min(maxCount, (previous?.count ?? 0) + 1), lastOpenedAt: Math.max(now, previous?.lastOpenedAt ?? 0) };
  try { localStorage.setItem(storageKey, JSON.stringify({ schemaVersion: 1, entries: trim([...entries.filter(entry => identity(entry) !== key), next]) })); }
  catch { /* Local storage may be unavailable or full; the tool still opens. */ }
}
/** Rank only the caller's current eligible commands; history never creates or authorizes a tool. */
export function rankUsedTools<T extends ToolCommandIdentity>(eligible: readonly T[], now = Date.now()): T[] {
  const history = new Map(read(timestamp(now) ? now : Date.now()).map(entry => [identity(entry), entry]));
  const seen = new Set<string>();
  const currentTime = timestamp(now) ? now : Date.now();
  return eligible.flatMap((command, index) => {
    const key = identity(command);
    if (seen.has(key)) return [];
    seen.add(key);
    const entry = history.get(key);
    // Frequency is logarithmic and halves in weight every two weeks without another open.
    const score = entry ? Math.log2(1 + entry.count) * 2 ** (-Math.max(0, currentTime - entry.lastOpenedAt) / halfLifeMs) : 0;
    return [{ command, index, score }];
  }).sort((a, b) => b.score - a.score || a.index - b.index).map(item => item.command);
}

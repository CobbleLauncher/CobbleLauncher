// SPDX-License-Identifier: MIT
import type { ToolCommandIdentity } from './tool-usage';
const key = 'cobble.running-tool-pins.v1';
const valid = (value: unknown): value is string => typeof value === 'string' && /^[a-zA-Z0-9][a-zA-Z0-9._:-]{0,159}$/.test(value);
const validPin = (value: unknown): value is string => typeof value === 'string' && value.length <= 321 && /^[a-zA-Z0-9][a-zA-Z0-9._:-]*$/.test(value);
const identity = (command: ToolCommandIdentity) => `${command.pluginId}:${command.commandId}`;
export type RunningToolPins = Record<string, string[]>;
export function readRunningToolPins(): RunningToolPins {
  try {
    const raw = localStorage.getItem(key);
    if (!raw || raw.length > 32_768) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).slice(0, 1000).filter(([id]) => valid(id)).map(([id, value]) => [id, Array.isArray(value) ? [...new Set(value.filter(validPin))].slice(0, 8) : []]));
  } catch { return {}; }
}
export function toggleRunningToolPin(pins: RunningToolPins, gameId: string, command: ToolCommandIdentity): RunningToolPins {
  if (!valid(gameId) || !valid(command.pluginId) || !valid(command.commandId)) return pins;
  const id = identity(command), current = pins[gameId] ?? [];
  const next = current.includes(id) ? current.filter(value => value !== id) : [...current, id].slice(0, 8);
  const updated = { ...pins, [gameId]: next };
  try { localStorage.setItem(key, JSON.stringify(updated)); } catch { /* Current session still reflects the pin. */ }
  return updated;
}
export function rankRunningTools<T extends ToolCommandIdentity>(eligible: readonly T[], pinned: readonly string[]): T[] {
  return [...eligible].sort((a, b) => Number(pinned.includes(identity(b))) - Number(pinned.includes(identity(a))));
}
export function isRunningToolPinned(pins: RunningToolPins, gameId: string, command: ToolCommandIdentity) { return pins[gameId]?.includes(identity(command)) ?? false; }

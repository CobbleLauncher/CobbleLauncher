// SPDX-License-Identifier: MIT
export type PluginAccessEntry = {
  timestamp_ms: number;
  plugin_id: string;
  capability: string;
  action: string;
  outcome: 'allowed' | 'denied' | 'completed' | 'failed' | 'cancelled';
  target: string | null;
};
export function accessTarget(entry: PluginAccessEntry): string {
  return entry.target?.startsWith('game:') ? entry.target.slice(5) : entry.target ?? '';
}
export function accessTime(timestamp: number, locale: string): string {
  return new Date(timestamp).toLocaleString(locale);
}

// SPDX-License-Identifier: MIT
import type { RecoverySnapshot, RecoveryTarget } from '../../../../packages/plugin-web-sdk/src/recovery-workspace';
export type RecoveryEvent = { key: string; kind: 'mod' | 'config'; at: number; name: string; detail: string[]; status: string; target: RecoveryTarget | null };
export function recoveryEvents(snapshot: RecoverySnapshot, sinceBaseline: boolean, query: string, kind: 'all' | 'mod' | 'config', afterFailure: number | null = null): { items: RecoveryEvent[]; total: number; limited: boolean } {
  const after = sinceBaseline ? afterFailure ?? snapshot.lastCleanLaunch?.startedAt ?? null : null;
  const events: RecoveryEvent[] = [
    ...snapshot.mods.items.map(operation => ({ key: 'mod-' + operation.id, kind: 'mod' as const, at: operation.createdAt, name: operation.files.map(file => file.name).join(', '), detail: operation.files.map(file => `${file.before ?? '—'} → ${file.after ?? '—'}`), status: operation.status, target: operation.status === 'applied' ? { kind: 'modUndo' as const, operationId: operation.id } : null })),
    ...snapshot.configs.files.flatMap(file => file.backups.map(backup => ({ key: 'config-' + file.id + '-' + backup.id, kind: 'config' as const, at: backup.createdAt, name: file.name, detail: [] as string[], status: 'backup', target: { kind: 'configRestore' as const, configId: file.id, backupId: backup.id } }))),
  ];
  const needle = query.trim().toLocaleLowerCase();
  const valid = events.filter(event => Number.isSafeInteger(event.at) && event.at > 0 && event.at <= 8_640_000_000_000_000);
  const filtered = valid.filter(event => (after === null || event.at > after) && (kind === 'all' || event.kind === kind) && (!needle || [event.name, ...event.detail].join(' ').toLocaleLowerCase().includes(needle))).sort((a, b) => b.at - a.at || a.key.localeCompare(b.key));
  return { items: filtered.slice(0, 100), total: filtered.length, limited: filtered.length > 100 || valid.length !== events.length || snapshot.mods.truncated || snapshot.configs.truncated };
}

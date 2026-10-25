// SPDX-License-Identifier: MIT
import type { RecoveryComparison, RecoveryTarget } from '../../../../packages/plugin-web-sdk/src/recovery-workspace';

export type ComparisonField = 'minecraftVersion' | 'loaderKind' | 'loaderVersion' | 'javaMode' | 'javaMajor' | 'memoryMb' | 'jvmArgumentCount';
export type ComparisonChange = { field: ComparisonField; before: string | number | null; after: string | number | null };
export type ComparisonRecord = { kind: 'mod' | 'config'; at: number; name: string; detail: string[]; status: string; target: RecoveryTarget | null };

export function comparisonChanges(value: RecoveryComparison): ComparisonChange[] {
  if (!value.available) return [];
  const rows: ComparisonChange[] = [];
  const add = (field: ComparisonField, before: string | number | null, after: string | number | null) => {
    if (before !== after) rows.push({ field, before, after });
  };
  if (value.loader.changed === true && value.loader.before && value.loader.after) {
    add('minecraftVersion', value.loader.before.minecraftVersion, value.loader.after.minecraftVersion);
    add('loaderKind', value.loader.before.kind, value.loader.after.kind);
    add('loaderVersion', value.loader.before.version, value.loader.after.version);
  }
  if (value.runtime.changed === true && value.runtime.before && value.runtime.after) {
    add('javaMode', value.runtime.before.javaMode, value.runtime.after.javaMode);
    add('javaMajor', value.runtime.before.selectedJavaMajor, value.runtime.after.selectedJavaMajor);
    add('memoryMb', value.runtime.before.memoryMb, value.runtime.after.memoryMb);
    add('jvmArgumentCount', value.runtime.before.jvmArgumentCount, value.runtime.after.jvmArgumentCount);
  }
  return rows;
}

export function comparisonRecords(value: RecoveryComparison): ComparisonRecord[] {
  if (!value.available) return [];
  return [
    ...value.modOperations.map(operation => ({ kind: 'mod' as const, at: operation.createdAt, name: operation.files.map(file => file.name).join(', '), detail: operation.files.map(file => `${file.before ?? '—'} → ${file.after ?? '—'}`), status: operation.status, target: operation.status === 'applied' ? { kind: 'modUndo' as const, operationId: operation.id } : null })),
    ...value.configBackups.map(backup => ({ kind: 'config' as const, at: backup.createdAt, name: backup.name, detail: [] as string[], status: 'backup', target: { kind: 'configRestore' as const, configId: backup.configId, backupId: backup.backupId } })),
  ].sort((a, b) => a.at - b.at || a.name.localeCompare(b.name));
}

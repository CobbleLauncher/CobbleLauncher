// SPDX-License-Identifier: MIT
export type RecoveryRuntime = { javaMode: 'inherit' | 'automatic' | 'custom'; selectedJavaMajor: number | null; memoryMb: number; jvmArgumentCount: number };
export type RecoveryLoader = { minecraftVersion: string; kind: string | null; version: string | null };
export type RecoveryComparison = {
  available: boolean; reason: 'noBaseline' | 'baselineUnavailable' | 'noFailedAttempt' | 'failedAttemptUnavailable' | 'baselineAfterFailure' | null;
  baselineEndedAt: number | null; failureAt: number | null;
  runtime: { changed: boolean | null; before: RecoveryRuntime | null; after: RecoveryRuntime | null };
  loader: { changed: boolean | null; before: RecoveryLoader | null; after: RecoveryLoader | null };
  /** createdAt is the operation start; status is current at snapshot time, not proven at failure. */
  modOperations: { id: string; createdAt: number; status: string; statusAtFailure: 'unknown'; files: { name: string; before: string | null; after: string | null }[] }[];
  configBackups: { configId: string; name: string; backupId: string; createdAt: number }[];
  limited: boolean;
};
export type RecoveryTarget = { kind: 'modUndo'; operationId: string } | { kind: 'configRestore'; configId: string; backupId: string } | { kind: 'runtimeSettings' };
export type RecoverySnapshot = {
  schemaVersion: 2; gameId: string; revision: string; baselineAvailable: boolean;
  lastCleanLaunch: { startedAt: number; endedAt: number; runtime: RecoveryRuntime } | null;
  runtime: RecoveryRuntime & { changedSinceClean: boolean | null }; launchState: string;
  mods: { available: boolean; truncated: boolean; items: { id: string; createdAt: number; status: string; files: { name: string; before: string | null; after: string | null }[] }[] };
  configs: { available: boolean; truncated: boolean; files: { id: string; name: string; backups: { id: string; createdAt: number }[] }[] };
  comparison: RecoveryComparison;
};
type Transport = (request: { method: 'recovery.snapshot' | 'recovery.openRequest'; params: Record<string, unknown> }) => Promise<unknown>;
function id(value: string) { if (typeof value !== 'string' || !/^[a-zA-Z0-9_-]{1,128}$/u.test(value)) throw new Error('RECOVERY_TARGET_INVALID'); return value; }
function digest(value: string) { if (typeof value !== 'string' || !/^[a-fA-F0-9]{64}$/u.test(value)) throw new Error('RECOVERY_SNAPSHOT_CHANGED'); return value; }
function target(value: RecoveryTarget): RecoveryTarget {
  if (!value || typeof value !== 'object') throw new Error('RECOVERY_TARGET_INVALID');
  if (value.kind === 'runtimeSettings' && Object.keys(value).length === 1) return { kind: value.kind };
  if (value.kind === 'modUndo' && Object.keys(value).length === 2) return { kind: value.kind, operationId: id(value.operationId) };
  if (value.kind === 'configRestore' && Object.keys(value).length === 3) return { kind: value.kind, configId: digest(value.configId), backupId: id(value.backupId) };
  throw new Error('RECOVERY_TARGET_INVALID');
}
export function createRecoveryWorkspace(transport: Transport) {
  return Object.freeze({
    snapshot: (args: { gameId: string }) => transport({ method: 'recovery.snapshot', params: { gameId: id(args.gameId) } }) as Promise<RecoverySnapshot>,
    /** Opens an existing host preview/confirmation flow. It does not execute a rollback. */
    openRequest: (args: { gameId: string; revision: string; target: RecoveryTarget }) => transport({ method: 'recovery.openRequest', params: { gameId: id(args.gameId), revision: digest(args.revision), target: target(args.target) } }) as Promise<{ requested: true }>,
  });
}

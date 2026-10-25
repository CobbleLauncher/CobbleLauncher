// SPDX-License-Identifier: MIT
import type { AppPage } from './navigation';
export type ConfigRestoreTarget = { configId: string; backupId: string };
export type RecoveryDetailTarget = { kind: 'modUndo'; operationId: string } | { kind: 'runtimeSettings' };
export type RecoveryNavigationRequest = { game_id: string; plugin_id: string; target: RecoveryDetailTarget | ({ kind: 'configRestore' } & ConfigRestoreTarget) };
const opaque = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9_-]{1,128}$/u.test(value);
export function parseRecoveryNavigation(value: unknown): RecoveryNavigationRequest | null {
  if (!value || typeof value !== 'object') return null;
  const input = value as Record<string, unknown>, target = input.target as Record<string, unknown> | undefined;
  if (!opaque(input.game_id) || !opaque(input.plugin_id) || !target || typeof target !== 'object') return null;
  if (target.kind === 'runtimeSettings' && Object.keys(target).length === 1) return { game_id: input.game_id, plugin_id: input.plugin_id, target: { kind: target.kind } };
  if (target.kind === 'modUndo' && Object.keys(target).length === 2 && opaque(target.operationId)) return { game_id: input.game_id, plugin_id: input.plugin_id, target: { kind: target.kind, operationId: target.operationId } };
  if (target.kind === 'configRestore' && Object.keys(target).length === 3 && typeof target.configId === 'string' && /^[a-f0-9]{64}$/u.test(target.configId) && opaque(target.backupId)) return { game_id: input.game_id, plugin_id: input.plugin_id, target: { kind: target.kind, configId: target.configId, backupId: target.backupId } };
  return null;
}
/** The caller performs the normal workspace leave guard before applying this route. */
export function recoveryNavigationPage(request: RecoveryNavigationRequest): AppPage {
  const { target } = request;
  return target.kind === 'configRestore'
    ? { view: 'tools', location: { pluginId: 'config-doctor', instanceId: request.game_id, configRestoreTarget: { configId: target.configId, backupId: target.backupId } } }
    : { view: 'details', gameId: request.game_id, recoveryTarget: { ...target } };
}

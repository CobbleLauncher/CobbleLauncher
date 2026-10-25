// SPDX-License-Identifier: MIT
import { toolPage, type NavigationState } from './navigation';

export type PluginHandoffRequest = { handoffId: string; targetPluginId: string; gameId: string | null };

export function parsePluginHandoffRequest(value: unknown): PluginHandoffRequest | null {
  if (!value || typeof value !== 'object') return null;
  const item = value as Record<string, unknown>;
  if (typeof item.handoffId !== 'string' || !/^[a-f0-9]{32}$/u.test(item.handoffId)
    || typeof item.targetPluginId !== 'string' || !/^[a-z0-9][a-z0-9._-]{0,127}$/u.test(item.targetPluginId)
    || !(typeof item.gameId === 'string' && /^[a-zA-Z0-9._-]{1,128}$/u.test(item.gameId) || item.gameId === null && item.interfaceId === 'launcher:materials-import' && item.version === '0.1.0')) return null;
  return { handoffId: item.handoffId, targetPluginId: item.targetPluginId, gameId: item.gameId };
}

/** Build the exact receiver route only for a game already present in the host library. */
export function openPluginHandoff(state: NavigationState, request: PluginHandoffRequest, hasInstalledGame: (id: string) => boolean): NavigationState | null {
  if (request.gameId !== null && !hasInstalledGame(request.gameId)) return null;
  return toolPage(state, { pluginId: request.targetPluginId, instanceId: request.gameId ?? undefined, handoffId: request.handoffId });
}

// SPDX-License-Identifier: MIT
import type { JsonValue, Transport } from './index';

/** Launcher-owned private plugin data. These IDs never select filesystem paths. */
export type DataScope = { gameId?: string; worldId?: string; serverId?: string };
export type DataDocument = { value: JsonValue; revision: string | null };
export type DataTarget = { scope: DataScope; key: string };
export type DataWrite = DataTarget & { value: JsonValue; expectedRevision: string | null };
export type DataRemove = DataTarget & { expectedRevision: string | null };

function target(args: DataTarget): DataTarget {
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(args.key)) throw new Error('PLUGIN_DATA_INVALID_KEY');
  const { gameId, worldId, serverId } = args.scope;
  if (gameId !== undefined && (!gameId || gameId.length > 128 || /[\u0000-\u001f\u007f]/u.test(gameId))) throw new Error('PLUGIN_GAME_SCOPE');
  if (worldId !== undefined && (!gameId || !worldId || worldId.length > 128 || /[/\\\u0000-\u001f\u007f]/u.test(worldId) || ['.', '..'].includes(worldId))) throw new Error('PLUGIN_WORLD_SCOPE');
  if (serverId !== undefined && (!gameId || worldId !== undefined || !/^[a-f0-9]{64}$/.test(serverId))) throw new Error('PLUGIN_SERVER_SCOPE');
  return { scope: { ...(gameId === undefined ? {} : { gameId }), ...(worldId === undefined ? {} : { worldId }), ...(serverId === undefined ? {} : { serverId }) }, key: args.key };
}
function revision(value: string | null): string | null {
  if (value !== null && (!value || value.length > 128 || /[\u0000-\u001f\u007f]/u.test(value))) throw new Error('PLUGIN_DATA_INVALID_REVISION');
  return value;
}
export function createDataClient(transport: Transport) {
  return Object.freeze({
    read: (args: DataTarget) => transport({ method: 'data.read', params: target(args) }) as Promise<DataDocument>,
    /** Null revision creates a missing document. Stale revisions fail without overwriting. */
    write: (args: DataWrite) => transport({ method: 'data.write', params: { ...target(args), value: args.value, expectedRevision: revision(args.expectedRevision) } }) as Promise<DataDocument>,
    remove: (args: DataRemove) => transport({ method: 'data.remove', params: { ...target(args), expectedRevision: revision(args.expectedRevision) } }) as Promise<null>,
  });
}

// SPDX-License-Identifier: MIT
import type { InspectionSnapshot } from './index';
/** Host-listed saved endpoint identity; never a network address or list index. */
export type ServerContext = { serverId: string; name: string };
export type ServerContexts = { available: boolean; truncated: boolean; items: ServerContext[] };
type InspectionClient = { inspection: { snapshot(gameId: string, scope: 'servers'): Promise<InspectionSnapshot> } };
/** Call after explicit game selection; callers must explicitly select a returned server. */
export async function listServerContexts(client: InspectionClient, gameId: string): Promise<ServerContexts> {
  const snapshot = await client.inspection.snapshot(gameId, 'servers');
  if (snapshot.scope !== 'servers' || snapshot.game.id !== gameId || !snapshot.data || Array.isArray(snapshot.data) || typeof snapshot.data !== 'object') throw new Error('PLUGIN_SERVER_LIST_INVALID');
  const data = snapshot.data;
  if (typeof data.available !== 'boolean' || typeof data.truncated !== 'boolean' || !Array.isArray(data.items)) throw new Error('PLUGIN_SERVER_LIST_INVALID');
  const items: ServerContext[] = [];
  const seen = new Set<string>();
  for (const item of data.items) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) continue;
    if (typeof item.serverId !== 'string' || !/^[a-f0-9]{64}$/.test(item.serverId) || typeof item.name !== 'string' || seen.has(item.serverId)) continue;
    seen.add(item.serverId);
    items.push({ serverId: item.serverId, name: item.name });
  }
  return { available: data.available, truncated: data.truncated, items };
}

// SPDX-License-Identifier: MIT
import type { MeshOverview } from './mesh';
import type { MeshAppSnapshot, MeshMessage } from './mesh-social';
export type MeshEventMetadata = { sequence: number; direction: 'incoming' | 'outgoing'; at: number; bytes: number; contact: number };
export function messageMetadata(messages: MeshMessage[], contacts: Map<string, number>): MeshEventMetadata[] {
  return messages.slice(0, 40).map(message => {
    if (!contacts.has(message.peer_id)) contacts.set(message.peer_id, contacts.size + 1);
    return { sequence: message.sequence, direction: message.direction, at: message.at, bytes: new TextEncoder().encode(message.text).byteLength, contact: contacts.get(message.peer_id)! };
  });
}
export function meshDiagnostics(overview: MeshOverview | null, app: MeshAppSnapshot | null, events: MeshEventMetadata[], readAt: number | null) {
  const session = overview?.session;
  return {
    schemaVersion: 1, readAt,
    local: { component: overview?.component_state ?? 'unknown', contactsService: app ? 'running' : 'inactive', transportNeighbors: overview?.neighbors.length ?? 0, connectedContacts: app?.contacts.filter(contact => contact.connected).length ?? 0 },
    discovery: { state: overview?.discovery_state ?? 'unknown', signedAdvertisements: overview?.nodes.length ?? 0, reportedTotal: overview?.discovery_total ?? 0 },
    capabilities: overview?.capabilities ?? null,
    services: (overview?.configured_services ?? []).map(service => ({ kind: service.kind, state: service.state, discovery: service.discovery, gameRelay: service.game_relay })),
    session: session ? { role: session.role, state: session.state, route: session.route, target: session.target, localPort: session.local_port, expiresAt: session.expires_at } : null,
    messageEvents: events.slice(-160).map(event => ({ sequence: event.sequence, direction: event.direction, at: event.at, bytes: event.bytes, contact: event.contact })),
  };
}

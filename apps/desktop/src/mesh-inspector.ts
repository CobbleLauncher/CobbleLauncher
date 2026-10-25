// SPDX-License-Identifier: MIT
import type { MeshNetworkSnapshot, MeshOverview, MeshNodeSnapshot } from './mesh';
import type { MeshAppSnapshot, MeshMessage } from './mesh-social';
export type MeshEventMetadata = { sequence: number; direction: 'incoming' | 'outgoing'; at: number; bytes: number; contact: number };
export function messageMetadata(messages: MeshMessage[], contacts: Map<string, number>): MeshEventMetadata[] {
  return messages.slice(0, 40).map(message => {
    if (!contacts.has(message.peer_id)) contacts.set(message.peer_id, contacts.size + 1);
    return { sequence: message.sequence, direction: message.direction, at: message.at, bytes: new TextEncoder().encode(message.text).byteLength, contact: contacts.get(message.peer_id)! };
  });
}
export function meshDiagnostics(overview: MeshOverview | null, app: MeshAppSnapshot | null, events: MeshEventMetadata[], readAt: number | null, network: MeshNetworkSnapshot | null = null, node: MeshNodeSnapshot | null = null) {
  const session = overview?.session;
  return {
    schemaVersion: 1, readAt,
    routePolicy: { authorization:overview?.relay.route_policy?.authorization ?? 'coordination', hasObserver:!!overview?.relay.route_policy?.observer },
    localRelay: node ? { state:node.state, access:node.access, allowedPeerCount:node.allowed_peer_count, limits:node.limits, transportBytes:node.transport_bytes, connections:node.connections, expiresAt:node.expires_at } : null,
    local: { component: overview?.component_state ?? 'unknown', contactsService: app ? 'running' : 'inactive', transportNeighbors: overview?.neighbors.length ?? 0, connectedContacts: app?.contacts.filter(contact => contact.connected).length ?? 0 },
    discovery: { state: overview?.discovery_state ?? 'unknown', signedAdvertisements: overview?.nodes.filter(node => node.expires_at > (readAt ?? Date.now() / 1000)).length ?? 0, reportedTotal: overview?.discovery_total ?? 0 },
    network: network ? { phase: network.phase, generation: network.generation, attempt: network.attempt, retryAt: network.retry_at, relayNegotiated: network.phase === 'online' && network.relay_available, actualRoute: network.session_evidence?.selected_route ?? null, streamBytes: network.session_evidence ? { sent: network.session_evidence.bytes_sent, received: network.session_evidence.bytes_received, active: network.session_evidence.active_streams, completed: network.session_evidence.completed_streams } : null, checks: (network.checks ?? []).map(check => ({ capability: check.capability, supported: check.supported, state: check.state, observedAt: check.observed_at, stage: check.stage, peerScope: check.peer_scope === 'local' || check.peer_scope === 'global' ? check.peer_scope : 'peer', errorCode: check.error_code })) } : null,
    services: (network?.services ?? []).map(service => ({ kind: service.kind, state: service.state, discovery: service.discovery, gameRelay: service.game_relay })),
    session: session ? { role: session.role, state: session.state, target: session.target, localPort: session.local_port, expiresAt: session.expires_at } : null,
    messageEvents: events.slice(-160).map(event => ({ sequence: event.sequence, direction: event.direction, at: event.at, bytes: event.bytes, contact: event.contact })),
  };
}

// SPDX-License-Identifier: MIT
export type MeshPublicProfile = { peer_id: string; display_name: string; suffix: string; revision: number };
export type MeshContact = { profile: MeshPublicProfile; connected: boolean; last_seen: number };
export type MeshAppSnapshot = {
  profile: MeshPublicProfile;
  contacts: MeshContact[];
  capabilities: { contact_invites: boolean; direct_messages: boolean; lan_only: boolean; relay_messages: boolean; groups: boolean; vlan: boolean; events_public?: boolean };
  public_events_paused?: boolean;
  event_route?: {
    authorization: 'coordination' | 'community' | 'private' | 'legacy_test';
    relay_peer_id: string;
    state: 'starting' | 'ready' | 'failed';
    game_coexistence: false;
    last_error?: 'event_route_failed' | 'event_direct_required' | null;
    last_delivery?: { message_id: string; peer_id: string; connection_id: string; route: 'direct' | 'relay' | 'lan' } | null;
  } | null;
};
export type MeshMessage = { sequence: number; id: string; peer_id: string; direction: 'incoming' | 'outgoing'; text: string; at: number };
export type MeshMessagePage = { messages: MeshMessage[]; next_after: number; oldest_sequence: number | null; has_more: boolean; generation: string };
export function meshProfileLabel(profile: MeshPublicProfile) { return `${profile.display_name}#${profile.suffix}`; }
export const MAX_MESH_MESSAGE_BYTES = 1024;
export function meshMessageBytes(text: string) { return new TextEncoder().encode(text).byteLength; }
// SPDX-License-Identifier: MIT
export type MeshContactInvitation = { invitation: string; expires_at: number };
export type MeshMessageReceipt = { id: string; received: boolean };
export type MeshMessageComposer = { plugin_id: string; name: string };

export type MeshIdentityState = { profile: MeshPublicProfile | null; mode: 'persistent' | 'temporary'; active: boolean };
export function meshIdentityCard(profile: MeshPublicProfile): string {
  return JSON.stringify({ type: 'cobble-mesh-identity', version: 1, peerId: profile.peer_id, displayName: profile.display_name }, null, 2);
}
export async function meshIdentityDigest(peerId: string): Promise<string> {
  const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(peerId));
  return [...new Uint8Array(bytes)].map(value => value.toString(16).padStart(2, '0')).join('');
}
export function meshIdentityTiles(digest: string): { x: number; y: number; triangle: boolean }[] {
  if (!/^[a-f0-9]{64}$/i.test(digest)) return [];
  return Array.from({ length: 36 }, (_, index) => ({ x: index % 6, y: Math.floor(index / 6), value: parseInt(digest[(index * 7) % digest.length]!, 16) })).filter(tile => tile.value > 5).map(tile => ({ x: tile.x, y: tile.y, triangle: tile.value % 2 === 0 }));
}

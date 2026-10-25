// SPDX-License-Identifier: MIT
export type MeshGroupMember = { peer_id: string; state: 'connected'; is_host: boolean };
export type MeshGroupSnapshot = {
  group_id: number;
  game_id: string;
  game_name: string;
  role: 'host' | 'guest';
  state: 'hosting' | 'bridge_ready' | 'ended' | 'failed';
  host_peer_id: string;
  capacity: number;
  members: MeshGroupMember[];
  membership_complete: boolean;
  invitation_expires_at: number | null;
  admission_open: boolean;
  expires_at: number;
  route: 'unknown' | 'direct' | 'relay' | 'explicit_relay';
  bridge_address?: string | null;
};
export type MeshGroupEnvelope = { generation: number; group: MeshGroupSnapshot | null };
export type MeshGroupCreateResult = MeshGroupEnvelope & { invitation: string };
export type MeshGroupJoinResult = MeshGroupEnvelope & { bridge_address: string };
export function validGroupPort(port: unknown): port is number { return typeof port === 'number' && Number.isInteger(port) && port >= 1024 && port <= 65535; }
export function groupActive(group: MeshGroupSnapshot | null): boolean { return group?.state === 'hosting' || group?.state === 'bridge_ready'; }
/** Guest snapshots contain only connections this device actually observes. */
export function groupFreeSlots(group: MeshGroupSnapshot): number | null {
  return groupActive(group) && group.role === 'host' && group.membership_complete ? Math.max(0, group.capacity - group.members.length) : null;
}

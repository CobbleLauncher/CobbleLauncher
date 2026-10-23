// SPDX-License-Identifier: MIT
export type MeshAvailability = { available: boolean; reason: string | null; can_host: boolean };
export type MeshRelay = { address: string | null; source: 'last_successful' | null; state: 'unconfigured' | 'configured' };
export type MeshConfiguredService = { id: string; kind: 'bootstrap' | 'relay'; endpoint: string; game_relay: boolean; state: 'configured' | 'authenticated' | 'unavailable'; discovery: 'available' | 'unavailable' | 'not_checked' };
export type MeshOverview = MeshAvailability & {
  component_state: 'missing' | 'idle' | 'running'; mode: 'explicit_relay'; discovery_state: 'inactive' | 'published' | 'unavailable';
  discovery_source: string | null; discovery_total: number;
  nodes: { peer_id: string; endpoint: string; relay: boolean; discovery: boolean; expires_at: number; evidence: 'signed_advertisement' }[];
  configured_services: MeshConfiguredService[];
  neighbors: never[]; capabilities: { relay: boolean; direct: boolean; hole_punch: boolean };
  relay: MeshRelay; session: MeshSession | null;
};
export type MeshHostOptions = { game_id: string; lan: { state: 'detected' | 'not_open' | 'unverified'; port: number | null }; relay: MeshRelay };
export type MeshSession = {
  session_id: number;
  game_id: string;
  target: 'game' | 'server';
  local_port: number | null;
  role: 'host' | 'guest';
  state: 'listening' | 'bridge_ready' | 'completed' | 'failed' | 'ended';
  route: 'explicit_relay';
  scope: 'public';
  expires_at: number;
};
export type MeshHostResult = { session: MeshSession; invitation: string };
export type MeshJoinResult = { session: MeshSession; bridge_address: string };
export function meshSessionActive(session: MeshSession | null): boolean {
  return session?.state === 'listening' || session?.state === 'bridge_ready';
}

// SPDX-License-Identifier: MIT
export type MeshAvailability = { available: boolean; reason: string | null; can_host: boolean };
export type MeshNetworkPhase = 'offline' | 'connecting' | 'online' | 'reconnecting' | 'failed';
export type MeshNetworkCheck = {
  capability: 'relay' | 'direct' | 'hole_punch';
  supported: boolean;
  state: 'not_tested' | 'testing' | 'verified' | 'failed' | 'unsupported';
  observed_at: number | null;
  stage: string;
  peer_scope: string;
  error_code: string | null;
};
export type MeshSessionEvidence = {
  peer_scope: string;
  selected_route: 'direct' | 'relay' | 'unknown';
  selected_connection: string | null;
  bytes_sent: number;
  bytes_received: number;
  completed_streams: number;
  active_streams: number;
};
export type MeshNetworkSnapshot = {
  enabled: boolean;
  phase: MeshNetworkPhase;
  generation: number;
  attempt: number;
  retry_at: number | null;
  profile: { peer_id: string; display_name: string; suffix: string; revision: number } | null;
  services: MeshConfiguredService[];
  last_error: string | null;
  relay_available: boolean;
  session_route: string | null;
  checks: MeshNetworkCheck[];
  session_evidence?: MeshSessionEvidence | null;
};
export function newerNetworkSnapshot(current: MeshNetworkSnapshot | null, incoming: MeshNetworkSnapshot): MeshNetworkSnapshot {
  return current && current.generation > incoming.generation ? current : incoming;
}
export type MeshRoutePolicy = { authorization: 'coordination' | 'community' | 'private' | 'legacy_test'; observer: string | null };
export const defaultMeshRoutePolicy = (): MeshRoutePolicy => ({ authorization: 'coordination', observer: null });
export type MeshRelay = { address: string | null; source: 'last_successful' | null; state: 'unconfigured' | 'configured'; route_policy?: MeshRoutePolicy };
export type MeshNodeLimits = { bandwidth_bytes_per_second: number; max_bytes: number; max_connections: number; duration_seconds: number };
export type MeshNodeSnapshot = { state: 'running' | 'stopped' | 'expired' | 'exhausted' | 'failed'; endpoint: string; peer_id: string; access: 'community' | 'private'; allowed_peer_count: number; limits: MeshNodeLimits; transport_bytes: number; connections: number; expires_at: number; last_error: string | null };
export type MeshConfiguredService = { id: string; kind: 'bootstrap' | 'relay'; endpoint: string; game_relay: boolean; state: 'configured' | 'authenticated' | 'unavailable'; discovery: 'available' | 'unavailable' | 'not_checked' };
export type MeshOverview = MeshAvailability & {
  component_state: 'missing' | 'idle' | 'running'; mode: 'automatic' | 'explicit_relay'; discovery_state: 'inactive' | 'published' | 'unavailable';
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
  route: 'unknown' | 'direct' | 'relay' | 'explicit_relay';
  scope: 'public';
  expires_at: number;
};
export type MeshHostResult = { session: MeshSession; invitation: string };
export type MeshJoinResult = { session: MeshSession; bridge_address: string };
export function meshSessionActive(session: MeshSession | null): boolean {
  return session?.state === 'listening' || session?.state === 'bridge_ready';
}

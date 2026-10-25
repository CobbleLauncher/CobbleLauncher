// SPDX-License-Identifier: MIT
// Cached synthetic native snapshots only; no Mesh companion, game process, or public request.
import { expect, test, type Page } from '@playwright/test';
import type { Game } from '../src/games';
import type { MeshNetworkSnapshot, MeshOverview } from '../src/mesh';
import type { MeshAppSnapshot } from '../src/mesh-social';

type Call = { command: string; args: Record<string, unknown> };
type FixtureWindow = Window & { __meshNetworkCalls: Call[]; __emitMeshNetwork: (snapshot: MeshNetworkSnapshot, persist?: boolean) => void };
const game: Game = { id: 'network-game', name: 'Copper Valley', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'idle' };
const profile = { peer_id: 'SYNTHETIC_FULL_MESH_ID', display_name: 'Aster', suffix: '7D42', revision: 1 };
const service = { id: 'official-relay', kind: 'relay' as const, endpoint: '/dns4/relay.example.invalid/tcp/443/p2p/SYNTHETIC_PEER', state: 'authenticated' as const, discovery: 'not_checked' as const, game_relay: false };
const overview: MeshOverview = { available: true, reason: null, can_host: false, component_state: 'running', mode: 'explicit_relay', discovery_state: 'published', discovery_source: null, discovery_total: 2, configured_services: [service], nodes: [{ peer_id: 'SYNTHETIC_PEER', endpoint: service.endpoint, relay: true, discovery: true, expires_at: 2000000000, evidence: 'signed_advertisement' }, { peer_id: 'EXPIRED_PEER', endpoint: '/dns4/expired.invalid/tcp/443/p2p/EXPIRED_PEER', relay: true, discovery: true, expires_at: 1, evidence: 'signed_advertisement' }], neighbors: [], capabilities: { relay: true, direct: true, hole_punch: true }, relay: { address: null, source: null, state: 'unconfigured' }, session: { session_id: 1, game_id: game.id, target: 'game', local_port: 25565, role: 'host', state: 'listening', route: 'explicit_relay', scope: 'public', expires_at: 2000000000 } };
const app: MeshAppSnapshot = { profile, contacts: [], capabilities: { contact_invites: false, direct_messages: false, lan_only: true, relay_messages: false, groups: false, vlan: false } };
const baseline: MeshNetworkSnapshot = { enabled: true, phase: 'online', generation: 1, attempt: 1, retry_at: null, profile, services: [service], last_error: null, relay_available: true, session_route: null, checks: [{ capability: 'relay', supported: true, state: 'verified', observed_at: Math.floor(Date.now() / 1000) - 5, stage: 'hop_protocol', peer_scope: 'official_seed:SYNTHETIC_PEER', error_code: null }, { capability: 'direct', supported: true, state: 'not_tested', observed_at: null, stage: 'none', peer_scope: 'none', error_code: null }, { capability: 'hole_punch', supported: true, state: 'not_tested', observed_at: null, stage: 'none', peer_scope: 'none', error_code: null }] };

async function mockNative(page: Page, language: string, theme: string) {
  await page.route('https://**/*', route => route.abort());
  await page.addInitScript(({ language, theme, game, overview, app, initial }) => {
    const target = window as unknown as FixtureWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true;
    target.__meshNetworkCalls = [];
    localStorage.setItem('cobble-language', language);
    localStorage.setItem('cobble.appearance', theme);
    const callbacks = new Map<number, (event: unknown) => void>();
    const listeners = new Map<string, number[]>();
    let nextCallback = 0;
    let network = initial;
    target.__emitMeshNetwork = (value, persist = true) => {
      if (persist) network = value;
      for (const id of listeners.get('mesh-network-changed') ?? []) callbacks.get(id)?.({ event: 'mesh-network-changed', id: 1, payload: value });
    };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = ++nextCallback; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__meshNetworkCalls.push({ command, args });
        if (command === 'plugin:event|listen') { const key = String(args.event); listeners.set(key, [...(listeners.get(key) ?? []), args.handler as number]); return 1; }
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [game];
        if (command === 'list_tasks') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'mesh_network_snapshot' || command === 'mesh_network_start') return network;
        if (command === 'mesh_network_retry') { network = { ...network, phase: 'reconnecting', generation: network.generation + 1, attempt: network.attempt + 1 }; return network; }
        if (command === 'mesh_network_set_enabled') { network = { ...network, enabled: Boolean(args.enabled), phase: args.enabled ? 'connecting' : 'offline', generation: network.generation + 1 }; return network; }
        if (command === 'mesh_overview') return overview;
        if (command === 'mesh_app_snapshot') return app;
        if (command === 'mesh_identity_state') return { profile: app.profile, mode: 'persistent', active: true };
        if (command === 'mesh_list_messages') return { messages: [], next_after: 0, oldest_sequence: null, has_more: false, generation: 'fixture' };
        if (command === 'mesh_status') return { session: overview.session };
        if (command.startsWith('list_')) return [];
        if (command === 'start_game') throw new Error('Synthetic fixture must not launch Minecraft');
        return null;
      },
    };
  }, { language, theme, game, overview, app, initial: baseline });
}

async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as FixtureWindow).__meshNetworkCalls.filter(call => call.command === name), command);
}

for (const { language, theme, width } of [{ language: 'en-US', theme: 'light', width: 1100 }, { language: 'zh-CN', theme: 'dark', width: 560 }]) {
  const en = language === 'en-US';
  test(`native Mesh state and evidence in ${language} ${theme} ${width}px`, async ({ page }, info) => {
    await mockNative(page, language, theme);
    await page.setViewportSize({ width, height: 850 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/');
    const header = page.locator('.mesh-status-button');
    await expect(header).toHaveText(en ? 'Online' : '在线');
    await page.screenshot({ path: info.outputPath('online-header.png'), fullPage: true, animations: 'disabled' });
    await page.evaluate(value => (window as unknown as FixtureWindow).__emitMeshNetwork(value), { ...baseline, phase: 'connecting', generation: 2 });
    await expect(header).toHaveText(en ? 'Connecting' : '连接中');
    await page.evaluate(value => (window as unknown as FixtureWindow).__emitMeshNetwork(value), { ...baseline, generation: 3 });
    await expect(header).toHaveText(en ? 'Online' : '在线');
    await page.evaluate(value => (window as unknown as FixtureWindow).__emitMeshNetwork(value, false), { ...baseline, phase: 'offline', enabled: false, generation: 1 });
    await expect(header).toHaveText(en ? 'Online' : '在线');
    await header.click();
    await page.getByRole('menuitem', { name: en ? 'Multiplayer & contacts' : '联机与联系人' }).click();
    await page.getByRole('button', { name: en ? 'Mesh Inspector' : 'Mesh 检查器' }).click();
    const inspector = page.locator('.mesh-inspector');
    await expect(inspector).toContainText(en ? 'HOP negotiated' : 'HOP 已协商');
    await expect(inspector).toContainText(en ? 'Verified' : '已验证');
    await expect(inspector).toContainText(en ? 'Not tested' : '未测试');
    await expect(inspector).toContainText(en ? 'No route confirmed' : '尚无已确认路径');
    await expect(inspector).not.toContainText('EXPIRED_PEER');
    await page.screenshot({ path: info.outputPath('inspector-evidence.png'), fullPage: true, animations: 'disabled' });

    const activeSession: MeshNetworkSnapshot = { ...baseline, generation: 3, session_evidence: { peer_scope: 'peer:SYNTHETIC_CONTACT', selected_route: 'direct', selected_connection: '/ip4/192.0.2.1/tcp/24111', bytes_sent: 128, bytes_received: 64, completed_streams: 1, active_streams: 1 }, checks: [baseline.checks[0]!, { capability: 'direct', supported: true, state: 'verified', observed_at: Math.floor(Date.now() / 1000), stage: 'game_stream', peer_scope: 'peer:SYNTHETIC_CONTACT', error_code: null }, baseline.checks[2]!] };
    await page.evaluate(value => (window as unknown as FixtureWindow).__emitMeshNetwork(value), activeSession);
    await expect(inspector).toContainText(en ? 'Actual routeDirect' : '实际路径直连');
    await expect(inspector).toContainText(en ? '128 B sent' : '128 B 已发送');
    await page.screenshot({ path: info.outputPath('session-evidence.png'), fullPage: true, animations: 'disabled' });

    const failed: MeshNetworkSnapshot = { ...baseline, phase: 'failed', generation: 4, relay_available: false, last_error: 'SEED_AUTH_FAILED', checks: [{ capability: 'relay', supported: true, state: 'failed', observed_at: Math.floor(Date.now() / 1000), stage: 'official_seed', peer_scope: 'official_seed:SYNTHETIC_PEER', error_code: 'SEED_AUTH_FAILED' }, ...baseline.checks.slice(1)] };
    await page.evaluate(value => (window as unknown as FixtureWindow).__emitMeshNetwork(value), failed);
    await expect(header).toHaveText(en ? 'Connection failed' : '连接失败');
    await inspector.getByRole('button', { name: en ? 'Refresh' : '刷新' }).click();
    await expect(inspector).toContainText('SEED_AUTH_FAILED');
    await page.screenshot({ path: info.outputPath('failed-network.png'), fullPage: true, animations: 'disabled' });
    await header.click();
    await page.getByRole('menuitem', { name: en ? 'Retry connection' : '重试连接' }).click();
    expect(await calls(page, 'mesh_network_retry')).toHaveLength(1);
    await expect(header).toHaveText(en ? 'Reconnecting' : '重新连接中');
    await page.screenshot({ path: info.outputPath('reconnecting.png'), fullPage: true, animations: 'disabled' });
    await page.evaluate(value => (window as unknown as FixtureWindow).__emitMeshNetwork(value), { ...baseline, enabled: false, phase: 'offline', generation: 6, relay_available: false });
    await expect(header).toHaveText(en ? 'Offline' : '离线');
    await page.screenshot({ path: info.outputPath('offline.png'), fullPage: true, animations: 'disabled' });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(await calls(page, 'mesh_discover')).toHaveLength(0);
    expect(await calls(page, 'start_game')).toHaveLength(0);
  });
}

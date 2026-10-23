// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only: no real invitation, relay, game or companion.
import { expect, test, type Page } from '@playwright/test';
import type { Game } from '../src/games';
import type { MeshSession } from '../src/mesh';

type Call = { command: string; args: Record<string, unknown> };
type MeshWindow = Window & { __meshCalls: Call[]; __meshScenario: (scenario: 'disconnect' | 'expiredHost') => void };
const game: Game = { id: 'mesh-browser-game', name: '联机测试', version: '1.21.1', directory: 'synthetic', status: 'installed', error: null, progress: null, launch_state: 'running' };
async function mockMesh(page: Page) {
  await page.addInitScript(initialGame => {
    const target = window as unknown as MeshWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__meshCalls = [];
    localStorage.setItem('cobble-language', 'zh-CN');
    const callbacks = new Map<number, (event: unknown) => void>();
    let session: MeshSession | null = null;
    let nextSessionId = 1;
    let discoveryPublished = false;
    let expiredHost = false;
    target.__meshScenario = scenario => {
      if (scenario === 'disconnect' && session) session = { ...session, state: 'failed' };
      if (scenario === 'expiredHost') expiredHost = true;
    };
    const createSession = (role: 'host' | 'guest'): MeshSession => ({ session_id: nextSessionId++, target: 'game', local_port: null, game_id: initialGame.id, role, state: role === 'host' ? 'listening' : 'bridge_ready', route: 'explicit_relay', scope: 'public', expires_at: 2000000000 });
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__meshCalls.push({ command, args });
        if (command === 'plugin:event|listen') return 1;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [initialGame];
        if (command === 'list_tasks' || command === 'list_game_mods' || command === 'list_game_packs') return [];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'mesh-player', kind: 'offline', name: 'Player', uuid: 'synthetic-player', note: null }], active_id: 'mesh-player' };
        if (command === 'mesh_discover') discoveryPublished = true;
        if (command === 'mesh_overview' || command === 'mesh_discover') return { available: true, reason: null, can_host: true, component_state: session ? 'running' : 'idle', mode: 'explicit_relay', discovery_state: discoveryPublished ? 'published' : 'inactive', discovery_source: null, discovery_total: 1, configured_services: discoveryPublished ? [{id:'official-relay',kind:'relay',endpoint:'/dns4/relay.example.invalid/tcp/443/tls/x-parity-ws/%2F_cobble%2Fmesh-ws/p2p/SYNTHETIC_PEER',state:'authenticated',discovery:'unavailable',game_relay:false},{id:'official-bootstrap',kind:'bootstrap',endpoint:'https://directory.example.invalid/v1/bootstrap',state:'unavailable',discovery:'unavailable',game_relay:false}] : [], nodes: discoveryPublished ? [{ peer_id: 'SYNTHETIC_PEER', endpoint: '/dns4/relay.example.invalid/tcp/443/tls/x-parity-ws/%2F_cobble%2Fmesh-ws/p2p/SYNTHETIC_PEER', relay: true, discovery: true, expires_at: 2000000000, evidence: 'signed_advertisement' }] : [], neighbors: [], capabilities: { relay: true, direct: false, hole_punch: false }, relay: { address: null, source: null, state: 'unconfigured' }, session };
        if (command === 'mesh_host_options') return { game_id: initialGame.id, lan: { state: 'detected', port: 25565 }, relay: { address: null, source: null, state: 'unconfigured' } };
        if (command === 'mesh_app_snapshot') return { profile: {peer_id:'PRIVATE_FIXTURE_ID',display_name:'Private Fixture',suffix:'ABCD',revision:1},contacts:[],capabilities:{contact_invites:true,direct_messages:true,lan_only:true,relay_messages:false,groups:false,vlan:false} };
        if (command === 'mesh_list_messages') return {messages:[{sequence:1,id:'PRIVATE_MESSAGE_ID',peer_id:'PRIVATE_CONTACT',direction:'incoming',text:'PRIVATE MESSAGE BODY',at:1800000000}],next_after:1,oldest_sequence:1,has_more:false,generation:'synthetic'};
        if (command === 'mesh_host') { session = createSession('host'); if (expiredHost) session.expires_at = 1; return { session, invitation: 'SYNTHETIC-MESH-HOST-INVITATION-NOT-VALID' }; }
        if (command === 'mesh_host_server') { session = { ...createSession('host'), target: 'server', game_id: '', local_port: Number(args.localPort) }; return { session, invitation: 'SYNTHETIC-MESH-SERVER-INVITATION-NOT-VALID' }; }
        if (command === 'mesh_join') { if (args.invitation === 'SYNTHETIC-EXPIRED-INVITATION') throw '邀请无效或已过期'; session = createSession('guest'); return { session, bridge_address: '127.0.0.1:40123' }; }
        if (command === 'mesh_status') return { session };
        if (command === 'mesh_end') { session = session ? { ...session, state: 'ended' } : null; return { session }; }
        if (command === 'start_game') throw new Error('Browser fixture must not start a game');
        return null;
      },
    };
  }, game);
}
async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as MeshWindow).__meshCalls.filter(call => call.command === name), command);
}

test('synthetic Mesh guided host, server and join render at wide and narrow widths', async ({ page }, testInfo) => {
  await mockMesh(page);
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  expect(await calls(page, 'mesh_overview')).toHaveLength(0);
  await page.getByRole('button', { name: '管理 联机测试', exact: true }).click();
  await page.getByRole('button', { name: 'Mesh 联机', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Mesh 联机', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '和朋友一起玩', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByText('进入世界，按 Esc → 对局域网开放 → 创建局域网世界。', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '创建邀请', exact: true })).toBeEnabled();
  expect(await calls(page, 'mesh_discover')).toHaveLength(1);
  await expect(page.getByLabel('公共中转地址', { exact: true })).not.toBeVisible();
  const capture = async (name: string) => {
    const path = testInfo.outputPath(`${name}.png`);
    await page.screenshot({ path, fullPage: true });
    await testInfo.attach(name, { path, contentType: 'image/png' });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  };
  await capture('synthetic-mesh-host-1280');
  await page.setViewportSize({ width: 560, height: 800 });
  await capture('synthetic-mesh-host-560');
  const relay = '/dns4/relay.example.invalid/tcp/443/tls/x-parity-ws/%2F_cobble%2Fmesh-ws/p2p/SYNTHETIC_PEER';
  await page.getByRole('button', { name: '创建邀请', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^等待玩家加入$/ })).toBeVisible();
  await capture('synthetic-mesh-waiting-560');
  await page.setViewportSize({ width: 1280, height: 800 });
  await capture('synthetic-mesh-waiting-1280');
  await page.setViewportSize({ width: 560, height: 800 });
  expect((await calls(page, 'mesh_host'))[0]?.args).toEqual({ gameId: game.id, localPort: 25565, relay, approveLocalPort: true });
  await page.getByRole('button', { name: '复制邀请', exact: true }).click();
  expect((await calls(page, 'plugin:clipboard-manager|write_text'))[0]?.args).toEqual({ text: 'SYNTHETIC-MESH-HOST-INVITATION-NOT-VALID' });
  await page.getByRole('button', { name: '返回联机测试', exact: true }).click();
  await page.getByRole('button', { name: 'Mesh 联机', exact: true }).click();
  await expect(page.getByLabel('私密邀请', { exact: true })).toHaveValue('SYNTHETIC-MESH-HOST-INVITATION-NOT-VALID');
  await page.getByRole('button', { name: '结束会话', exact: true }).click();
  await page.getByRole('button', { name: '分享服务器', exact: true }).click();
  await expect(page.getByLabel('服务器端口', { exact: true })).toHaveValue('25565');
  await page.getByLabel('服务器端口', { exact: true }).fill('25566');
  await capture('synthetic-mesh-server-560');
  await page.getByRole('button', { name: '创建邀请', exact: true }).click();
  await expect(page.getByLabel('私密邀请', { exact: true })).toHaveValue('SYNTHETIC-MESH-SERVER-INVITATION-NOT-VALID');
  expect((await calls(page, 'mesh_host_server'))[0]?.args).toEqual({ localPort: 25566, relay, approveLocalPort: true });
  await page.getByRole('button', { name: '结束会话', exact: true }).click();
  await page.getByRole('button', { name: '加入朋友', exact: true }).click();
  await page.getByLabel('私密邀请', { exact: true }).fill('SYNTHETIC-MESH-JOIN-INVITATION-NOT-VALID');
  await page.getByRole('button', { name: '连接游戏', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^可以连接$/ })).toBeVisible();
  await expect(page.getByLabel('Minecraft 服务器地址', { exact: true })).toHaveValue('127.0.0.1:40123');
  expect((await calls(page, 'mesh_join'))[0]?.args).toEqual({ gameId: game.id, invitation: 'SYNTHETIC-MESH-JOIN-INVITATION-NOT-VALID' });
  await page.getByRole('button', { name: '复制地址', exact: true }).click();
  expect((await calls(page, 'plugin:clipboard-manager|write_text'))[1]?.args).toEqual({ text: '127.0.0.1:40123' });
  expect(await page.evaluate(() => Object.values(localStorage).some(value => String(value).includes('SYNTHETIC-MESH')))).toBe(false);
  await capture('synthetic-mesh-joined-560');
  await page.setViewportSize({ width: 1280, height: 800 });
  await capture('synthetic-mesh-joined-1280');
  await page.getByRole('button', { name: '连接详情', exact: true }).click();
  await expect(page.getByText('中继', { exact: true }).first()).toBeVisible();
  await capture('synthetic-mesh-details-1280');
  await page.getByRole('button', { name: '结束会话', exact: true }).click();
  expect((await calls(page, 'mesh_end')).map(call => call.args)).toEqual([{ sessionId: 1 }, { sessionId: 2 }, { sessionId: 3 }]);
  await expect(page.getByLabel('Minecraft 服务器地址', { exact: true })).toHaveCount(0);
});

test('synthetic Mesh disconnect recovery and invitation expiry require explicit actions', async ({ page }, testInfo) => {
  await mockMesh(page);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.getByRole('button', { name: '管理 联机测试', exact: true }).click();
  await page.getByRole('button', { name: 'Mesh 联机', exact: true }).click();
  const capture = async (state: string) => {
    for (const width of [1280, 560]) {
      await page.setViewportSize({ width, height: 800 });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
      const path = testInfo.outputPath(`synthetic-mesh-${state}-${width}.png`);
      await page.screenshot({ path, fullPage: true });
      await testInfo.attach(`synthetic-mesh-${state}-${width}`, { path, contentType: 'image/png' });
    }
  };
  await page.getByRole('button', { name: '加入朋友', exact: true }).click();
  const originalInvitation = 'SYNTHETIC-RECOVERY-INVITATION-NOT-VALID';
  await page.getByLabel('私密邀请', { exact: true }).fill(originalInvitation);
  await page.getByRole('button', { name: '连接游戏', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^可以连接$/ })).toBeVisible();
  await page.evaluate(() => (window as unknown as MeshWindow).__meshScenario('disconnect'));
  await expect(page.getByRole('status').filter({ hasText: /^连接已断开$/ })).toBeVisible({ timeout: 10000 });
  await expect(page.getByLabel('Minecraft 服务器地址', { exact: true })).toHaveCount(0);
  expect(await calls(page, 'mesh_join')).toHaveLength(1);
  await capture('disconnected');
  expect(await calls(page, 'mesh_join')).toHaveLength(1);
  await page.getByRole('button', { name: '重新连接', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^可以连接$/ })).toBeVisible();
  expect((await calls(page, 'mesh_join')).map(call => call.args)).toEqual([
    { gameId: game.id, invitation: originalInvitation },
    { gameId: game.id, invitation: originalInvitation },
  ]);
  await page.getByRole('button', { name: '结束会话', exact: true }).click();
  await page.getByLabel('私密邀请', { exact: true }).fill('SYNTHETIC-EXPIRED-INVITATION');
  await page.getByRole('button', { name: '连接游戏', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^请让好友发送新邀请。$/ })).toBeVisible();
  await expect(page.getByRole('button', { name: '重新连接', exact: true })).toHaveCount(0);
  await capture('invalid-or-expired-invitation');
  await page.getByRole('button', { name: '和朋友一起玩', exact: true }).click();
  await page.evaluate(() => (window as unknown as MeshWindow).__meshScenario('expiredHost'));
  await page.getByRole('button', { name: '创建邀请', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^邀请已过期。结束此会话后创建新邀请。$/ })).toBeVisible();
  await expect(page.getByRole('button', { name: '复制邀请', exact: true })).toHaveCount(0);
  await expect(page.locator('textarea[readonly]')).toHaveCount(0);
  await capture('host-invitation-expired');
  await page.getByRole('button', { name: '结束会话', exact: true }).click();
  expect(await calls(page, 'start_game')).toHaveLength(0);
  expect(await page.evaluate(() => Object.values(localStorage).some(value => String(value).includes('SYNTHETIC-')))).toBe(false);
});

test('synthetic Mesh Inspector renders local diagnostics without private message content', async ({ page }, testInfo) => {
  await mockMesh(page);
  await page.setViewportSize({width:1280,height:900});
  await page.goto('/');
  await page.getByRole('button',{name:'管理 联机测试',exact:true}).click();
  await page.getByRole('button',{name:'Mesh 联机',exact:true}).click();
  await page.getByRole('button',{name:'连接详情',exact:true}).click();
  await page.getByRole('button',{name:'打开检查器',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Mesh 检查器',exact:true})).toBeVisible();
  await expect(page.getByText('传输邻居',{exact:true})).toBeVisible();
  await expect(page.getByText('PRIVATE MESSAGE BODY')).toHaveCount(0);
  await page.getByRole('button',{name:'复制诊断',exact:true}).click();
  const copied=JSON.stringify((await calls(page,'plugin:clipboard-manager|write_text')).at(-1)?.args);
  expect(copied).not.toContain('PRIVATE');
  for(const width of [1280,560]) {
    await page.setViewportSize({width,height:900});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
    const path=testInfo.outputPath(`mesh-inspector-${width}.png`);
    await page.screenshot({path,fullPage:true});
    await testInfo.attach(`mesh-inspector-${width}`,{path,contentType:'image/png'});
  }
  await page.getByRole('button',{name:'返回联机',exact:true}).click();
  await expect(page.getByRole('button',{name:'创建邀请',exact:true})).toBeVisible();
});


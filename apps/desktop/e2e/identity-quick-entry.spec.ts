// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only: no Minecraft process, local server file, or public network request.
import { expect, test, type Page } from '@playwright/test';
import type { Game } from '../src/games';
import type { MeshAppSnapshot, MeshIdentityState, MeshPublicProfile } from '../src/mesh-social';
import type { MeshNetworkSnapshot, MeshOverview } from '../src/mesh';

type Call = { command: string; args: Record<string, unknown> };
type FixtureWindow = Window & { __quickCalls: Call[] };
const game: Game = { id: 'quick-game', name: 'Copper Valley', version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', progress: null, error: null, launch_state: 'idle' };
const saved: MeshPublicProfile = { peer_id: 'SYNTHETIC_FULL_SAVED_MESH_ID_12D3KooW_AAAA', display_name: 'Aster', suffix: '7D42', revision: 1 };
const temporary: MeshPublicProfile = { peer_id: 'SYNTHETIC_FULL_TEMP_MESH_ID_12D3KooW_BBBB', display_name: 'Aster', suffix: '4B19', revision: 1 };
const capabilities = { contact_invites: false, direct_messages: false, lan_only: true, relay_messages: false, groups: false, vlan: false };
const overview: MeshOverview = { available: true, reason: null, can_host: false, component_state: 'idle', mode: 'explicit_relay', discovery_state: 'inactive', discovery_source: null, discovery_total: 0, configured_services: [], nodes: [], neighbors: [], capabilities: { relay: false, direct: false, hole_punch: false }, relay: { address: null, source: null, state: 'unconfigured' }, session: null };

async function mockNative(page: Page, language: string, theme: string, freshIdentity = false) {
  await page.route('https://**/*', route => route.abort());
  await page.addInitScript(({ language, theme, game, saved, temporary, capabilities, overview, freshIdentity }) => {
    const target = window as unknown as FixtureWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true;
    target.__quickCalls = [];
    localStorage.setItem('cobble-language', language);
    localStorage.setItem('cobble.appearance', theme);
    let identity: MeshIdentityState = { profile: freshIdentity ? null : saved, mode: 'persistent', active: !freshIdentity };
    let meshEnabled = true;
    const network = (): MeshNetworkSnapshot => ({ enabled: meshEnabled, phase: meshEnabled ? 'online' : 'offline', generation: 1, attempt: 1, retry_at: null, profile: identity.profile, services: [], last_error: null, relay_available: false, session_route: null, checks: [] });
    const snapshot = (): MeshAppSnapshot | null => identity.active && identity.profile ? { profile: identity.profile, contacts: [], capabilities } : null;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    let callback = 0;
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__quickCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games') return [game];
        if (command === 'list_tasks') return [];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'fixture-player', kind: 'offline', name: 'Player', uuid: 'synthetic', note: null }], active_id: 'fixture-player' };
        if (command === 'create_game_shortcut') return null;
        if (command === 'list_game_servers') return { items: [{ id: 'server-1', name: 'Friends', address: 'play.example.invalid:25565', can_connect: true }], direct_connect: true };
        if (command === 'start_game_server') return game;
        if (command === 'mesh_network_snapshot' || command === 'mesh_network_start') return network();
        if (command === 'mesh_network_set_enabled') { meshEnabled = Boolean(args.enabled); return network(); }
        if (command === 'mesh_network_retry') return network();
        if (command === 'mesh_overview' || command === 'mesh_discover') return meshEnabled ? freshIdentity ? { ...overview, can_host: true, component_state: 'running', discovery_state: 'published', configured_services: [{ id: 'official-relay', kind: 'relay', endpoint: '/dns4/relay.example.invalid/tcp/443/wss/p2p/fixture', state: 'authenticated', discovery: 'unavailable', game_relay: true }] } : overview : { ...overview, available: false, can_host: false, reason: 'MESH_OFFLINE' };
        if (command === 'mesh_identity_state') return identity;
        if (command === 'mesh_app_snapshot') return snapshot();
        if (command === 'mesh_set_identity_mode') { identity = { profile: args.temporary ? temporary : saved, mode: args.temporary ? 'temporary' : 'persistent', active: true }; return snapshot(); }
        if (command === 'mesh_start_app') { identity = { ...identity, active: true }; return snapshot(); }
        if (command === 'mesh_status') return { session: null };
        if (command === 'get_launcher_preferences') return { memory_mb: 4096, game_directory: 'synthetic', custom_game_directory: false };
        if (command === 'get_java_settings') return { mode: 'automatic', executable: null, runtime: null };
        if (command === 'list_java_runtimes') return { runtimes: [], recommendedId: null };
        if (command.startsWith('list_')) return [];
        if (command === 'start_game') throw new Error('Synthetic fixture must not start Minecraft');
        return null;
      },
    };
  }, { language, theme, game, saved, temporary, capabilities, overview, freshIdentity });
}

test('first Mesh visit shows name entry when no identity exists', async ({ page }, info) => {
  await mockNative(page, 'zh-CN', 'light', true);
  await page.setViewportSize({ width: 1000, height: 720 });
  await page.goto('/');
  await expect(page.locator('.mesh-status-button')).toHaveText('在线');
  await page.locator('.mesh-status-button').click();
  await page.getByRole('menuitem', { name: '联机与联系人' }).click();
  await expect(page.locator('form.mesh-start')).toBeVisible();
  await expect(page.getByText('无法加载 Mesh。', { exact: true })).toHaveCount(0);
  expect(await calls(page, 'mesh_start_app')).toHaveLength(0);
  await page.screenshot({ path: info.outputPath('mesh-first-visit.png'), animations: 'disabled' });
  await page.getByRole('button', { name: 'Mesh 检查器', exact: true }).click();
  await expect(page.locator('.mesh-inspector')).toBeVisible();
  await page.screenshot({ path: info.outputPath('mesh-inspector-entry.png'), animations: 'disabled' });
});

async function calls(page: Page, command: string) {
  return page.evaluate(name => (window as unknown as FixtureWindow).__quickCalls.filter(call => call.command === name), command);
}

test('Mesh status badge is before Tasks and switches offline without starting a game', async ({ page }, info) => {
  await mockNative(page, 'zh-CN', 'dark');
  await page.setViewportSize({ width: 560, height: 800 });
  await page.goto('/');
  const badge = page.locator('.mesh-status-button');
  await expect(badge).toHaveText('在线');
  await expect(badge).toHaveCSS('border-top-width', '0px');
  await expect(badge).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
  expect(await badge.evaluate(element => Boolean(element.compareDocumentPosition(document.querySelector('.tasks-button')!) & Node.DOCUMENT_POSITION_FOLLOWING))).toBe(true);
  await badge.click();
  await page.screenshot({ path: info.outputPath('mesh-status-menu.png'), animations: 'disabled' });
  await page.getByRole('menuitem', { name: '切换为离线模式' }).click();
  await expect(badge).toHaveText('离线');
  expect((await calls(page, 'mesh_network_set_enabled')).at(-1)?.args).toEqual({ enabled: false });
  await badge.click();
  await page.getByRole('menuitem', { name: '联机与联系人' }).click();
  await expect(page.getByText('Mesh 已离线，可通过顶部状态上线。')).toBeVisible();
  await expect(page.locator('.mesh-identity-card')).toHaveCount(0);
  await page.screenshot({ path: info.outputPath('mesh-offline.png'), animations: 'disabled' });
  await badge.click();
  await page.getByRole('menuitem', { name: '上线', exact: true }).click();
  await expect(badge).toHaveText('在线');
  expect((await calls(page, 'mesh_network_set_enabled')).at(-1)?.args).toEqual({ enabled: true });
  expect(await calls(page, 'start_game')).toHaveLength(0);
});

for (const { language, theme, width } of [
  { language: 'en-US', theme: 'light', width: 1100 },
  { language: 'zh-CN', theme: 'dark', width: 560 },
]) {
  const en = language === 'en-US';
  test(`identity and quick entry in ${language} ${theme} ${width}px`, async ({ page }, info) => {
    await mockNative(page, language, theme);
    await page.setViewportSize({ width, height: 850 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/');

    const article = page.getByRole('article', { name: game.name });
    const options = article.getByRole('button', { name: en ? 'Options for Copper Valley' : 'Copper Valley的选项' });
    await options.focus();
    await options.press('Enter');
    await expect(page.getByRole('menuitem', { name: en ? 'Create desktop shortcut' : '创建桌面快捷方式' })).toBeVisible();
    await page.screenshot({ path: info.outputPath('shortcut-menu.png'), fullPage: true, animations: 'disabled' });
    await page.getByRole('menuitem', { name: en ? 'Create desktop shortcut' : '创建桌面快捷方式' }).click();
    expect((await calls(page, 'create_game_shortcut'))[0]?.args).toEqual({ id: game.id });
    await expect(article.getByRole('status')).toContainText(en ? 'Desktop shortcut created' : '已创建桌面快捷方式');

    await page.getByRole('button', { name: en ? 'Manage Copper Valley' : '管理 Copper Valley', exact: true }).click();
    await page.getByRole('tab', { name: en ? 'Servers' : '服务器', exact: true }).click();
    const servers = page.locator('.game-servers-panel');
    await expect(servers.locator('.saved-server-row')).toContainText('Friends');
    await expect(servers.locator('.saved-server-row')).toContainText('play.example.invalid:25565');
    expect((await calls(page, 'list_game_servers'))[0]?.args).toEqual({ gameId: game.id });
    await servers.getByRole('button', { name: en ? 'Copy address for Friends' : '复制 Friends 的地址' }).click();
    expect((await calls(page, 'plugin:clipboard-manager|write_text')).at(-1)?.args).toEqual({ text: 'play.example.invalid:25565' });
    await page.screenshot({ path: info.outputPath('saved-servers.png'), fullPage: true, animations: 'disabled' });
    await servers.getByRole('button', { name: en ? 'Join' : '加入', exact: true }).click();
    expect((await calls(page, 'start_game_server'))[0]?.args).toEqual({ id: game.id, serverId: 'server-1' });

    await page.getByRole('button', { name: en ? 'Mesh multiplayer' : 'Mesh 联机', exact: true }).click();
    const identityCard = page.locator('.mesh-identity-card');
    await expect(identityCard).toContainText('Aster#7D42');
    await expect(identityCard.locator('.identity-summary')).not.toContainText(saved.peer_id);
    await page.screenshot({ path: info.outputPath('saved-identity.png'), fullPage: true, animations: 'disabled' });
    await identityCard.getByRole('button', { name: en ? 'Copy identity card' : '复制身份卡' }).click();
    const cardText = (await calls(page, 'plugin:clipboard-manager|write_text')).at(-1)?.args.text;
    expect(JSON.parse(String(cardText))).toMatchObject({ type: 'cobble-mesh-identity', version: 1, peerId: saved.peer_id, displayName: saved.display_name });
    await identityCard.getByRole('button', { name: en ? 'Identity details' : '身份详情' }).click();
    await expect(identityCard).toContainText(saved.peer_id);
    await expect(identityCard.locator('.mesh-identity-avatar')).toHaveCSS('stroke', 'none');
    await identityCard.getByRole('button', { name: en ? 'Copy Mesh ID' : '复制 Mesh ID' }).click();
    expect((await calls(page, 'plugin:clipboard-manager|write_text')).at(-1)?.args).toEqual({ text: saved.peer_id });
    await page.screenshot({ path: info.outputPath('identity-details.png'), fullPage: true, animations: 'disabled' });

    const switchLabel = en ? 'Use temporary identity' : '使用临时身份';
    await identityCard.getByRole('button', { name: switchLabel }).click();
    const dialog = page.getByRole('dialog', { name: switchLabel });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: switchLabel }).click();
    expect((await calls(page, 'mesh_set_identity_mode'))[0]?.args).toEqual({ temporary: true, displayName: saved.display_name });
    await expect(identityCard).toContainText('Aster#4B19');
    await page.screenshot({ path: info.outputPath('temporary-identity.png'), fullPage: true, animations: 'disabled' });

    const returnLabel = en ? 'Return to saved identity' : '恢复已保存身份';
    await identityCard.getByRole('button', { name: returnLabel }).click();
    await page.getByRole('dialog', { name: returnLabel }).getByRole('button', { name: returnLabel }).click();
    expect((await calls(page, 'mesh_set_identity_mode'))[1]?.args).toEqual({ temporary: false, displayName: temporary.display_name });
    await expect(identityCard).toContainText('Aster#7D42');
    await expect(identityCard).toContainText(saved.peer_id);
    await expect(identityCard.getByRole('button', { name: en ? 'Disable contacts' : '关闭联系人' })).toHaveCount(0);
    await expect(identityCard).toContainText(saved.peer_id);
    await page.screenshot({ path: info.outputPath('restored-saved-identity.png'), fullPage: true, animations: 'disabled' });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(await calls(page, 'start_game')).toHaveLength(0);
  });
}

// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only; no Modrinth requests or real downloads occur.
import { test, expect, type Page } from '@playwright/test';
import type { Game } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type Kind = 'resource_pack' | 'shader_pack';
type Hit = { project_id: string; slug: string; title: string; description: string; author: string; downloads: number };
type Version = { version_id: string; name: string; version_number: string; version_type: 'release' | 'beta' | 'alpha'; date_published: string };
type LocalPack = { file_name: string; name: string; description: string | null; size_bytes: number; pack_format: number | null };
type Mock = Window & {
  __calls: Call[];
  __failCommand: string;
  __failSearchOnce: boolean;
  __blockInstall: boolean;
  __finishInstall: () => void;
  __emitGame: (game: Game) => void;
};
const game: Game = { id: 'pack-discovery-game', name: '林间小屋', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installed', error: null, progress: null };
const resourceHit: Hit = { project_id: 'mossy-resources', slug: 'mossy-resources', title: '苔藓与落叶', description: '柔和的林地纹理', author: 'Moss', downloads: 12500 };
const shaderHit: Hit = { project_id: 'soft-shader', slug: 'soft-shader', title: '林间柔光', description: '轻量级柔光效果', author: 'Fern', downloads: 8700 };
const resourceVersion: Version = { version_id: 'mossy-v2', name: '苔藓与落叶', version_number: '2.1.0', version_type: 'release', date_published: '2026-10-20T12:00:00Z' };
const betaVersion: Version = { version_id: 'mossy-v3-beta', name: '苔藓与落叶', version_number: '3.0.0-beta.1', version_type: 'beta', date_published: '2026-10-21T12:00:00Z' };
const shaderVersion: Version = { version_id: 'soft-v1', name: '林间柔光', version_number: '1.4.0', version_type: 'release', date_published: '2026-10-19T12:00:00Z' };
const installedResource: LocalPack = { file_name: 'mossy-resources-2.1.0.zip', name: resourceHit.title, description: resourceHit.description, size_bytes: 2048, pack_format: 15 };
const installedShader: LocalPack = { file_name: 'soft-shader-1.4.0.zip', name: shaderHit.title, description: shaderHit.description, size_bytes: 4096, pack_format: null };

async function setup(page: Page) {
  await page.addInitScript(({ game, resourceHit, shaderHit, resourceVersion, betaVersion, shaderVersion, installedResource, installedShader }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__failCommand = ''; target.__failSearchOnce = false; target.__blockInstall = false;
    let currentGame = game;
    let resources: LocalPack[] = [];
    let shaders: LocalPack[] = [];
    let finishInstall: () => void = () => {};
    let listener = 0;
    const callbacks = new Map<number, (event: unknown) => void>();
    target.__emitGame = (next) => { currentGame = next; callbacks.get(listener)?.({ event: 'game-install', id: 1, payload: next }); };
    target.__finishInstall = () => finishInstall();
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (target.__failCommand === command) throw new Error('synthetic content-pack failure');
        if (command === 'plugin:event|listen') { listener = args.handler as number; return 1; }
        if (command === 'list_games') return [currentGame];
        if (command === 'list_player_profiles') return { profiles: [{ id: 'test-player', kind: 'offline', name: 'Player', uuid: 'synthetic', note: null }], active_id: 'test-player' };
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_game_mods') return [];
        if (command === 'list_game_packs') return args.kind === 'resource_pack' ? resources : shaders;
        if (command === 'search_game_content_packs') {
          if (target.__failSearchOnce) { target.__failSearchOnce = false; throw new Error('synthetic search retry'); }
          const kind = args.kind as Kind;
          const hit = kind === 'resource_pack' ? resourceHit : shaderHit;
          const query = String(args.query);
          const offset = Number(args.offset);
          if (query === 'empty') return { hits: [], offset, total_hits: 0 };
          if (query === 'pages') return { hits: offset === 0 ? [hit] : [{ ...hit, project_id: `${hit.project_id}-second`, title: `${hit.title} Plus` }], offset, total_hits: 2 };
          if (query === 'noversion') return { hits: [{ ...hit, project_id: 'no-version-project', title: `${hit.title} 无兼容版本` }], offset, total_hits: 1 };
          return { hits: [{ ...hit, title: query === 'needle' ? `${hit.title} 搜索结果` : hit.title }], offset, total_hits: 1 };
        }
        if (command === 'list_game_content_pack_versions') {
          const projectId = String(args.projectId);
          if (projectId === 'bad-version-project') throw new Error('synthetic version retry');
          if (projectId === 'no-version-project') return [];
          if (projectId === resourceHit.project_id) return [resourceVersion, betaVersion];
          if (projectId === shaderHit.project_id) return [shaderVersion];
          return [];
        }
        if (command === 'install_game_content_pack') {
          if (target.__blockInstall) await new Promise<void>((resolve) => { finishInstall = resolve; });
          if (args.kind === 'resource_pack') resources = [installedResource];
          else shaders = [installedShader];
          return args.kind === 'resource_pack' ? resources : shaders;
        }
        if (command === 'start_game') return { ...currentGame, launch_state: 'preparing' };
        return null;
      },
    };
  }, { game, resourceHit, shaderHit, resourceVersion, betaVersion, shaderVersion, installedResource, installedShader });
  await page.goto('/');
  await page.getByRole('button', { name: '管理内容' }).click();
}

async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}
async function openDiscovery(page: Page, kind: 'resource_pack' | 'shader_pack') {
  const label = kind === 'resource_pack' ? '资源包' : '光影包';
  await page.getByRole('tab', { name: label, exact: true }).click();
  await page.getByRole('tab', { name: kind === 'resource_pack' ? '发现资源包' : '发现光影包', exact: true }).click();
}
async function search(page: Page, kind: 'resource_pack' | 'shader_pack', query: string) {
  const noun = kind === 'resource_pack' ? '资源包' : '光影包';
  await page.getByRole('searchbox', { name: `搜索 Modrinth ${noun}` }).fill(query);
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  if (query === 'empty') await expect(page.getByText(`没有找到适用的${noun}，换个关键词试试。`)).toBeVisible();
  else await expect(page.getByRole('list', { name: `发现${noun}` })).toBeVisible();
}

test('resource-pack discovery is on demand, keeps version choice explicit, and installs into the installed list', async ({ page }, testInfo) => {
  await setup(page);
  expect(await calls(page, 'search_game_content_packs')).toHaveLength(0);
  expect(await calls(page, 'list_game_content_pack_versions')).toHaveLength(0);
  await openDiscovery(page, 'resource_pack');
  expect(await calls(page, 'search_game_content_packs')).toHaveLength(0);
  await search(page, 'resource_pack', 'moss');
  expect((await calls(page, 'search_game_content_packs'))[0].args).toEqual({ id: game.id, kind: 'resource_pack', query: 'moss', offset: 0 });
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('resource-pack-search-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('resource-pack-search-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: `查看 ${resourceHit.title}`, exact: true }).click();
  await expect(page.getByLabel('资源包版本')).toHaveValue(resourceVersion.version_id);
  expect((await calls(page, 'list_game_content_pack_versions'))[0].args).toEqual({ id: game.id, kind: 'resource_pack', projectId: resourceHit.project_id });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('resource-pack-version-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('resource-pack-version-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '安装资源包', exact: true }).click();
  await expect(page.getByRole('tab', { name: /^已安装/ })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('list', { name: '本地资源包' })).toContainText(resourceHit.title);
  expect((await calls(page, 'install_game_content_pack'))[0].args).toEqual({ id: game.id, kind: 'resource_pack', projectId: resourceHit.project_id, versionId: resourceVersion.version_id });
});

test('resource and shader searches use their own kinds and search failures can retry', async ({ page }) => {
  await setup(page);
  await openDiscovery(page, 'resource_pack');
  await page.evaluate(() => { (window as unknown as Mock).__failSearchOnce = true; });
  await page.getByRole('searchbox', { name: '搜索 Modrinth 资源包' }).fill('needle');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await page.getByRole('button', { name: '重试搜索', exact: true }).click();
  await expect(page.getByRole('button', { name: /查看 .*搜索结果/ })).toBeVisible();
  expect((await calls(page, 'search_game_content_packs')).map((call) => call.args)).toEqual([
    { id: game.id, kind: 'resource_pack', query: 'needle', offset: 0 },
    { id: game.id, kind: 'resource_pack', query: 'needle', offset: 0 },
  ]);
  await page.getByRole('tab', { name: '光影包', exact: true }).click();
  await page.getByRole('tab', { name: '发现光影包', exact: true }).click();
  await search(page, 'shader_pack', 'soft');
  expect((await calls(page, 'search_game_content_packs')).at(-1)?.args).toEqual({ id: game.id, kind: 'shader_pack', query: 'soft', offset: 0 });
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'list_game_content_pack_versions'; });
  await page.getByRole('button', { name: `查看 ${shaderHit.title}`, exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '重试读取版本', exact: true }).click();
  await expect(page.getByLabel('光影包版本')).toHaveValue(shaderVersion.version_id);
  await expect(page.getByRole('button', { name: '安装光影包', exact: true })).toBeEnabled();
});

test('search with no matches and an empty compatible-version set cannot install', async ({ page }) => {
  await setup(page);
  await openDiscovery(page, 'shader_pack');
  await search(page, 'shader_pack', 'empty');
  await expect(page.getByText(/没有找到/)).toBeVisible();
  await expect(page.getByRole('button', { name: /^查看 / })).toHaveCount(0);
  await search(page, 'shader_pack', 'noversion');
  await page.getByRole('button', { name: /查看 .*无兼容版本/ }).click();
  await expect(page.getByText('没有适用于当前游戏的光影包版本。')).toBeVisible();
  await expect(page.getByRole('button', { name: '安装光影包', exact: true })).toHaveCount(0);
});

test('search pagination uses the next offset and preserves query when returning to results', async ({ page }) => {
  await setup(page);
  await openDiscovery(page, 'resource_pack');
  await search(page, 'resource_pack', 'pages');
  await page.getByRole('button', { name: `查看 ${resourceHit.title}`, exact: true }).click();
  await page.getByRole('button', { name: /返回搜索结果/ }).click();
  await page.getByRole('button', { name: '加载更多', exact: true }).click();
  await expect(page.getByRole('button', { name: `查看 ${resourceHit.title} Plus`, exact: true })).toBeVisible();
  expect((await calls(page, 'search_game_content_packs')).map((call) => call.args)).toEqual([
    { id: game.id, kind: 'resource_pack', query: 'pages', offset: 0 },
    { id: game.id, kind: 'resource_pack', query: 'pages', offset: 1 },
  ]);
});

test('install failure retains project and selected version for retry; running prevents installation', async ({ page }) => {
  await setup(page);
  await openDiscovery(page, 'resource_pack');
  await search(page, 'resource_pack', 'moss');
  await page.getByRole('button', { name: `查看 ${resourceHit.title}`, exact: true }).click();
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'install_game_content_pack'; });
  await page.getByRole('button', { name: '安装资源包', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByLabel('资源包版本')).toHaveValue(resourceVersion.version_id);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '重试安装', exact: true }).click();
  await expect(page.getByRole('list', { name: '本地资源包' })).toContainText(resourceHit.title);
  await page.getByRole('tab', { name: '发现资源包', exact: true }).click();
  await expect(page.getByRole('button', { name: '安装资源包', exact: true })).toBeEnabled();
  await page.evaluate((value) => (window as unknown as Mock).__emitGame({ ...value, launch_state: 'running' }), game);
  await expect(page.getByRole('button', { name: '安装资源包', exact: true })).toBeDisabled();
});

test('installation locks navigation and filters until native completion', async ({ page }) => {
  await setup(page);
  await openDiscovery(page, 'shader_pack');
  await search(page, 'shader_pack', 'soft');
  await page.getByRole('button', { name: `查看 ${shaderHit.title}`, exact: true }).click();
  await page.evaluate(() => { (window as unknown as Mock).__blockInstall = true; });
  await page.getByRole('button', { name: '安装光影包', exact: true }).click();
  await expect(page.getByRole('button', { name: '正在安装…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '返回游戏库' })).toBeDisabled();
  await expect(page.getByRole('tab', { name: 'Mod', exact: true })).toBeDisabled();
  await page.evaluate(() => (window as unknown as Mock).__finishInstall());
  await expect(page.getByRole('list', { name: '本地光影包' })).toContainText(shaderHit.title);
  expect(await calls(page, 'install_game_content_pack')).toHaveLength(1);
});

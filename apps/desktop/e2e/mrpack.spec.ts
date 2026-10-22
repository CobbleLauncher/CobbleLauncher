// SPDX-License-Identifier: MIT
// Synthetic native boundary: these browser tests do not download or launch a game.
import { test, expect, type Page } from '@playwright/test';
import type { Game, PackPreview } from '../src/games';

const pack: PackPreview = { preview_id: 'opaque-preview', name: '林间漫游', version_id: '1.2.0', summary: '一个安静的生存世界。', minecraft_version: '1.21.1', loader_version: '0.16.14', required_files: 4, optional_files: [{ path: 'mods/extra.jar', size: 2048 }, { path: 'resourcepacks/leaves.zip', size: 1048576 }], overrides: 2 };
const game: Game = { id: 'synthetic-pack', name: pack.name, pack_name: pack.name, pack_version: pack.version_id, version: pack.minecraft_version, loader_version: pack.loader_version, directory: 'C:\\Cobble\\games\\synthetic-pack', status: 'installing', error: null, progress: { phase: 'pack-download', completed: 1, total: 4, reused: 0 } };
type Call = { command: string; args: Record<string, unknown> };
type TestWindow = Window & { __calls: Call[]; __pick: 'pack' | 'cancel' | 'replacement' | 'unsupported'; __createFails: boolean; __directoryCancel: boolean; __gameEvent: (game: Game) => void };

async function mockNative(page: Page, stored: Game[] = [], packSelection: PackPreview = pack, source = 'bmclapi') {
  const selectedGame = { ...game, loader_kind: packSelection.loader_kind, loader_version: packSelection.loader_version };
  await page.addInitScript(({ pack, game, stored, source }) => {
    const target = window as unknown as TestWindow & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    const callbacks = new Map<number, (event: unknown) => void>();
    const listeners = new Map<string, number>();
    target.isTauri = true;
    target.__calls = [];
    target.__pick = 'pack';
    target.__createFails = false;
    target.__directoryCancel = false;
    target.__gameEvent = (value) => callbacks.get(listeners.get('game-install') ?? -1)?.({ event: 'game-install', id: 1, payload: value });
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (callback: (event: unknown) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id; },
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), args.handler as number); return 1; }
        if (command === 'list_games') return stored;
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_download_settings') return { source, mirror: '' };
        if (command === 'default_game_directory') return 'C:\\Cobble\\games';
        if (command === 'pick_game_directory') return target.__directoryCancel ? null : 'D:\\My Games';
        if (command === 'preview_mrpack') {
          if (target.__pick === 'cancel') return null;
          if (target.__pick === 'unsupported') throw new Error('暂不支持 Quilt 整合包');
          return target.__pick === 'replacement' ? { ...pack, preview_id: 'replacement-id', name: '另一个世界' } : pack;
        }
        if (command === 'create_mrpack_game') {
          if (target.__createFails) throw new Error('整合包预览已失效，请重新选择文件');
          return { ...game, name: args.name };
        }
        if (command === 'retry_game') return game;
        if (command === 'list_game_mods') return [];
        if (command === 'plugin:window|is_maximized') return false;
        return null;
      },
    };
  }, { pack: packSelection, game: selectedGame, stored, source });
}
async function openImport(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: '导入整合包', exact: false }).click();
  await expect(page.getByRole('heading', { name: '导入整合包', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '选择整合包文件', exact: false }).click();
}

test('mrpack creation uses the saved Automatic source choice', async ({ page }) => {
  await mockNative(page, [], pack, 'automatic');
  await openImport(page);
  await expect(page.getByRole('radio', { name: '自动（推荐）', exact: true })).toHaveAttribute('aria-checked', 'true');
  await expect(page.getByText('自动选择响应较快的下载源，失败时切换。')).toBeVisible();
  await page.getByRole('button', { name: '导入并安装' }).click();
  const args = await page.evaluate(() => (window as unknown as TestWindow).__calls.find((call) => call.command === 'create_mrpack_game')?.args);
  expect(args).toEqual({ previewId: pack.preview_id, name: pack.name, source: { source: 'automatic', mirror: '' }, useDefaultDirectory: true, optionalFiles: [] });
});

test('preview choices, cancellation, source and native-owned creation with install events and retry', async ({ page }, testInfo) => {
  await mockNative(page);
  await openImport(page);
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: pack.name, exact: true })).toBeVisible();
  await expect(page.getByText('Fabric 0.16.14', { exact: true })).toBeVisible();
  await expect(page.getByRole('checkbox')).toHaveCount(2);
  await expect(page.getByRole('checkbox').first()).not.toBeChecked();
  await expect(page.getByRole('checkbox').last()).not.toBeChecked();
  await page.getByRole('checkbox').first().check();
  await page.getByLabel('游戏名称').fill('我的林间世界');
  await page.evaluate(() => { (window as unknown as TestWindow).__pick = 'cancel'; });
  await page.getByRole('button', { name: '更换整合包', exact: false }).click();
  await expect(page.getByRole('checkbox').first()).toBeChecked();
  await expect(page.getByLabel('游戏名称')).toHaveValue('我的林间世界');
  await page.getByRole('radio', { name: '自定义镜像', exact: true }).click();
  await page.getByLabel('镜像地址').fill('https://mirror.example.com/');
  await page.getByRole('button', { name: '更改', exact: false }).click();
  await expect(page.getByRole('button', { name: 'D:\\My Games 更改' })).toBeVisible();
  await page.evaluate(() => { (window as unknown as TestWindow).__directoryCancel = true; });
  await page.getByRole('button', { name: '更改', exact: false }).click();
  await expect(page.getByRole('button', { name: 'D:\\My Games 更改' })).toBeVisible();
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
  await page.screenshot({ path: testInfo.outputPath('mrpack-preview-light.png'), fullPage: true });
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.screenshot({ path: testInfo.outputPath('mrpack-preview-dark.png'), fullPage: true });
  await page.setViewportSize({ width: 560, height: 420 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('button', { name: '导入并安装' }).click();
  await expect(page.getByRole('article', { name: '我的林间世界' })).toBeVisible();
  const calls = await page.evaluate(() => (window as unknown as TestWindow).__calls);
  expect(calls.find((call) => call.command === 'preview_mrpack')?.args).toEqual({});
  expect(calls.find((call) => call.command === 'create_mrpack_game')?.args).toEqual({ previewId: pack.preview_id, name: '我的林间世界', source: { source: 'custom', mirror: 'https://mirror.example.com/' }, useDefaultDirectory: false, optionalFiles: ['mods/extra.jar'] });
  await expect(page.getByRole('status')).toHaveText('下载整合包文件1 / 4');
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...game, progress: { phase: 'pack-base-complete', completed: 1, total: 1, reused: 0 } } satisfies Game);
  await expect(page.getByRole('status')).toHaveText('准备整合包文件1 / 1');
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...game, progress: { phase: 'pack-install', completed: 1, total: 4, reused: 0 } } satisfies Game);
  await expect(page.getByRole('status')).toHaveText('安装整合包文件1 / 4');
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...game, name: '我的林间世界', progress: { phase: 'pack-overrides', completed: 1, total: 2, reused: 0 } } satisfies Game);
  await expect(page.getByRole('status')).toHaveText('安装整合包配置1 / 2');
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...game, status: 'failed', error: '整合包下载中断' } satisfies Game);
  await expect(page.getByRole('alert')).toContainText('整合包下载中断');
  await page.getByRole('button', { name: '重试安装' }).click();
  await expect(page.getByRole('progressbar')).toBeVisible();
  await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...game, status: 'installed', progress: { phase: 'complete', completed: 4, total: 4, reused: 0 } } satisfies Game);
  await expect(page.getByText('整合包 · 林间漫游 · 1.2.0')).toBeVisible();
  await page.getByRole('button', { name: '管理内容' }).click();
  await expect(page.getByText('整合包 · 林间漫游 · 1.2.0')).toBeVisible();
});

test('replacement clears optionals; readable unsupported and creation errors remain retryable', async ({ page }) => {
  await mockNative(page, [{ ...game, status: 'installed' }]);
  await openImport(page);
  await page.getByRole('checkbox').first().check();
  await page.evaluate(() => { (window as unknown as TestWindow).__pick = 'replacement'; });
  await page.getByRole('button', { name: '更换整合包', exact: false }).click();
  await expect(page.getByLabel('游戏名称')).toHaveValue('另一个世界');
  await expect(page.getByRole('checkbox').first()).not.toBeChecked();
  await page.evaluate(() => { (window as unknown as TestWindow).__pick = 'unsupported'; });
  await page.getByRole('button', { name: '更换整合包', exact: false }).click();
  await expect(page.getByRole('alert')).toContainText('暂不支持 Quilt');
  await expect(page.getByLabel('游戏名称')).toHaveValue('另一个世界');
  await page.evaluate(() => { (window as unknown as TestWindow).__createFails = true; });
  await page.getByRole('button', { name: '导入并安装' }).click();
  await expect(page.getByRole('alert')).toContainText('整合包预览已失效');
  await expect(page.getByRole('button', { name: '导入并安装' })).toBeEnabled();
  await page.getByRole('radio', { name: '自定义镜像', exact: true }).click();
  await page.getByRole('button', { name: '导入并安装' }).click();
  await expect(page.getByRole('alert')).toContainText('HTTPS');
  await page.getByRole('radio', { name: '官方', exact: true }).click();
  await page.evaluate(() => { (window as unknown as TestWindow).__createFails = false; });
  await page.getByRole('button', { name: '导入并安装' }).click();
  await expect(page.getByRole('heading', { name: '游戏库' })).toBeVisible();
  const args = await page.evaluate(() => (window as unknown as TestWindow).__calls.filter((call) => call.command === 'create_mrpack_game').at(-1)?.args);
  expect(args).toEqual({ previewId: 'replacement-id', name: '另一个世界', source: { source: 'official', mirror: '' }, useDefaultDirectory: true, optionalFiles: [] });
});

test('browser import requires the desktop app and does not invent a preview', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: '导入整合包' }).click();
  await page.getByRole('button', { name: '选择整合包文件', exact: false }).click();
  await expect(page.getByRole('alert')).toHaveText('请在桌面应用中导入整合包');
  await expect(page.getByRole('button', { name: '导入并安装' })).toHaveCount(0);
});

for (const [kind, label, version] of [['forge', 'Forge', '52.0.28'], ['neoforge', 'NeoForge', '21.1.100']] as const) {
  test(`${label} pack preview and creation preserve the native-selected loader`, async ({ page }) => {
    await mockNative(page, [], { ...pack, loader_kind: kind, loader_version: version });
    await openImport(page);
    await expect(page.getByText(`${label} ${version}`, { exact: true })).toBeVisible();
    await expect(page.getByText(`安装时会运行 ${label} 官方安装器。`, { exact: true })).toBeVisible();
    await expect(page.getByText('Fabric', { exact: false })).toHaveCount(0);
    await expect(page.getByText('Minecraft 文件使用所选下载源，整合包文件使用包内来源。')).toBeVisible();
    await page.getByRole('button', { name: '导入并安装' }).click();
    await expect(page.getByRole('article')).toContainText(`${label} ${version}`);
    const args = await page.evaluate(() => (window as unknown as TestWindow).__calls.find((call) => call.command === 'create_mrpack_game')?.args);
    expect(args).toEqual({ previewId: pack.preview_id, name: pack.name, source: { source: 'bmclapi', mirror: '' }, useDefaultDirectory: true, optionalFiles: [] });
    await page.evaluate((game) => (window as unknown as TestWindow).__gameEvent(game), { ...game, loader_kind: kind, loader_version: version, progress: { phase: kind === 'forge' ? 'pack-base-forge-installer' : 'pack-base-neoforge-installer', completed: 0, total: 1, reused: 0 } } satisfies Game);
    await expect(page.getByRole('status')).toHaveText(`安装 ${label}`);
    await expect(page.getByRole('progressbar')).toHaveCount(0);
  });
}

// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary only; no Modrinth requests or game downloads occur.
import { test, expect, type Page } from '@playwright/test';
import type { Game, PackPreview } from '../src/games';

type Call = { command: string; args: Record<string, unknown> };
type SearchHit = { project_id: string; slug: string; title: string; description: string; author: string; downloads: number };
type PackVersion = { version_id: string; name: string; version_number: string; minecraft_versions: string[]; loaders: string[]; version_type: 'release' | 'beta' | 'alpha'; date_published: string };
type Mock = Window & {
  __calls: Call[];
  __failCommand: string;
  __blockNextPreview: () => void;
  __finishPreview: () => void;
};
const game: Game = { id: 'imported-pack', name: 'Quiet Forest', version: '1.21.1', loader_version: '0.16.14', directory: 'synthetic', status: 'installing', error: null, progress: null };
const hit: SearchHit = { project_id: 'quiet-forest-project', slug: 'quiet-forest', title: 'Quiet Forest', description: 'A calm woodland pack', author: 'Moss', downloads: 42000 };
const version: PackVersion = { version_id: 'quiet-forest-v3', name: 'Quiet Forest', version_number: '3.0.0', minecraft_versions: ['1.21.1'], loaders: ['fabric'], version_type: 'release', date_published: '2026-10-20T12:00:00Z' };
const preview: PackPreview = { preview_id: 'opaque-pack-preview', name: 'Quiet Forest', version_id: '3.0.0', summary: 'A calm woodland pack', minecraft_version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', required_files: 3, optional_files: [{ path: 'resourcepacks/leaves.zip', size: 4096 }], overrides: 1 };

async function setup(page: Page) {
  await page.addInitScript(({ game, hit, version, preview }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__failCommand = '';
    let previewBlocked = false;
    let finishPreview: () => void = () => {};
    target.__blockNextPreview = () => { previewBlocked = true; };
    target.__finishPreview = () => { previewBlocked = false; finishPreview(); };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => 1,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (target.__failCommand === command) throw new Error('synthetic unavailable');
        if (command === 'plugin:event|listen') return 1;
        if (command === 'list_games') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'get_download_settings') return { source: 'automatic', mirror: '' };
        if (command === 'default_game_directory') return 'C:\\Cobble\\games';
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'search_modrinth_packs') {
          const query = String(args.query);
          if (query === 'slow') await new Promise((resolve) => setTimeout(resolve, 450));
          if (query === 'empty') return { hits: [], offset: args.offset, total_hits: 0 };
          return { hits: [{ ...hit, title: query === 'slow' ? 'Slow Forest' : query === 'fast' ? 'Fast Forest' : hit.title }], offset: args.offset, total_hits: 1 };
        }
        if (command === 'list_modrinth_pack_versions') {
          if (args.minecraftVersion === '9.9.9' || args.loader === 'forge') return [];
          return [version];
        }
        if (command === 'preview_modrinth_pack') {
          if (previewBlocked) await new Promise<void>((resolve) => { finishPreview = resolve; });
          return preview;
        }
        if (command === 'create_mrpack_game') return { ...game, id: 'created-pack', name: args.name };
        return null;
      },
    };
  }, { game, hit, version, preview });
  await page.goto('/');
}
async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}
async function openDiscovery(page: Page) {
  await page.getByRole('button', { name: '发现整合包', exact: true }).click();
  await expect(page.getByRole('heading', { name: '发现整合包', exact: true })).toBeVisible();
}
async function search(page: Page, query = 'quiet forest') {
  await page.getByRole('searchbox', { name: '搜索整合包' }).fill(query);
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('button', { name: `查看 ${hit.title}`, exact: true })).toBeVisible();
}

test('discovery makes no initial request and searches only after explicit submit with filters', async ({ page }) => {
  await setup(page);
  expect(await calls(page, 'search_modrinth_packs')).toHaveLength(0);
  expect(await calls(page, 'list_modrinth_pack_versions')).toHaveLength(0);
  await openDiscovery(page);
  await page.getByLabel('Minecraft 版本').fill('1.21.1');
  await page.getByLabel('加载器').selectOption('fabric');
  await page.getByRole('searchbox', { name: '搜索整合包' }).fill('quiet forest');
  expect(await calls(page, 'search_modrinth_packs')).toHaveLength(0);
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('list', { name: '发现整合包' })).toContainText('Quiet Forest');
  expect((await calls(page, 'search_modrinth_packs'))[0].args).toEqual({ query: 'quiet forest', minecraftVersion: '1.21.1', loader: 'fabric', offset: 0 });
  await page.getByRole('button', { name: '查看 Quiet Forest', exact: true }).click();
  await expect(page.getByLabel('整合包版本')).toBeVisible();
  expect((await calls(page, 'list_modrinth_pack_versions'))[0].args).toEqual({ projectId: hit.project_id, minecraftVersion: '1.21.1', loader: 'fabric' });
  await expect(page.getByLabel('整合包版本')).toHaveValue(version.version_id);
});

test('preview is explicit, locks controls while loading, and installation requires final confirmation', async ({ page }, testInfo) => {
  await setup(page);
  await openDiscovery(page);
  await page.getByLabel('Minecraft 版本').fill('1.21.1');
  await page.getByLabel('加载器').selectOption('fabric');
  await search(page);
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('discovery-results-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('discovery-results-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '查看 Quiet Forest', exact: true }).click();
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('discovery-version-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('discovery-version-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.evaluate(() => (window as unknown as Mock).__blockNextPreview());
  await page.getByRole('button', { name: '查看安装预览', exact: true }).click();
  await expect(page.getByRole('button', { name: '准备预览中…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: /返回搜索结果/ })).toBeDisabled();
  await expect(page.getByLabel('整合包版本')).toBeDisabled();
  expect(await calls(page, 'create_mrpack_game')).toHaveLength(0);
  await page.evaluate(() => (window as unknown as Mock).__finishPreview());
  await expect(page.getByRole('heading', { name: '导入整合包', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Quiet Forest', exact: true })).toBeVisible();
  expect((await calls(page, 'preview_modrinth_pack'))[0].args).toEqual({ projectId: hit.project_id, versionId: version.version_id });
  expect(await calls(page, 'create_mrpack_game')).toHaveLength(0);
  await page.getByRole('button', { name: '返回发现整合包', exact: true }).click();
  await expect(page.getByLabel('整合包版本')).toHaveValue(version.version_id);
  await page.getByRole('button', { name: /返回搜索结果/ }).click();
  await expect(page.getByRole('searchbox', { name: '搜索整合包' })).toHaveValue('quiet forest');
  await expect(page.getByLabel('Minecraft 版本')).toHaveValue('1.21.1');
  await expect(page.getByLabel('加载器')).toHaveValue('fabric');
  await page.getByRole('button', { name: '查看 Quiet Forest', exact: true }).click();
  await expect(page.getByLabel('整合包版本')).toHaveValue(version.version_id);
  await page.getByRole('button', { name: '查看安装预览', exact: true }).click();
  await expect(page.getByRole('heading', { name: '导入整合包', exact: true })).toBeVisible();
  await page.setViewportSize({ width: 560, height: 800 });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: testInfo.outputPath('discovered-pack-light-560.png'), fullPage: true, animations: 'disabled' });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
  await page.screenshot({ path: testInfo.outputPath('discovered-pack-dark-560.png'), fullPage: true, animations: 'disabled' });
  await page.getByRole('button', { name: '导入并安装', exact: true }).click();
  const args = (await calls(page, 'create_mrpack_game')).at(-1)?.args;
  expect(args).toEqual({ previewId: preview.preview_id, name: preview.name, source: { source: 'automatic', mirror: '' }, useDefaultDirectory: true, optionalFiles: [] });
});

test('search failure retries retained filters and late search responses cannot replace newer results', async ({ page }) => {
  await setup(page);
  await openDiscovery(page);
  await page.getByLabel('Minecraft 版本').fill('1.21.1');
  await page.getByLabel('加载器').selectOption('fabric');
  await page.getByRole('searchbox', { name: '搜索整合包' }).fill('quiet forest');
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'search_modrinth_packs'; });
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: /重试/ }).click();
  await expect(page.getByRole('button', { name: '查看 Quiet Forest', exact: true })).toBeVisible();
  expect((await calls(page, 'search_modrinth_packs')).at(-1)?.args).toEqual({ query: 'quiet forest', minecraftVersion: '1.21.1', loader: 'fabric', offset: 0 });
  await page.getByRole('searchbox', { name: '搜索整合包' }).fill('slow');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect.poll(async () => (await calls(page, 'search_modrinth_packs')).length).toBe(3);
  await page.getByRole('searchbox', { name: '搜索整合包' }).fill('fast');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('button', { name: '查看 Fast Forest', exact: true })).toBeVisible();
  await page.waitForTimeout(500);
  await expect(page.getByRole('button', { name: '查看 Slow Forest', exact: true })).toHaveCount(0);
});

test('unsupported filters show no installable version and version lookup can retry', async ({ page }) => {
  await setup(page);
  await openDiscovery(page);
  await page.getByLabel('Minecraft 版本').fill('9.9.9');
  await page.getByRole('searchbox', { name: '搜索整合包' }).fill('quiet forest');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('button', { name: '查看 Quiet Forest', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '查看 Quiet Forest', exact: true }).click();
  await expect(page.getByText('没有符合筛选条件的可安装版本。')).toBeVisible();
  await expect(page.getByRole('button', { name: '查看安装预览', exact: true })).toHaveCount(0);
  expect(await calls(page, 'preview_modrinth_pack')).toHaveLength(0);
  expect((await calls(page, 'search_modrinth_packs'))[0].args).toEqual({ query: 'quiet forest', minecraftVersion: '9.9.9', loader: null, offset: 0 });
});

test('version and preview errors retain the project and retry the selected version', async ({ page }) => {
  await setup(page);
  await openDiscovery(page);
  await search(page);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'list_modrinth_pack_versions'; });
  await page.getByRole('button', { name: '查看 Quiet Forest', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '重试读取版本', exact: true }).click();
  await expect(page.getByLabel('整合包版本')).toHaveValue(version.version_id);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = 'preview_modrinth_pack'; });
  await page.getByRole('button', { name: '查看安装预览', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByLabel('整合包版本')).toHaveValue(version.version_id);
  await page.evaluate(() => { (window as unknown as Mock).__failCommand = ''; });
  await page.getByRole('button', { name: '重试安装预览', exact: true }).click();
  await expect(page.getByRole('heading', { name: '导入整合包', exact: true })).toBeVisible();
  expect((await calls(page, 'preview_modrinth_pack')).map((call) => call.args)).toEqual([
    { projectId: hit.project_id, versionId: version.version_id }, { projectId: hit.project_id, versionId: version.version_id },
  ]);
});

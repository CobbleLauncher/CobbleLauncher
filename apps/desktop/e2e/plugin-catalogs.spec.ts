// SPDX-License-Identifier: MIT
// Synthetic catalog and Tauri boundary only; no remote catalog or Wasm is fetched.
import { test, expect, type Page } from '@playwright/test';
import type { CatalogManifest, PluginCatalogView } from '../src/plugin-catalogs';

type Call = { command: string; args: Record<string, unknown> };
type Mock = Window & { __calls: Call[]; __refreshFails: boolean };
const manifest: CatalogManifest = {
  id: 'org.example.schematic-viewer', name: 'Schematic Viewer', version: '1.2.0',
  api_version: '1.0.0', publisher: 'Example Tools', capabilities: ['viewer'],
  file_extensions: ['schem'], component_sha256: 'a'.repeat(64),
};
const sourceUrl = 'https://catalog.example.test/community/catalog.json';
const catalog: PluginCatalogView = {
  source: { id: 'community', name: 'Community Tools', url: sourceUrl, enabled: true },
  snapshot: {
    source_id: 'community', source_url: sourceUrl, retrieved_at: 1792627200,
    index: { schema_version: 1, name: 'Community Tools Index', plugins: [{
      manifest, description: 'Preview local schematic files.',
      component_url: 'https://catalog.example.test/components/schematic-viewer-1.2.0.wasm',
      project_url: 'https://catalog.example.test/projects/schematic-viewer',
    }] },
  },
};

async function setup(page: Page, subscribed = false) {
  await page.addInitScript(({ catalog, manifest, subscribed }) => {
    const target = window as unknown as Mock & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; target.__calls = []; target.__refreshFails = false;
    let catalogs = subscribed ? [catalog] : [];
    let callbackId = 0;
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callbackId,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__calls.push({ command, args });
        if (command === 'plugin:event|listen') return callbackId;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games' || command === 'list_plugins' || command === 'list_bundled_plugins') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'list_plugin_catalogs') return catalogs;
        if (command === 'add_plugin_catalog') { catalogs = [catalog]; return catalogs; }
        if (command === 'refresh_plugin_catalog') {
          catalogs = catalogs.map((item) => ({ ...item, error: target.__refreshFails ? 'Catalog refresh unavailable; showing cached entries.' : undefined }));
          return catalogs;
        }
        if (command === 'set_plugin_catalog_enabled') {
          catalogs = catalogs.map((item) => ({ ...item, source: { ...item.source, enabled: Boolean(args.enabled) } }));
          return catalogs;
        }
        if (command === 'preview_catalog_plugin') return manifest;
        if (command === 'cancel_plugin_preview') return null;
        if (command === 'install_plugin') return [{ manifest, enabled: true }];
        return null;
      },
    };
  }, { catalog, manifest, subscribed });
  await page.goto('/');
  await page.getByRole('button', { name: '打开设置' }).click();
  await page.getByRole('tab', { name: '扩展', exact: true }).click();
  await expect(page.getByRole('heading', { name: '插件目录', exact: true })).toBeVisible();
}

async function calls(page: Page, command: string) {
  return page.evaluate((name) => (window as unknown as Mock).__calls.filter((call) => call.command === name), command);
}

test('catalog subscription is explicit and failed refresh retains cached entries', async ({ page }) => {
  const remoteRequests: string[] = [];
  page.on('request', (request) => { if (new URL(request.url()).hostname !== '127.0.0.1') remoteRequests.push(request.url()); });
  await setup(page);
  expect(await calls(page, 'list_plugin_catalogs')).toHaveLength(1);
  for (const command of ['add_plugin_catalog', 'refresh_plugin_catalog', 'preview_catalog_plugin', 'install_plugin']) expect(await calls(page, command)).toHaveLength(0);
  expect(remoteRequests).toEqual([]);
  await page.getByLabel('目录名称', { exact: true }).fill('Community Tools');
  await page.getByLabel('HTTPS 地址', { exact: true }).fill(sourceUrl);
  expect(await calls(page, 'add_plugin_catalog')).toHaveLength(0);
  await page.getByRole('button', { name: '添加目录', exact: true }).click();
  await expect(page.getByRole('heading', { name: manifest.name, exact: true })).toBeVisible();
  expect((await calls(page, 'add_plugin_catalog'))[0].args).toEqual({ name: 'Community Tools', url: sourceUrl });
  await page.evaluate(() => { (window as unknown as Mock).__refreshFails = true; });
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('showing cached entries');
  await expect(page.getByRole('heading', { name: manifest.name, exact: true })).toBeVisible();
  await page.getByRole('button', { name: '禁用', exact: true }).click();
  await expect(page.getByRole('button', { name: '安装', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeDisabled();
  expect(await calls(page, 'preview_catalog_plugin')).toHaveLength(0);
  expect(await calls(page, 'install_plugin')).toHaveLength(0);
  expect(remoteRequests).toEqual([]);
});

test('catalog preview requires parent approval and cancellation does not install', async ({ page }, testInfo) => {
  await setup(page, true);
  await page.getByRole('button', { name: '安装', exact: true }).click();
  await expect(page.getByRole('button', { name: '允许并安装', exact: true })).toBeVisible();
  await expect(page.getByText('来源：Community Tools', { exact: true })).toBeVisible();
  await expect(page.getByText('读取你打开的 .schem 文件并提供预览。', { exact: true })).toBeVisible();
  expect((await calls(page, 'preview_catalog_plugin'))[0].args).toEqual({ sourceId: 'community', pluginId: manifest.id });
  expect(await calls(page, 'install_plugin')).toHaveLength(0);
  for (const width of [560, 340]) {
    await page.setViewportSize({ width, height: 800 });
    await expect(page.getByRole('button', { name: '允许并安装', exact: true })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath(`catalog-approval-${width}.png`), fullPage: true, animations: 'disabled' });
  }
  await page.getByRole('button', { name: '取消', exact: true }).click();
  await expect(page.getByRole('button', { name: '允许并安装', exact: true })).toHaveCount(0);
  expect(await calls(page, 'cancel_plugin_preview')).toHaveLength(1);
  expect(await calls(page, 'install_plugin')).toHaveLength(0);
  await page.getByRole('button', { name: '安装', exact: true }).click();
  await page.getByRole('button', { name: '允许并安装', exact: true }).click();
  await expect(page.getByRole('button', { name: '卸载', exact: true })).toBeVisible();
  expect((await calls(page, 'install_plugin')).map((call) => call.args)).toEqual([{ id: manifest.id }]);
});

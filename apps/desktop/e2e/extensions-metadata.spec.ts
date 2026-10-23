// SPDX-License-Identifier: MIT
// Synthetic Tauri listings; verifies metadata/search/approval rendering, not native installation.
import { expect, test } from '@playwright/test';
test('extensions show searchable metadata and retain approval at narrow widths', async ({ page }, info) => {
  await page.addInitScript(() => {
    const target = window as unknown as { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown };
    target.isTauri = true; localStorage.setItem('cobble-language', 'en-US'); let callback = 0;
    const manifest = { id: 'sample.catalog', name: 'Blueprint Catalog', description: { en: 'Find original community blueprints', 'zh-CN': '查找社区原创蓝图' }, tags: ['blueprints', 'community'], version: '1.0', api_version: '0.2.0', publisher: 'Example author', capabilities: ['blueprint-provider'], file_extensions: [], provider_origins: ['https://catalog.example'], component_sha256: 'a'.repeat(64) };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback, invoke: async (command: string) => {
      if (command === 'plugin:event|listen') return callback;
      if (command === 'plugin:window|is_maximized') return false;
      if (command === 'list_player_profiles') return { profiles: [], active_id: null };
      if (command === 'list_bundled_plugins') return [manifest,...Array.from({length:11},(_,index)=>({...manifest,id:`sample.helper${index}`,name:`Helper ${index+1}`,description:{en:'Local utility', 'zh-CN':'本地工具'},tags:['utility']}))];
      if (command === 'preview_bundled_plugin') return manifest;
      if (['list_games', 'list_tasks', 'list_plugins', 'list_extension_contributions', 'list_plugin_catalogs'].includes(command)) return [];
      return null;
    } };
  });
  await page.setViewportSize({ width: 560, height: 800 }); await page.goto('/');
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  await page.getByRole('tab', { name: 'Discover', exact: true }).click();
  await expect(page.locator('.bundled-plugins>ul>li')).toHaveCount(8);
  await expect(page.getByText('Page 1 of 2 · 12 items',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Next',exact:true}).click();
  await expect(page.locator('.bundled-plugins>ul>li')).toHaveCount(4);
  await expect(page.getByText('Page 2 of 2 · 12 items',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Previous',exact:true}).click();
  await expect(page.locator('.plugin-identity').filter({hasText:'Blueprint Catalog'})).toContainText('Find original community blueprints');
  await page.getByRole('searchbox', { name: 'Search extensions', exact: true }).fill('community');
  await expect(page.locator('.bundled-plugins>ul>li')).toHaveCount(1);
  await expect(page.getByLabel('Catalog name', { exact: true })).not.toBeVisible();
  await page.screenshot({ path: info.outputPath('extensions-metadata-560.png'), animations: 'disabled' });
  await page.getByRole('searchbox').fill('no match');
  await expect(page.locator('.bundled-plugins>ul>li')).toHaveCount(0);
  await page.getByRole('searchbox').fill('blueprints');
  await page.locator('.bundled-plugins button').click();
  await expect(page.locator('.plugin-approval')).toContainText('https://catalog.example');
  await expect(page.locator('.plugin-approval')).toContainText('1.0');
  await expect(page.locator('.plugin-approval .primary-button')).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

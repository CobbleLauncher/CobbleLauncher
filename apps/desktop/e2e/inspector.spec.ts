// SPDX-License-Identifier: MIT
// Synthetic Tauri boundary: this checks paging/rendering, not picker or Wasm behavior.
import { expect, test } from '@playwright/test';
import type { InspectorQuery } from '../src/inspector/types';
test('Inspector pages server data, searches beyond the first page, and renders full values safely at 560px', async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    const target = window as unknown as Window & { isTauri: boolean; __TAURI_INTERNALS__: unknown; __TAURI_EVENT_PLUGIN_INTERNALS__: unknown; __inspectorCalls: { command: string; args: Record<string, unknown> }[] };
    target.isTauri = true; target.__inspectorCalls = []; localStorage.setItem('cobble-language', 'en-US');
    let callback = 0;
    const cell = (text: string, complete = true) => ({ text, byte_length: new TextEncoder().encode(text).length, complete });
    const summary = { name: cell('Copper Pack'), version_id: cell('1.0'), declared_files: 122, override_files: 0, declared_download_bytes: 8192 };
    target.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    target.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } }, transformCallback: () => ++callback,
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        target.__inspectorCalls.push({ command, args });
        if (command === 'plugin:event|listen') return callback;
        if (command === 'plugin:window|is_maximized') return false;
        if (command === 'list_games' || command === 'list_tasks' || command === 'list_plugin_catalogs' || command === 'list_bundled_plugins') return [];
        if (command === 'list_player_profiles') return { profiles: [], active_id: null };
        if (command === 'list_plugins') return [{ enabled: true, manifest: { id: 'custom.inspector', name: 'Pack Inspector', version: '1', api_version: '0.1.0', publisher: 'Example', capabilities: ['report'], file_extensions: ['json','mrpack'], report_protocols: ['cobble.mrpack-page@1'] } }];
        if (command === 'begin_pack_inspection') return { sessionId: 'synthetic-inspector', summary: { current: summary, baseline: null, changes: null } };
        if (command === 'query_pack_inspection') {
          const query = args.query as InspectorQuery;
          const ids = Array.from({ length: 122 }, (_, i) => i).filter(i => !query.search || `mods/item-${i}.jar`.includes(query.search));
          const rows = ids.slice(query.offset, query.offset + query.limit).map(i => ({ id: `row-${i}`, path: cell(`mods/item-${i}.jar`), source: 'declared_file', change: null, before: null, after: cell('<script>metadata preview</script>', false) }));
          return { sessionId: 'synthetic-inspector', revision: args.revision, page: { query, total_rows: 122, matched_rows: ids.length, next_offset: query.offset + rows.length < ids.length ? query.offset + rows.length : null, rows }, report: { schema_version: 1, findings: [], tables: [] } };
        }
        if (command === 'detail_pack_inspection') {
          const query = args.query as { row_id: string; field: string; offset: number };
          return { sessionId: 'synthetic-inspector', revision: args.revision, detail: { row_id: query.row_id, field: query.field, offset: query.offset, next_offset: null, total_bytes: 24, text: '<img src=x>full metadata' } };
        }
        return null;
      },
    };
  });
  await page.setViewportSize({ width: 560, height: 800 }); await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.getByRole('button', { name: /打开设置|Open settings/ }).click();
  await page.getByRole('tab', { name: /扩展|Extensions/, exact: true }).click();
  await page.getByRole('button', { name: 'Open modpack', exact: true }).click();
  await expect(page.locator('.inspector-panel tbody tr')).toHaveCount(50);
  await expect(page.locator('.inspector-panel')).toContainText('Items 1–50 of 122');
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(page.locator('.inspector-panel')).toContainText('Items 51–100 of 122');
  await expect(page.locator('.inspector-panel tbody tr').first()).toContainText('mods/item-50.jar');
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(page.locator('.inspector-panel tbody tr')).toHaveCount(22);
  await expect(page.getByRole('button', { name: 'Next page', exact: true })).toBeDisabled();
  await page.getByLabel('Search all contents', { exact: true }).fill('item-121.jar');
  await expect(page.locator('.inspector-panel tbody tr')).toHaveCount(1);
  await expect(page.locator('.inspector-panel')).toContainText('Items 1–1 of 1');
  await expect(page.locator('.inspector-panel')).toContainText('122 total items');
  await page.locator('.inspector-panel tbody button').click();
  await expect(page.locator('.inspector-detail pre')).toHaveText('<img src=x>full metadata');
  await expect(page.locator('.inspector-panel img, .inspector-panel script')).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('inspector-paging-560.png'), fullPage: true, animations: 'disabled' });
  await page.locator('.inspector-panel > .inspector-actions').getByRole('button', { name: 'Close', exact: true }).click();
  await expect(page.locator('.inspector-panel tbody')).toHaveCount(0);
  expect(await page.evaluate(() => (window as unknown as { __inspectorCalls: { command: string; args: unknown }[] }).__inspectorCalls.filter(call => call.command === 'close_pack_inspection').at(-1)?.args)).toEqual({ sessionId: 'synthetic-inspector' });
});
